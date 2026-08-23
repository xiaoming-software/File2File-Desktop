use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::Emitter;

use crate::nas;
use crate::storage;
use crate::webrpc;

const GET_FILE_TIMEOUT_MS: i64 = 10_000;
const STALL_WAIT: Duration = Duration::from_secs(180);
const IDLE_START_WAIT: Duration = Duration::from_secs(45);
const FAIL_CLOSED: &str = "连接已断开";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NasTask {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub name: String,
    pub nas_path: String,
    pub size: u64,
    #[serde(default)]
    pub has_size: bool,
    pub transferred: u64,
    pub elapsed_ms: u64,
    pub speed_bps: u64,
    pub local_path: String,
    pub dest_dir: String,
    #[serde(default)]
    pub error: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct TaskFile {
    #[serde(default)]
    tasks: Vec<NasTask>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct NasTaskEvent {
    peer_token: String,
    tasks: Vec<NasTask>,
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct DriveKey {
    owner: String,
    peer: String,
}

#[derive(Clone)]
struct LiveXfer {
    task_id: String,
    started: Instant,
    size: u64,
    bytes: u64,
}

#[derive(Default)]
struct TaskState {
    files: HashMap<DriveKey, Vec<NasTask>>,
    sessions: HashMap<u32, DriveKey>,
    gens: HashMap<DriveKey, u64>,
    pumping: HashSet<DriveKey>,
    busy: HashSet<u32>,
    cancelled: HashMap<u32, Arc<AtomicBool>>,
    uploads: HashMap<u32, LiveXfer>,
    downloads: HashMap<u32, LiveXfer>,
}

static NEXT_ID: AtomicU64 = AtomicU64::new(1);
static STATE: OnceLock<Mutex<TaskState>> = OnceLock::new();

fn lock() -> MutexGuard<'static, TaskState> {
    STATE
        .get_or_init(|| Mutex::new(TaskState::default()))
        .lock()
        .unwrap_or_else(|err| err.into_inner())
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|item| item.as_millis() as i64)
        .unwrap_or(0)
}

fn new_id() -> String {
    format!(
        "t-{}-{}",
        now_ms(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    )
}

fn drive_key(owner: &str, peer: &str) -> Result<DriveKey, String> {
    let owner = owner.trim().to_string();
    let peer = peer.trim().to_string();
    if owner.is_empty() || peer.is_empty() {
        return Err("token-empty".into());
    }
    Ok(DriveKey { owner, peer })
}

fn tasks_path(key: &DriveKey) -> Result<PathBuf, String> {
    let dir = storage::app_data_subdir("drives")?
        .join("tasks")
        .join(storage::sanitize_component(&key.owner));
    std::fs::create_dir_all(&dir).map_err(|err| format!("创建任务目录失败: {err}"))?;
    Ok(dir.join(format!("{}.json", storage::sanitize_component(&key.peer))))
}

fn load_disk(key: &DriveKey) -> Vec<NasTask> {
    let Ok(path) = tasks_path(key) else {
        return Vec::new();
    };
    if !path.exists() {
        return Vec::new();
    }
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    if raw.trim().is_empty() {
        return Vec::new();
    }
    let Ok(mut file) = serde_json::from_str::<TaskFile>(&raw) else {
        return Vec::new();
    };
    let now = now_ms();
    for task in &mut file.tasks {
        if task.status == "queued" || task.status == "running" || task.status == "interrupted" {
            task.status = "failed".into();
            if task.error.is_empty() || task.error == "已中断" {
                task.error = FAIL_CLOSED.into();
            }
            task.updated_at = now;
        }
    }
    let _ = save_disk(key, &file.tasks);
    file.tasks
}

fn save_disk(key: &DriveKey, tasks: &[NasTask]) -> Result<(), String> {
    let path = tasks_path(key)?;
    let text = serde_json::to_string_pretty(&TaskFile {
        tasks: tasks.to_vec(),
    })
    .map_err(|err| format!("序列化任务失败: {err}"))?;
    std::fs::write(&path, text).map_err(|err| format!("写入任务失败: {err}"))
}

fn cached_tasks(key: &DriveKey) -> Vec<NasTask> {
    let mut g = lock();
    if !g.files.contains_key(key) {
        let loaded = load_disk(key);
        g.files.insert(key.clone(), loaded);
    }
    g.files.get(key).cloned().unwrap_or_default()
}

fn with_tasks<F, R>(key: &DriveKey, f: F) -> R
where
    F: FnOnce(&mut Vec<NasTask>) -> R,
{
    let mut g = lock();
    if !g.files.contains_key(key) {
        let loaded = load_disk(key);
        g.files.insert(key.clone(), loaded);
    }
    let result = {
        let list = g.files.entry(key.clone()).or_default();
        f(list)
    };
    let snapshot = g.files.get(key).cloned().unwrap_or_default();
    drop(g);
    let _ = save_disk(key, &snapshot);
    emit_tasks(&key.peer, snapshot);
    result
}

fn emit_tasks(peer: &str, tasks: Vec<NasTask>) {
    if let Some(app) = webrpc::app_handle() {
        let _ = app.emit(
            "webrpc-nas-task",
            NasTaskEvent {
                peer_token: peer.to_string(),
                tasks,
            },
        );
    }
}

fn bind_session(key: DriveKey, session_id: u32) {
    if session_id == 0 {
        return;
    }
    let mut g = lock();
    let stale: Vec<u32> = g
        .sessions
        .iter()
        .filter(|(sid, item)| *item == &key && **sid != session_id)
        .map(|(sid, _)| *sid)
        .collect();
    for sid in stale {
        if let Some(flag) = g.cancelled.get(&sid) {
            flag.store(true, Ordering::SeqCst);
        }
        g.cancelled.remove(&sid);
        g.sessions.remove(&sid);
        g.busy.remove(&sid);
        g.uploads.remove(&sid);
        g.downloads.remove(&sid);
    }
    g.sessions.insert(session_id, key);
    g.cancelled
        .entry(session_id)
        .or_insert_with(|| Arc::new(AtomicBool::new(false)));
}

fn session_active(session_id: u32) -> bool {
    if session_id == 0 {
        return false;
    }
    let g = lock();
    if !g.sessions.contains_key(&session_id) {
        return false;
    }
    !g.cancelled
        .get(&session_id)
        .map(|flag| flag.load(Ordering::SeqCst))
        .unwrap_or(true)
}

fn transfer_alive(session_id: u32) -> bool {
    session_active(session_id) && nas::is_drive_session(session_id)
}

fn pump_gen(key: &DriveKey) -> u64 {
    lock().gens.get(key).copied().unwrap_or(0)
}

fn fail_open_tasks(key: &DriveKey, error: &str) {
    with_tasks(key, |list| {
        for task in list {
            if task.status == "queued" || task.status == "running" || task.status == "interrupted" {
                task.status = "failed".into();
                task.error = error.into();
                task.updated_at = now_ms();
            }
        }
    });
}

fn kick(key: DriveKey) {
    let gen = {
        let mut g = lock();
        if g.pumping.contains(&key) {
            return;
        }
        let slot = g.gens.entry(key.clone()).or_insert(0);
        *slot = slot.saturating_add(1);
        let gen = *slot;
        g.pumping.insert(key.clone());
        gen
    };
    let _ = thread::Builder::new()
        .name(format!("nas-task-{gen}"))
        .spawn(move || {
            loop {
                if pump_gen(&key) != gen {
                    return;
                }
                let Some((session_id, task)) = take_or_stop(&key, gen) else {
                    return;
                };
                if pump_gen(&key) != gen || !transfer_alive(session_id) {
                    return;
                }
                run_task(&key, session_id, task);
            }
        });
}

fn take_or_stop(key: &DriveKey, gen: u64) -> Option<(u32, NasTask)> {
    let mut g = lock();
    if g.gens.get(key).copied() != Some(gen) {
        return None;
    }
    let session_id = g
        .sessions
        .iter()
        .find_map(|(sid, item)| if item == key { Some(*sid) } else { None })
        .unwrap_or(0);
    if session_id == 0
        || g.cancelled
            .get(&session_id)
            .map(|flag| flag.load(Ordering::SeqCst))
            .unwrap_or(true)
    {
        g.pumping.remove(key);
        return None;
    }
    let Some(list) = g.files.get_mut(key) else {
        g.pumping.remove(key);
        return None;
    };
    if list.iter().any(|item| item.status == "running") {
        return None;
    }
    let Some(task) = list.iter_mut().find(|item| item.status == "queued") else {
        g.pumping.remove(key);
        return None;
    };
    task.status = "running".into();
    task.error.clear();
    task.transferred = 0;
    task.elapsed_ms = 0;
    task.speed_bps = 0;
    task.updated_at = now_ms();
    let task = task.clone();
    let snapshot = list.clone();
    drop(g);
    let _ = save_disk(key, &snapshot);
    emit_tasks(&key.peer, snapshot);
    Some((session_id, task))
}

fn update_task(key: &DriveKey, task_id: &str, mut edit: impl FnMut(&mut NasTask)) {
    with_tasks(key, |list| {
        if let Some(task) = list.iter_mut().find(|item| item.id == task_id) {
            edit(task);
            task.updated_at = now_ms();
        }
    });
}

fn finish_task(key: &DriveKey, task_id: &str, status: &str, error: &str, transferred: u64, started: Instant) {
    let elapsed = started.elapsed().as_millis().max(1) as u64;
    update_task(key, task_id, |task| {
        if task.status != "running" {
            return;
        }
        task.status = status.into();
        task.error = error.into();
        if status == "done" {
            if task.size < transferred {
                task.size = transferred;
            }
            task.transferred = task.size.max(transferred);
            task.has_size = true;
            task.speed_bps = if elapsed > 0 {
                task.transferred.saturating_mul(1000) / elapsed
            } else {
                0
            };
        } else if transferred > 0 {
            task.transferred = transferred;
            task.speed_bps = if elapsed > 0 {
                transferred.saturating_mul(1000) / elapsed
            } else {
                0
            };
        }
        task.elapsed_ms = elapsed;
    });
}

fn progress(key: &DriveKey, task_id: &str, transferred: u64, started: Instant, size: u64) {
    let elapsed = started.elapsed().as_millis().max(1) as u64;
    let snapshot = {
        let mut g = lock();
        let Some(list) = g.files.get_mut(key) else {
            return;
        };
        let Some(task) = list.iter_mut().find(|item| item.id == task_id) else {
            return;
        };
        if task.status != "running" {
            return;
        }
        task.transferred = transferred.min(if size > 0 { size } else { transferred });
        task.size = if size > 0 { size } else { task.size };
        task.elapsed_ms = elapsed;
        task.speed_bps = transferred.saturating_mul(1000) / elapsed;
        task.updated_at = now_ms();
        list.clone()
    };
    emit_tasks(&key.peer, snapshot);
}

fn wait_preview_idle(session_id: u32) {
    let deadline = Instant::now() + Duration::from_secs(90);
    while Instant::now() < deadline {
        if !transfer_alive(session_id) {
            return;
        }
        if !nas::recv_inflight(session_id) {
            return;
        }
        thread::sleep(Duration::from_millis(200));
    }
}

fn run_task(key: &DriveKey, session_id: u32, task: NasTask) {
    if !transfer_alive(session_id) {
        finish_task(key, &task.id, "failed", FAIL_CLOSED, 0, Instant::now());
        return;
    }
    {
        let mut g = lock();
        g.busy.insert(session_id);
    }
    wait_preview_idle(session_id);
    if !transfer_alive(session_id) {
        let mut g = lock();
        g.busy.remove(&session_id);
        drop(g);
        finish_task(key, &task.id, "failed", FAIL_CLOSED, 0, Instant::now());
        return;
    }
    let started = Instant::now();
    let result = if task.kind == "upload" {
        run_upload(key, session_id, &task, started)
    } else {
        run_download(key, session_id, &task, started)
    };
    {
        let mut g = lock();
        g.busy.remove(&session_id);
        g.uploads.remove(&session_id);
        g.downloads.remove(&session_id);
    }
    nas::clear_task_pending(session_id);
    if !transfer_alive(session_id) {
        finish_task(key, &task.id, "failed", FAIL_CLOSED, 0, started);
        return;
    }
    match result {
        Ok(bytes) => finish_task(key, &task.id, "done", "", bytes, started),
        Err(err) => {
            let msg = if err == "已中断" { FAIL_CLOSED } else { &err };
            finish_task(key, &task.id, "failed", msg, 0, started);
        }
    }
}

fn run_upload(
    key: &DriveKey,
    session_id: u32,
    task: &NasTask,
    started: Instant,
) -> Result<u64, String> {
    if !transfer_alive(session_id) {
        return Err(FAIL_CLOSED.into());
    }
    let src = PathBuf::from(task.local_path.trim());
    if !src.is_file() {
        return Err("本地文件不存在".into());
    }
    let name = storage::safe_filename(&task.name);
    let dir = nas::normalize_path(&task.nas_path);
    let size = std::fs::metadata(&src)
        .map(|meta| meta.len())
        .unwrap_or(0);
    update_task(key, &task.id, |item| {
        item.size = size;
        item.has_size = true;
        item.name = name.clone();
        item.nas_path = dir.clone();
    });
    let out = storage::drive_out_file(session_id, &dir, &name)?;
    std::fs::copy(&src, &out).map_err(|err| format!("准备上传失败: {err}"))?;
    let payload = serde_json::json!({
        "type": "putFile",
        "fileName": name,
        "filePath": dir,
        "size": size,
    })
    .to_string();
    if !webrpc::send_json_timeout(session_id, &payload, GET_FILE_TIMEOUT_MS) {
        return Err("上传请求超时".into());
    }
    if !transfer_alive(session_id) {
        return Err(FAIL_CLOSED.into());
    }
    if size == 0 {
        thread::sleep(Duration::from_millis(80));
        return Ok(0);
    }
    {
        let mut g = lock();
        g.uploads.insert(
            session_id,
            LiveXfer {
                task_id: task.id.clone(),
                started,
                size,
                bytes: 0,
            },
        );
    }
    spawn_live_ticker(session_id);
    thread::sleep(Duration::from_millis(80));
    if !transfer_alive(session_id) {
        return Err(FAIL_CLOSED.into());
    }
    let ok = webrpc::send_local_file(session_id, &out.to_string_lossy()) == 1;
    if !transfer_alive(session_id) {
        return Err(FAIL_CLOSED.into());
    }
    if !ok {
        return Err("上传失败".into());
    }
    Ok(size)
}

fn run_download(
    key: &DriveKey,
    session_id: u32,
    task: &NasTask,
    started: Instant,
) -> Result<u64, String> {
    if !transfer_alive(session_id) {
        return Err(FAIL_CLOSED.into());
    }
    let name = storage::safe_filename(&task.name);
    let dir = nas::normalize_path(&task.nas_path);
    let dest_dir = PathBuf::from(task.dest_dir.trim());
    if dest_dir.as_os_str().is_empty() {
        return Err("未选择保存目录".into());
    }
    std::fs::create_dir_all(&dest_dir).map_err(|err| format!("创建保存目录失败: {err}"))?;
    let dest = dest_dir.join(&name);
    let dest_s = dest.to_string_lossy().into_owned();
    update_task(key, &task.id, |item| {
        item.name = name.clone();
        item.nas_path = dir.clone();
        item.local_path = dest_s.clone();
    });
    if task.has_size && task.size == 0 {
        std::fs::write(&dest, []).map_err(|err| format!("保存失败: {err}"))?;
        return Ok(0);
    }
    let _ = std::fs::remove_file(&dest);
    {
        let mut g = lock();
        g.downloads.insert(
            session_id,
            LiveXfer {
                task_id: task.id.clone(),
                started,
                size: task.size,
                bytes: 0,
            },
        );
    }
    spawn_live_ticker(session_id);
    nas::arm_recv(
        session_id,
        name.clone(),
        dir.clone(),
        if task.has_size { Some(task.size) } else { None },
        Some(dest.clone()),
        true,
    );
    let payload = serde_json::json!({
        "type": "getFile",
        "fileName": name,
        "filePath": dir,
    })
    .to_string();
    eprintln!("webrpc: task getFile session={session_id} path={dir} name={name}");
    if !transfer_alive(session_id) {
        return Err(FAIL_CLOSED.into());
    }
    if !webrpc::send_json_timeout(session_id, &payload, GET_FILE_TIMEOUT_MS) {
        return Err("下载请求超时".into());
    }
    if !transfer_alive(session_id) {
        return Err(FAIL_CLOSED.into());
    }
    if wait_download(&dest, if task.has_size { task.size } else { 0 }, session_id, key, &task.id, started) {
        if !transfer_alive(session_id) {
            return Err(FAIL_CLOSED.into());
        }
        let bytes = std::fs::metadata(&dest).map(|meta| meta.len()).unwrap_or(task.size);
        return Ok(bytes);
    }
    if !transfer_alive(session_id) {
        return Err(FAIL_CLOSED.into());
    }
    Err("下载失败".into())
}

fn wait_download(
    dest: &Path,
    expected: u64,
    session_id: u32,
    key: &DriveKey,
    task_id: &str,
    started: Instant,
) -> bool {
    let mut last_len = 0u64;
    let mut last_change = Instant::now();
    let start = Instant::now();
    loop {
        if !transfer_alive(session_id) {
            return false;
        }
        if dest.is_file() {
            if let Ok(meta) = std::fs::metadata(dest) {
                let len = meta.len();
                if len != last_len {
                    last_len = len;
                    last_change = Instant::now();
                    {
                        let mut g = lock();
                        if let Some(live) = g.downloads.get_mut(&session_id) {
                            live.bytes = live.bytes.max(len);
                        }
                    }
                    progress(key, task_id, len, started, expected);
                }
                if expected > 0 && len == expected {
                    return true;
                }
            }
        }
        let idle = last_change.elapsed();
        if last_len == 0 && start.elapsed() > IDLE_START_WAIT {
            return false;
        }
        if last_len > 0 && idle > STALL_WAIT {
            return expected == 0 || last_len == expected;
        }
        thread::sleep(Duration::from_millis(200));
    }
}

fn spawn_live_ticker(session_id: u32) {
    let _ = thread::Builder::new()
        .name(format!("nas-task-tick-{session_id}"))
        .spawn(move || loop {
            thread::sleep(Duration::from_millis(200));
            if !transfer_alive(session_id) {
                return;
            }
            let found = {
                let g = lock();
                let live = g
                    .uploads
                    .get(&session_id)
                    .cloned()
                    .or_else(|| g.downloads.get(&session_id).cloned());
                let key = g.sessions.get(&session_id).cloned();
                (key, live)
            };
            let (Some(key), Some(live)) = found else {
                return;
            };
            progress(&key, &live.task_id, live.bytes, live.started, live.size);
        });
}

pub fn is_session_busy(session_id: u32) -> bool {
    if session_id == 0 {
        return false;
    }
    lock().busy.contains(&session_id)
}

pub fn on_upload_ack(session_id: u32, _file_name: &str, bytes: u64) {
    if !session_active(session_id) {
        return;
    }
    let (key, live) = {
        let mut g = lock();
        if let Some(live) = g.uploads.get_mut(&session_id) {
            live.bytes = live.bytes.max(bytes);
        }
        let live = g.uploads.get(&session_id).cloned();
        let key = g.sessions.get(&session_id).cloned();
        (key, live)
    };
    let Some(key) = key else {
        return;
    };
    let Some(live) = live else {
        return;
    };
    progress(&key, &live.task_id, live.bytes, live.started, live.size);
}

pub fn on_download_bytes(session_id: u32, bytes: u64) {
    if !session_active(session_id) {
        return;
    }
    let (key, live) = {
        let mut g = lock();
        if let Some(live) = g.downloads.get_mut(&session_id) {
            live.bytes = live.bytes.max(bytes);
        }
        let live = g.downloads.get(&session_id).cloned();
        let key = g.sessions.get(&session_id).cloned();
        (key, live)
    };
    let Some(key) = key else {
        return;
    };
    let Some(live) = live else {
        return;
    };
    progress(&key, &live.task_id, live.bytes, live.started, live.size);
}

pub fn on_session_dead(session_id: u32) {
    if session_id == 0 {
        return;
    }
    let key = {
        let mut g = lock();
        if let Some(flag) = g.cancelled.get(&session_id) {
            flag.store(true, Ordering::SeqCst);
        }
        let key = g.sessions.get(&session_id).cloned();
        if let Some(ref key) = key {
            let slot = g.gens.entry(key.clone()).or_insert(0);
            *slot = slot.saturating_add(1);
            g.pumping.remove(key);
        }
        g.busy.remove(&session_id);
        g.uploads.remove(&session_id);
        g.downloads.remove(&session_id);
        g.cancelled.remove(&session_id);
        g.sessions.remove(&session_id);
        key
    };
    nas::clear_task_pending(session_id);
    if let Some(key) = key {
        fail_open_tasks(&key, FAIL_CLOSED);
        eprintln!("webrpc: nas tasks recycled session={session_id}");
    }
}

pub fn interrupt_all() {
    let keys = {
        let mut g = lock();
        for flag in g.cancelled.values() {
            flag.store(true, Ordering::SeqCst);
        }
        for gen in g.gens.values_mut() {
            *gen = gen.saturating_add(1);
        }
        g.pumping.clear();
        g.busy.clear();
        g.uploads.clear();
        g.downloads.clear();
        g.sessions.clear();
        g.cancelled.clear();
        g.files.keys().cloned().collect::<Vec<_>>()
    };
    for key in keys {
        fail_open_tasks(&key, FAIL_CLOSED);
    }
    eprintln!("webrpc: nas tasks recycled all");
}

#[tauri::command]
pub fn nas_task_list(owner_token: String, peer_token: String) -> Result<Vec<NasTask>, String> {
    let key = drive_key(&owner_token, &peer_token)?;
    Ok(cached_tasks(&key))
}

#[tauri::command]
pub fn nas_task_bind(owner_token: String, peer_token: String, session_id: u32) -> Result<Vec<NasTask>, String> {
    let key = drive_key(&owner_token, &peer_token)?;
    bind_session(key.clone(), session_id);
    let list = cached_tasks(&key);
    kick(key);
    Ok(list)
}

#[tauri::command]
pub fn nas_task_upload(
    owner_token: String,
    peer_token: String,
    session_id: u32,
    local_path: String,
    nas_path: String,
) -> Result<Vec<NasTask>, String> {
    nas_task_upload_many(
        owner_token,
        peer_token,
        session_id,
        vec![NasUploadItem {
            local_path,
            nas_path,
        }],
    )
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NasUploadItem {
    pub local_path: String,
    pub nas_path: String,
}

fn make_upload_task(local_path: &str, nas_path: &str) -> Result<NasTask, String> {
    let src = PathBuf::from(local_path.trim());
    if crate::storage::is_dragout_path(&src) {
        return Err("dragout-dummy".into());
    }
    let meta = std::fs::metadata(&src).map_err(|err| format!("读取文件失败: {err}"))?;
    if !meta.is_file() {
        return Err("not-a-file".into());
    }
    let name = src
        .file_name()
        .and_then(|item| item.to_str())
        .map(storage::safe_filename)
        .filter(|item| !item.is_empty())
        .ok_or_else(|| "file-name-empty".to_string())?;
    let now = now_ms();
    Ok(NasTask {
        id: new_id(),
        kind: "upload".into(),
        status: "queued".into(),
        name,
        nas_path: nas::normalize_path(nas_path),
        size: meta.len(),
        has_size: true,
        transferred: 0,
        elapsed_ms: 0,
        speed_bps: 0,
        local_path: src.to_string_lossy().into_owned(),
        dest_dir: String::new(),
        error: String::new(),
        created_at: now,
        updated_at: now,
    })
}

#[tauri::command]
pub fn nas_task_upload_many(
    owner_token: String,
    peer_token: String,
    session_id: u32,
    items: Vec<NasUploadItem>,
) -> Result<Vec<NasTask>, String> {
    let key = drive_key(&owner_token, &peer_token)?;
    if session_id == 0 {
        return Err("not-connected".into());
    }
    let mut queued = Vec::new();
    for item in items {
        queued.push(make_upload_task(&item.local_path, &item.nas_path)?);
    }
    if queued.is_empty() {
        return Ok(cached_tasks(&key));
    }
    bind_session(key.clone(), session_id);
    with_tasks(&key, |list| list.extend(queued));
    kick(key);
    Ok(cached_tasks(&drive_key(&owner_token, &peer_token)?))
}

#[tauri::command]
pub fn nas_task_download(
    owner_token: String,
    peer_token: String,
    session_id: u32,
    file_name: String,
    nas_path: String,
    size: Option<u64>,
    dest_dir: String,
) -> Result<Vec<NasTask>, String> {
    let key = drive_key(&owner_token, &peer_token)?;
    if session_id == 0 {
        return Err("not-connected".into());
    }
    let name = storage::safe_filename(&file_name);
    if name.is_empty() {
        return Err("file-name-empty".into());
    }
    let dest_dir = dest_dir.trim().to_string();
    if dest_dir.is_empty() {
        return Err("dest-empty".into());
    }
    let dest = PathBuf::from(&dest_dir).join(&name);
    let now = now_ms();
    let task = NasTask {
        id: new_id(),
        kind: "download".into(),
        status: "queued".into(),
        name,
        nas_path: nas::normalize_path(&nas_path),
        size: size.unwrap_or(0),
        has_size: size.is_some(),
        transferred: 0,
        elapsed_ms: 0,
        speed_bps: 0,
        local_path: dest.to_string_lossy().into_owned(),
        dest_dir,
        error: String::new(),
        created_at: now,
        updated_at: now,
    };
    bind_session(key.clone(), session_id);
    with_tasks(&key, |list| list.push(task));
    kick(key);
    Ok(cached_tasks(&drive_key(&owner_token, &peer_token)?))
}

#[tauri::command]
pub fn nas_task_retry(
    owner_token: String,
    peer_token: String,
    session_id: u32,
    task_id: String,
) -> Result<Vec<NasTask>, String> {
    let key = drive_key(&owner_token, &peer_token)?;
    if session_id == 0 {
        return Err("not-connected".into());
    }
    bind_session(key.clone(), session_id);
    let ok = with_tasks(&key, |list| {
        let Some(task) = list.iter_mut().find(|item| item.id == task_id) else {
            return false;
        };
        if task.status != "failed" && task.status != "interrupted" {
            return false;
        }
        task.status = "queued".into();
        task.error.clear();
        task.transferred = 0;
        task.elapsed_ms = 0;
        task.speed_bps = 0;
        task.updated_at = now_ms();
        true
    });
    if !ok {
        return Err("task-missing".into());
    }
    kick(key);
    Ok(cached_tasks(&drive_key(&owner_token, &peer_token)?))
}

#[tauri::command]
pub fn nas_task_delete(
    owner_token: String,
    peer_token: String,
    task_id: String,
) -> Result<Vec<NasTask>, String> {
    let key = drive_key(&owner_token, &peer_token)?;
    with_tasks(&key, |list| {
        list.retain(|item| item.id != task_id || item.status == "running");
    });
    Ok(cached_tasks(&key))
}

#[tauri::command]
pub fn nas_task_clear(owner_token: String, peer_token: String) -> Result<Vec<NasTask>, String> {
    let key = drive_key(&owner_token, &peer_token)?;
    with_tasks(&key, |list| {
        list.retain(|item| item.status == "queued" || item.status == "running");
    });
    Ok(cached_tasks(&key))
}

#[tauri::command]
pub fn nas_task_wipe(owner_token: String, peer_token: String) -> Result<(), String> {
    let key = drive_key(&owner_token, &peer_token)?;
    {
        let mut g = lock();
        g.files.remove(&key);
        g.sessions.retain(|_, item| item != &key);
        g.pumping.remove(&key);
    }
    if let Ok(path) = tasks_path(&key) {
        let _ = std::fs::remove_file(path);
    }
    Ok(())
}
