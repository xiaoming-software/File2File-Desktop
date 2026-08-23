use std::cmp::Ordering;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::Emitter;

use crate::storage;
use crate::webrpc;

const NAS_POLL: Duration = Duration::from_secs(5);
const NAS_SEND_TIMEOUT_MS: i64 = 5_000;
const GET_PATH_TIMEOUT_MS: i64 = 10_000;
const GET_FILE_TIMEOUT_MS: i64 = 10_000;
const DELETE_TIMEOUT_MS: i64 = 60_000;
const SEARCH_TIMEOUT_MS: i64 = 30_000;
const GET_NAS_INFO: &str = "{\"type\":\"getNasInfo\"}";
const NAS_FILE_WAIT: Duration = Duration::from_secs(90);

#[derive(Clone)]
struct PendingFile {
    file_name: String,
    file_path: String,
    size: Option<u64>,
    epoch: u64,
    dest: Option<PathBuf>,
    task: bool,
}

#[derive(Default)]
struct NasWatches {
    gens: HashMap<u32, u64>,
    joins: HashMap<u32, JoinHandle<()>>,
    drive_ids: HashSet<u32>,
    pending: HashMap<u32, PendingFile>,
    list_paths: HashMap<u32, VecDeque<String>>,
}

static WATCHES: OnceLock<Mutex<NasWatches>> = OnceLock::new();

fn lock() -> MutexGuard<'static, NasWatches> {
    WATCHES
        .get_or_init(|| Mutex::new(NasWatches::default()))
        .lock()
        .unwrap_or_else(|err| err.into_inner())
}

fn gen_of(session_id: u32) -> u64 {
    lock().gens.get(&session_id).copied().unwrap_or(0)
}

pub fn is_drive_session(session_id: u32) -> bool {
    if session_id == 0 {
        return false;
    }
    let g = lock();
    g.drive_ids.contains(&session_id)
        || g.joins.contains_key(&session_id)
        || g.pending.contains_key(&session_id)
}

pub fn recv_file_path(session_id: u32, stream_name: &str) -> Option<std::path::PathBuf> {
    if session_id == 0 {
        return None;
    }
    let pending = {
        let g = lock();
        if !g.drive_ids.contains(&session_id)
            && !g.joins.contains_key(&session_id)
            && !g.pending.contains_key(&session_id)
        {
            return None;
        }
        g.pending.get(&session_id).cloned()
    };
    if let Some(item) = pending {
        if let Some(dest) = item.dest {
            Some(dest)
        } else {
            storage::drive_temp_file(session_id, &item.file_path, &item.file_name).ok()
        }
    } else {
        storage::drive_temp_file(session_id, "/", &storage::safe_filename(stream_name)).ok()
    }
}

pub fn recv_inflight(session_id: u32) -> bool {
    if session_id == 0 {
        return false;
    }
    let pending = lock().pending.get(&session_id).cloned();
    let Some(item) = pending else {
        return false;
    };
    if item.task {
        return false;
    }
    let dest = item.dest.clone().or_else(|| {
        storage::drive_temp_file(session_id, &item.file_path, &item.file_name).ok()
    });
    let Some(dest) = dest else {
        return true;
    };
    !cache_ready(&dest, item.size)
}

pub fn arm_recv(
    session_id: u32,
    file_name: String,
    file_path: String,
    size: Option<u64>,
    dest: Option<PathBuf>,
    task: bool,
) -> u64 {
    let mut g = lock();
    let epoch = g.pending.get(&session_id).map(|item| item.epoch).unwrap_or(0) + 1;
    g.pending.insert(
        session_id,
        PendingFile {
            file_name,
            file_path,
            size,
            epoch,
            dest,
            task,
        },
    );
    epoch
}

pub fn clear_task_pending(session_id: u32) {
    if session_id == 0 {
        return;
    }
    let mut g = lock();
    if g.pending.get(&session_id).map(|item| item.task).unwrap_or(false) {
        g.pending.remove(&session_id);
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NasFileEvent {
    pub session_id: u32,
    pub file_name: String,
    pub file_path: String,
    pub local_path: String,
    pub ok: bool,
}

pub fn emit_back_file(session_id: u32, file_name: &str, file_path: &str) {
    if session_id == 0 {
        return;
    }
    let name = storage::safe_filename(file_name);
    let dir = normalize_path(file_path);
    let pending = lock().pending.get(&session_id).cloned();
    if pending.as_ref().map(|item| item.task).unwrap_or(false) {
        return;
    }
    let (dest, expected, epoch, emit_name, emit_dir) = if let Some(item) = pending {
        (
            item.dest
                .clone()
                .or_else(|| storage::drive_temp_file(session_id, &item.file_path, &item.file_name).ok()),
            item.size,
            item.epoch,
            item.file_name,
            item.file_path,
        )
    } else {
        (
            storage::drive_temp_file(session_id, &dir, &name).ok(),
            None,
            0,
            name,
            dir,
        )
    };
    let Some(dest) = dest else {
        emit_nas_file_event(session_id, emit_name, emit_dir, String::new(), false);
        return;
    };
    thread::spawn(move || {
        let ok = wait_for_nas_cache(&dest, expected, NAS_FILE_WAIT);
        let current = lock().pending.get(&session_id).map(|item| item.epoch);
        if epoch != 0 && current != Some(epoch) {
            return;
        }
        let local = dest.to_string_lossy().into_owned();
        let ok = ok && dest.is_file();
        eprintln!(
            "webrpc: backFile session={session_id} path={emit_dir} name={emit_name} ok={ok} dest={}",
            dest.display()
        );
        emit_nas_file_event(session_id, emit_name, emit_dir, local, ok);
    });
}

fn emit_nas_file_event(
    session_id: u32,
    file_name: String,
    file_path: String,
    local_path: String,
    ok: bool,
) {
    if let Some(app) = webrpc::app_handle() {
        let _ = app.emit(
            "webrpc-nas-file",
            NasFileEvent {
                session_id,
                file_name,
                file_path,
                local_path,
                ok,
            },
        );
    }
}

fn wait_for_nas_cache(path: &std::path::Path, expected: Option<u64>, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    let mut last_len: Option<u64> = None;
    let mut stable = 0u32;
    while Instant::now() < deadline {
        if cache_ready(path, expected) {
            return true;
        }
        if expected.is_none() {
            if let Ok(meta) = std::fs::metadata(path) {
                if meta.is_file() && meta.len() > 0 {
                    if last_len == Some(meta.len()) {
                        stable = stable.saturating_add(1);
                        if stable >= 4 {
                            return true;
                        }
                    } else {
                        stable = 0;
                        last_len = Some(meta.len());
                    }
                }
            }
        }
        thread::sleep(Duration::from_millis(100));
    }
    if expected == Some(0) {
        return path.is_file()
            && std::fs::metadata(path)
                .map(|meta| meta.len() == 0)
                .unwrap_or(false);
    }
    path.is_file()
        && std::fs::metadata(path)
            .map(|meta| meta.len() > 0)
            .unwrap_or(false)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NasGetFileResult {
    pub cached: bool,
    pub local_path: String,
    pub busy: bool,
}

#[tauri::command]
pub fn nas_get_file(
    session_id: u32,
    file_name: String,
    file_path: String,
    size: Option<u64>,
) -> Option<NasGetFileResult> {
    if session_id == 0 {
        return None;
    }
    if crate::tasks::is_session_busy(session_id) {
        return Some(NasGetFileResult {
            cached: false,
            local_path: String::new(),
            busy: true,
        });
    }
    let name = storage::safe_filename(&file_name);
    let dir = normalize_path(&file_path);
    if name.is_empty() {
        return None;
    }
    let dest = match storage::drive_temp_file(session_id, &dir, &name) {
        Ok(path) => path,
        Err(_) => return None,
    };
    let local_path = dest.to_string_lossy().into_owned();
    if size == Some(0) {
        if std::fs::write(&dest, []).is_ok() {
            eprintln!(
                "webrpc: nas preview empty file session={session_id} path={dir} name={name}"
            );
            return Some(NasGetFileResult {
                cached: true,
                local_path,
                busy: false,
            });
        }
        return None;
    }
    if cache_ready(&dest, size) {
        eprintln!(
            "webrpc: nas preview cache hit session={session_id} path={dir} name={name}"
        );
        return Some(NasGetFileResult {
            cached: true,
            local_path,
            busy: false,
        });
    }
    storage::delete_drive_temp_file(session_id, &dir, &name);
    arm_recv(
        session_id,
        name.clone(),
        dir.clone(),
        size,
        None,
        false,
    );
    thread::spawn(move || {
        let payload = serde_json::json!({
            "type": "getFile",
            "fileName": name,
            "filePath": dir,
        })
        .to_string();
        eprintln!("webrpc: getFile session={session_id} path={dir} name={name}");
        let ok = webrpc::send_json_timeout(session_id, &payload, GET_FILE_TIMEOUT_MS);
        if !ok {
            eprintln!("webrpc: getFile send timeout session={session_id}");
        }
    });
    Some(NasGetFileResult {
        cached: false,
        local_path,
        busy: false,
    })
}

fn cache_ready(path: &std::path::Path, expected: Option<u64>) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    match expected {
        Some(n) => meta.len() == n,
        None => meta.len() > 0,
    }
}

const NAS_TEXT_MAX: u64 = 100 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NasPutEvent {
    pub session_id: u32,
    pub file_name: String,
    pub file_path: String,
    pub ok: bool,
}

#[tauri::command]
pub fn nas_read_text(path: String) -> Result<String, String> {
    let path = std::path::PathBuf::from(path.trim());
    let meta = std::fs::metadata(&path).map_err(|err| format!("读取文件失败: {err}"))?;
    if !meta.is_file() {
        return Err("not-a-file".into());
    }
    if meta.len() > NAS_TEXT_MAX {
        return Err("too-large".into());
    }
    let bytes = std::fs::read(&path).map_err(|err| format!("读取文件失败: {err}"))?;
    if bytes.contains(&0) {
        return Err("not-text".into());
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

#[tauri::command]
pub fn nas_write_text(path: String, text: String) -> Result<(), String> {
    let path = std::path::PathBuf::from(path.trim());
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| format!("创建目录失败: {err}"))?;
    }
    std::fs::write(&path, text.as_bytes()).map_err(|err| format!("写入文件失败: {err}"))
}

#[tauri::command]
pub fn nas_put_file(
    session_id: u32,
    file_name: String,
    file_path: String,
    local_path: Option<String>,
) {
    if session_id == 0 {
        return;
    }
    if crate::tasks::is_session_busy(session_id) {
        let name = storage::safe_filename(&file_name);
        let dir = normalize_path(&file_path);
        emit_nas_put(session_id, name, dir, false);
        return;
    }
    let name = storage::safe_filename(&file_name);
    let dir = normalize_path(&file_path);
    if name.is_empty() {
        return;
    }
    let src = local_path
        .as_deref()
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(std::path::PathBuf::from)
        .or_else(|| storage::drive_temp_file(session_id, &dir, &name).ok());
    let Some(src) = src else {
        emit_nas_put(session_id, name, dir, false);
        return;
    };
    if !src.is_file() {
        emit_nas_put(session_id, name, dir, false);
        return;
    }
    let Ok(out) = storage::drive_out_file(session_id, &dir, &name) else {
        emit_nas_put(session_id, name, dir, false);
        return;
    };
    if std::fs::copy(&src, &out).is_err() {
        emit_nas_put(session_id, name, dir, false);
        return;
    }
    let file_size = std::fs::metadata(&out).map(|meta| meta.len()).unwrap_or(0);
    thread::spawn(move || {
        let payload = serde_json::json!({
            "type": "putFile",
            "fileName": name,
            "filePath": dir,
            "size": file_size,
        })
        .to_string();
        let announced = webrpc::send_json_timeout(session_id, &payload, GET_FILE_TIMEOUT_MS);
        if !announced {
            eprintln!("webrpc: putFile send timeout session={session_id} path={dir} name={name}");
            emit_nas_put(session_id, name, dir, false);
            return;
        }
        if file_size == 0 {
            thread::sleep(Duration::from_millis(80));
            eprintln!("webrpc: nas put empty session={session_id} path={dir} name={name}");
            emit_nas_put(session_id, name, dir, true);
            return;
        }
        thread::sleep(Duration::from_millis(80));
        let ok = webrpc::send_local_file(session_id, &out.to_string_lossy()) == 1;
        eprintln!(
            "webrpc: nas put session={session_id} path={dir} name={name} ok={ok}"
        );
        emit_nas_put(session_id, name, dir, ok);
    });
}

fn emit_nas_put(session_id: u32, file_name: String, file_path: String, ok: bool) {
    if let Some(app) = webrpc::app_handle() {
        let _ = app.emit(
            "webrpc-nas-put",
            NasPutEvent {
                session_id,
                file_name,
                file_path,
                ok,
            },
        );
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NasInfoEvent {
    pub session_id: u32,
    pub disk_size: u64,
    pub banlen_size: u64,
    pub file_num: u64,
}

pub fn emit_back_nas_info(session_id: u32, data: &serde_json::Value) {
    if session_id == 0 {
        return;
    }
    let disk_size = data.get("diskSize").and_then(|v| v.as_u64()).unwrap_or(0);
    let banlen_size = data.get("banlenSize").and_then(|v| v.as_u64()).unwrap_or(0);
    let file_num = data
        .get("fileNum")
        .and_then(|v| v.as_u64().or_else(|| v.as_i64().map(|n| n.max(0) as u64)))
        .unwrap_or(0);
    eprintln!(
        "webrpc: backNasInfo session={session_id} disk={disk_size} free={banlen_size} files={file_num}"
    );
    if let Some(app) = webrpc::app_handle() {
        let _ = app.emit(
            "webrpc-nas-info",
            NasInfoEvent {
                session_id,
                disk_size,
                banlen_size,
                file_num,
            },
        );
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NasEntry {
    pub name: String,
    pub is_dir: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_date: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NasPathEvent {
    pub session_id: u32,
    pub ok: bool,
    pub path: String,
    pub entries: Vec<NasEntry>,
}

fn push_list_path(session_id: u32, path: &str) {
    lock()
        .list_paths
        .entry(session_id)
        .or_default()
        .push_back(normalize_path(path));
}

fn pop_list_path_front(session_id: u32) -> String {
    lock()
        .list_paths
        .get_mut(&session_id)
        .and_then(|q| q.pop_front())
        .unwrap_or_else(|| "/".into())
}

fn pop_list_path_back(session_id: u32) -> String {
    lock()
        .list_paths
        .get_mut(&session_id)
        .and_then(|q| q.pop_back())
        .unwrap_or_else(|| "/".into())
}

fn resolve_list_path(session_id: u32, reported: &str) -> String {
    let queued = pop_list_path_front(session_id);
    let reported = reported.trim();
    if reported.is_empty() {
        queued
    } else {
        normalize_path(reported)
    }
}

pub fn emit_back_path(session_id: u32, data: &serde_json::Value, path: &str) {
    if session_id == 0 {
        return;
    }
    let mut entries = Vec::new();
    if let Some(obj) = data.as_object() {
        for (name, flag) in obj {
            if name.is_empty() || name.starts_with('.') || !flag.is_object() {
                continue;
            }
            entries.push(parse_path_item(name, flag));
        }
    }
    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });
    let dir = resolve_list_path(session_id, path);
    eprintln!(
        "webrpc: backPath session={session_id} path={dir} files={}",
        entries.len()
    );
    emit_path_event(session_id, true, entries, dir);
}

fn parse_path_item(name: &str, value: &serde_json::Value) -> NasEntry {
    let obj = value.as_object();
    let if_file = obj
        .and_then(|m| m.get("ifFile"))
        .map(json_is_true)
        .unwrap_or(false);
    let size = obj.and_then(|m| m.get("size")).and_then(json_u64);
    let last_date = obj.and_then(|m| m.get("lastDate")).and_then(json_i64);
    NasEntry {
        name: name.to_string(),
        is_dir: !if_file,
        size: if if_file { size } else { None },
        last_date,
    }
}

fn json_is_true(value: &serde_json::Value) -> bool {
    value.as_bool().unwrap_or(false) || value.as_i64() == Some(1)
}

fn json_u64(value: &serde_json::Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_i64().and_then(|n| if n >= 0 { Some(n as u64) } else { None }))
}

fn json_i64(value: &serde_json::Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|n| i64::try_from(n).ok()))
}

fn emit_path_event(session_id: u32, ok: bool, entries: Vec<NasEntry>, path: String) {
    if let Some(app) = webrpc::app_handle() {
        let _ = app.emit(
            "webrpc-nas-path",
            NasPathEvent {
                session_id,
                ok,
                path,
                entries,
            },
        );
    }
}

pub fn normalize_path(raw: &str) -> String {
    let cleaned = raw.replace('\\', "/");
    let mut parts: Vec<&str> = Vec::new();
    for part in cleaned.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            parts.pop();
            continue;
        }
        if part.starts_with('.') {
            continue;
        }
        parts.push(part);
    }
    if parts.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", parts.join("/"))
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NasCreateEvent {
    pub session_id: u32,
    pub ok: bool,
    pub name: String,
    pub path: String,
    pub error: String,
}

pub fn emit_back_create_file(session_id: u32, value: &serde_json::Value) {
    if session_id == 0 {
        return;
    }
    let ok = value.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
    let name = value
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let path = normalize_path(value.get("path").and_then(|v| v.as_str()).unwrap_or("/"));
    let error = value
        .get("error")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    eprintln!(
        "webrpc: backCreateFile session={session_id} path={path} name={name} ok={ok} error={error}"
    );
    if let Some(app) = webrpc::app_handle() {
        let _ = app.emit(
            "webrpc-nas-create",
            NasCreateEvent {
                session_id,
                ok,
                name,
                path,
                error,
            },
        );
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NasMoveItem {
    pub name: String,
    pub ok: bool,
    pub error: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NasMoveEvent {
    pub session_id: u32,
    pub ok: bool,
    pub name: String,
    pub path: String,
    pub target_path: String,
    pub error: String,
    pub results: Vec<NasMoveItem>,
}

fn parse_move_results(value: &serde_json::Value, name: &str, ok: bool, error: &str) -> Vec<NasMoveItem> {
    if let Some(arr) = value.get("results").and_then(|v| v.as_array()) {
        let items: Vec<NasMoveItem> = arr
            .iter()
            .filter_map(|item| {
                let item_name = item.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                if item_name.is_empty() && name.is_empty() {
                    return None;
                }
                Some(NasMoveItem {
                    name: if item_name.is_empty() {
                        name.to_string()
                    } else {
                        item_name
                    },
                    ok: item.get("ok").and_then(|v| v.as_bool()).unwrap_or(false),
                    error: item
                        .get("error")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                })
            })
            .collect();
        if !items.is_empty() {
            return items;
        }
    }
    if name.is_empty() {
        Vec::new()
    } else {
        vec![NasMoveItem {
            name: name.to_string(),
            ok,
            error: error.to_string(),
        }]
    }
}

fn clean_move_names(names: Vec<String>) -> Vec<String> {
    let mut out = Vec::new();
    for raw in names {
        let name = raw.trim().replace(['\\', '/', '\0'], "");
        if name.is_empty() || name == "." || name == ".." || name.starts_with('.') {
            continue;
        }
        if !out.iter().any(|item| item == &name) {
            out.push(name);
        }
    }
    out
}

pub fn emit_back_move_file(session_id: u32, value: &serde_json::Value) {
    if session_id == 0 {
        return;
    }
    let mut results = parse_move_results(
        value,
        value.get("name").and_then(|v| v.as_str()).unwrap_or(""),
        value.get("ok").and_then(|v| v.as_bool()).unwrap_or(false),
        value.get("error").and_then(|v| v.as_str()).unwrap_or(""),
    );
    let ok = value
        .get("ok")
        .and_then(|v| v.as_bool())
        .unwrap_or_else(|| !results.is_empty() && results.iter().all(|item| item.ok));
    let path = normalize_path(value.get("path").and_then(|v| v.as_str()).unwrap_or("/"));
    let target_path = normalize_path(
        value
            .get("targetPath")
            .and_then(|v| v.as_str())
            .unwrap_or("/"),
    );
    if results.is_empty() && !ok {
        results.push(NasMoveItem {
            name: String::new(),
            ok: false,
            error: value
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("failed")
                .to_string(),
        });
    }
    let name = results
        .first()
        .map(|item| item.name.clone())
        .unwrap_or_default();
    let error = results
        .iter()
        .find(|item| !item.ok)
        .map(|item| item.error.clone())
        .unwrap_or_default();
    eprintln!(
        "webrpc: backMoveFile session={session_id} from={path} to={target_path} count={} ok={ok} error={error}",
        results.len()
    );
    if let Some(app) = webrpc::app_handle() {
        let _ = app.emit(
            "webrpc-nas-move",
            NasMoveEvent {
                session_id,
                ok,
                name,
                path,
                target_path,
                error,
                results,
            },
        );
    }
}

#[tauri::command]
pub fn nas_move_entry(session_id: u32, names: Vec<String>, path: String, target_path: String) {
    if session_id == 0 {
        return;
    }
    let names = clean_move_names(names);
    let from = normalize_path(&path);
    let to = normalize_path(&target_path);
    if names.is_empty() {
        emit_back_move_file(
            session_id,
            &serde_json::json!({
                "ok": false,
                "path": from,
                "targetPath": to,
                "error": "invalid",
                "results": [{ "name": "", "ok": false, "error": "invalid" }],
            }),
        );
        return;
    }
    thread::spawn(move || {
        let payload = serde_json::json!({
            "type": "moveFiles",
            "fileNames": names,
            "filePath": from,
            "targetPath": to,
        })
        .to_string();
        eprintln!(
            "webrpc: moveFiles session={session_id} from={from} to={to} count={}",
            names.len()
        );
        let ok = webrpc::send_json_timeout(session_id, &payload, GET_PATH_TIMEOUT_MS);
        if !ok {
            eprintln!("webrpc: moveFiles timeout session={session_id}");
            let results: Vec<serde_json::Value> = names
                .iter()
                .map(|name| {
                    serde_json::json!({
                        "name": name,
                        "ok": false,
                        "error": "failed",
                    })
                })
                .collect();
            emit_back_move_file(
                session_id,
                &serde_json::json!({
                    "ok": false,
                    "path": from,
                    "targetPath": to,
                    "error": "failed",
                    "results": results,
                }),
            );
        }
    });
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NasDeleteEvent {
    pub session_id: u32,
    pub ok: bool,
    pub name: String,
    pub path: String,
    pub error: String,
    pub results: Vec<NasMoveItem>,
}

pub fn emit_back_delete_file(session_id: u32, value: &serde_json::Value) {
    if session_id == 0 {
        return;
    }
    let mut results = parse_move_results(
        value,
        value.get("name").and_then(|v| v.as_str()).unwrap_or(""),
        value.get("ok").and_then(|v| v.as_bool()).unwrap_or(false),
        value.get("error").and_then(|v| v.as_str()).unwrap_or(""),
    );
    let ok = value
        .get("ok")
        .and_then(|v| v.as_bool())
        .unwrap_or_else(|| !results.is_empty() && results.iter().all(|item| item.ok));
    let path = normalize_path(value.get("path").and_then(|v| v.as_str()).unwrap_or("/"));
    if results.is_empty() && !ok {
        results.push(NasMoveItem {
            name: String::new(),
            ok: false,
            error: value
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("failed")
                .to_string(),
        });
    }
    let name = results
        .first()
        .map(|item| item.name.clone())
        .unwrap_or_default();
    let error = results
        .iter()
        .find(|item| !item.ok)
        .map(|item| item.error.clone())
        .unwrap_or_default();
    eprintln!(
        "webrpc: backDeleteFile session={session_id} path={path} count={} ok={ok} error={error}",
        results.len()
    );
    if let Some(app) = webrpc::app_handle() {
        let _ = app.emit(
            "webrpc-nas-delete",
            NasDeleteEvent {
                session_id,
                ok,
                name,
                path,
                error,
                results,
            },
        );
    }
}

#[tauri::command]
pub fn nas_delete_entry(session_id: u32, names: Vec<String>, path: String) {
    if session_id == 0 {
        return;
    }
    let names = clean_move_names(names);
    let from = normalize_path(&path);
    if names.is_empty() {
        emit_back_delete_file(
            session_id,
            &serde_json::json!({
                "ok": false,
                "path": from,
                "error": "invalid",
                "results": [{ "name": "", "ok": false, "error": "invalid" }],
            }),
        );
        return;
    }
    thread::spawn(move || {
        let payload = serde_json::json!({
            "type": "deleteFile",
            "fileNames": names,
            "filePath": from,
        })
        .to_string();
        eprintln!(
            "webrpc: deleteFile session={session_id} path={from} count={}",
            names.len()
        );
        let ok = webrpc::send_json_timeout(session_id, &payload, DELETE_TIMEOUT_MS);
        if !ok {
            eprintln!("webrpc: deleteFile timeout session={session_id}");
            let results: Vec<serde_json::Value> = names
                .iter()
                .map(|name| {
                    serde_json::json!({
                        "name": name,
                        "ok": false,
                        "error": "failed",
                    })
                })
                .collect();
            emit_back_delete_file(
                session_id,
                &serde_json::json!({
                    "ok": false,
                    "path": from,
                    "error": "failed",
                    "results": results,
                }),
            );
        }
    });
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NasRenameEvent {
    pub session_id: u32,
    pub ok: bool,
    pub name: String,
    pub new_name: String,
    pub path: String,
    pub error: String,
}

pub fn emit_back_rename_file(session_id: u32, value: &serde_json::Value) {
    if session_id == 0 {
        return;
    }
    let ok = value.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
    let name = value
        .get("fileName")
        .and_then(|v| v.as_str())
        .or_else(|| value.get("name").and_then(|v| v.as_str()))
        .unwrap_or("")
        .to_string();
    let new_name = value
        .get("newFileName")
        .and_then(|v| v.as_str())
        .or_else(|| value.get("newName").and_then(|v| v.as_str()))
        .unwrap_or("")
        .to_string();
    let path = normalize_path(value.get("path").and_then(|v| v.as_str()).unwrap_or("/"));
    let error = value
        .get("error")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    eprintln!(
        "webrpc: backRenameFile session={session_id} path={path} from={name} to={new_name} ok={ok} error={error}"
    );
    if let Some(app) = webrpc::app_handle() {
        let _ = app.emit(
            "webrpc-nas-rename",
            NasRenameEvent {
                session_id,
                ok,
                name,
                new_name,
                path,
                error,
            },
        );
    }
}

fn clean_rename_name(raw: &str) -> String {
    raw.trim().replace(['\\', '/', '\0'], "")
}

#[tauri::command]
pub fn nas_rename_entry(session_id: u32, name: String, new_name: String, path: String) {
    if session_id == 0 {
        return;
    }
    let from = clean_rename_name(&name);
    let to = clean_rename_name(&new_name);
    let dir = normalize_path(&path);
    if from.is_empty()
        || to.is_empty()
        || from == "."
        || from == ".."
        || to == "."
        || to == ".."
        || from.starts_with('.')
        || to.starts_with('.')
    {
        emit_back_rename_file(
            session_id,
            &serde_json::json!({
                "ok": false,
                "fileName": from,
                "newFileName": to,
                "path": dir,
                "error": "invalid",
            }),
        );
        return;
    }
    thread::spawn(move || {
        let payload = serde_json::json!({
            "type": "renameFile",
            "fileName": from,
            "filePath": dir,
            "newFileName": to,
        })
        .to_string();
        eprintln!("webrpc: renameFile session={session_id} path={dir} from={from} to={to}");
        let ok = webrpc::send_json_timeout(session_id, &payload, GET_PATH_TIMEOUT_MS);
        if !ok {
            eprintln!("webrpc: renameFile timeout session={session_id}");
            emit_back_rename_file(
                session_id,
                &serde_json::json!({
                    "ok": false,
                    "fileName": from,
                    "newFileName": to,
                    "path": dir,
                    "error": "failed",
                }),
            );
        }
    });
}

#[tauri::command]
pub fn nas_create_entry(session_id: u32, name: String, path: String) {
    if session_id == 0 {
        return;
    }
    let name = name.trim().replace(['\\', '/', '\0'], "");
    let dir = normalize_path(&path);
    if name.is_empty() || name == "." || name == ".." || name.starts_with('.') {
        emit_back_create_file(
            session_id,
            &serde_json::json!({
                "ok": false,
                "name": name,
                "path": dir,
                "error": "invalid",
            }),
        );
        return;
    }
    thread::spawn(move || {
        let payload = serde_json::json!({
            "type": "createFile",
            "data": {
                "name": name,
                "path": dir,
            },
        })
        .to_string();
        eprintln!("webrpc: createFile session={session_id} path={dir} name={name}");
        let ok = webrpc::send_json_timeout(session_id, &payload, GET_PATH_TIMEOUT_MS);
        if !ok {
            eprintln!("webrpc: createFile timeout session={session_id}");
            emit_back_create_file(
                session_id,
                &serde_json::json!({
                    "ok": false,
                    "name": name,
                    "path": dir,
                    "error": "failed",
                }),
            );
        }
    });
}

fn get_path_payload(path: &str) -> String {
    serde_json::json!({
        "type": "getPath",
        "data": normalize_path(path),
    })
    .to_string()
}

fn request_path(session_id: u32, path: &str) {
    let dir = normalize_path(path);
    push_list_path(session_id, &dir);
    let payload = get_path_payload(&dir);
    eprintln!("webrpc: getPath session={session_id} path={dir}");
    let ok = webrpc::send_json_timeout(session_id, &payload, GET_PATH_TIMEOUT_MS);
    if !ok {
        eprintln!("webrpc: getPath timeout session={session_id}");
        let reported = pop_list_path_back(session_id);
        emit_path_event(session_id, false, Vec::new(), reported);
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NasSearchHit {
    pub name: String,
    pub file_path: String,
    pub is_dir: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NasSearchEvent {
    pub session_id: u32,
    pub ok: bool,
    pub keyword: String,
    pub truncated: bool,
    pub results: Vec<NasSearchHit>,
}

pub fn emit_back_search_file(session_id: u32, value: &serde_json::Value) {
    if session_id == 0 {
        return;
    }
    let ok = value.get("ok").and_then(|v| v.as_bool()).unwrap_or(true);
    let keyword = value
        .get("keyword")
        .and_then(|v| v.as_str())
        .or_else(|| value.get("fileName").and_then(|v| v.as_str()))
        .unwrap_or("")
        .to_string();
    let truncated = value
        .get("truncated")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let results: Vec<NasSearchHit> = value
        .get("results")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| {
                    let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    if name.is_empty() {
                        return None;
                    }
                    Some(NasSearchHit {
                        name,
                        file_path: normalize_path(
                            item.get("filePath")
                                .and_then(|v| v.as_str())
                                .unwrap_or("/"),
                        ),
                        is_dir: item.get("isDir").and_then(|v| v.as_bool()).unwrap_or(false),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    eprintln!(
        "webrpc: backSearchFile session={session_id} keyword={keyword} hits={} truncated={truncated} ok={ok}",
        results.len()
    );
    if let Some(app) = webrpc::app_handle() {
        let _ = app.emit(
            "webrpc-nas-search",
            NasSearchEvent {
                session_id,
                ok,
                keyword,
                truncated,
                results,
            },
        );
    }
}

#[tauri::command]
pub fn nas_search(session_id: u32, keyword: String) {
    if session_id == 0 {
        return;
    }
    let keyword = keyword.trim().to_string();
    thread::spawn(move || {
        let payload = serde_json::json!({
            "type": "searchFile",
            "fileName": keyword,
        })
        .to_string();
        eprintln!("webrpc: searchFile session={session_id} keyword={keyword}");
        let ok = webrpc::send_json_timeout(session_id, &payload, SEARCH_TIMEOUT_MS);
        if !ok {
            eprintln!("webrpc: searchFile timeout session={session_id}");
            emit_back_search_file(
                session_id,
                &serde_json::json!({
                    "ok": false,
                    "keyword": keyword,
                    "truncated": false,
                    "results": [],
                }),
            );
        }
    });
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NasZipEvent {
    pub session_id: u32,
    pub ok: bool,
    pub done: bool,
    pub progress: i64,
    pub name: String,
    pub path: String,
    pub zip_name: String,
    pub error: String,
}

pub fn emit_back_zip_file(session_id: u32, value: &serde_json::Value) {
    if session_id == 0 {
        return;
    }
    let ok = value.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
    let done = value.get("done").and_then(|v| v.as_bool()).unwrap_or(false);
    let progress = value
        .get("progress")
        .and_then(|v| v.as_i64())
        .or_else(|| {
            value
                .get("progress")
                .and_then(|v| v.as_f64())
                .map(|n| n.round() as i64)
        })
        .unwrap_or(0);
    let name = value
        .get("fileName")
        .and_then(|v| v.as_str())
        .or_else(|| value.get("name").and_then(|v| v.as_str()))
        .unwrap_or("")
        .to_string();
    let path = normalize_path(value.get("filePath").and_then(|v| v.as_str()).or_else(|| value.get("path").and_then(|v| v.as_str())).unwrap_or("/"));
    let zip_name = value
        .get("zipName")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let error = value
        .get("error")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    eprintln!(
        "webrpc: backZipFile session={session_id} path={path} name={name} zip={zip_name} done={done} ok={ok} progress={progress} error={error}"
    );
    if let Some(app) = webrpc::app_handle() {
        let _ = app.emit(
            "webrpc-nas-zip",
            NasZipEvent {
                session_id,
                ok,
                done,
                progress,
                name,
                path,
                zip_name,
                error,
            },
        );
    }
}

#[tauri::command]
pub fn nas_zip_entry(session_id: u32, name: String, path: String) {
    if session_id == 0 {
        return;
    }
    let name = name.trim().replace(['\\', '/', '\0'], "");
    let dir = normalize_path(&path);
    if name.is_empty() || name == "." || name == ".." || name.starts_with('.') {
        emit_back_zip_file(
            session_id,
            &serde_json::json!({
                "ok": false,
                "done": true,
                "progress": 0,
                "fileName": name,
                "filePath": dir,
                "zipName": format!("{name}.zip"),
                "error": "invalid",
            }),
        );
        return;
    }
    thread::spawn(move || {
        let payload = serde_json::json!({
            "type": "zipFile",
            "fileName": name,
            "filePath": dir,
        })
        .to_string();
        eprintln!("webrpc: zipFile session={session_id} path={dir} name={name}");
        let ok = webrpc::send_json_timeout(session_id, &payload, GET_PATH_TIMEOUT_MS);
        if !ok {
            eprintln!("webrpc: zipFile timeout session={session_id}");
            emit_back_zip_file(
                session_id,
                &serde_json::json!({
                    "ok": false,
                    "done": true,
                    "progress": 0,
                    "fileName": name,
                    "filePath": dir,
                    "zipName": format!("{name}.zip"),
                    "error": "failed",
                }),
            );
        }
    });
}

#[tauri::command]
pub fn nas_list_path(session_id: u32, path: String) {
    if session_id == 0 {
        return;
    }
    thread::spawn(move || {
        request_path(session_id, &path);
    });
}

#[tauri::command]
pub fn nas_watch_start(session_id: u32) {
    start_watch(session_id);
}

#[tauri::command]
pub fn nas_watch_stop(session_id: u32) {
    stop_watch(session_id);
}

pub fn start_watch(session_id: u32) {
    if session_id == 0 {
        return;
    }
    stop_watch(session_id);
    let epoch = {
        let mut g = lock();
        g.drive_ids.insert(session_id);
        let gen = g.gens.entry(session_id).or_insert(0);
        *gen = gen.saturating_add(1);
        *gen
    };
    let spawned = thread::Builder::new()
        .name(format!("nas-watch-{session_id}"))
        .spawn(move || watch_loop(session_id, epoch));
    if let Ok(join) = spawned {
        lock().joins.insert(session_id, join);
        eprintln!("webrpc: nas watch start session={session_id}");
    }
}

pub fn stop_watch(session_id: u32) {
    let _join = {
        let mut g = lock();
        *g.gens.entry(session_id).or_insert(0) += 1;
        g.joins.remove(&session_id);
        g.drive_ids.remove(&session_id);
        g.list_paths.remove(&session_id);
        g.pending.remove(&session_id)
    };
    storage::delete_drive_temp_session(session_id);
    eprintln!("webrpc: nas watch stop session={session_id}");
}

pub fn stop_all() {
    let _joins = {
        let mut g = lock();
        for gen in g.gens.values_mut() {
            *gen = gen.saturating_add(1);
        }
        g.drive_ids.clear();
        g.pending.clear();
        g.list_paths.clear();
        std::mem::take(&mut g.joins)
    };
    storage::delete_all_drive_temp();
}

fn watch_loop(session_id: u32, epoch: u64) {
    if gen_of(session_id) != epoch {
        return;
    }
    request_path(session_id, "/");
    loop {
        if gen_of(session_id) != epoch {
            return;
        }
        webrpc::send_json_timeout(session_id, GET_NAS_INFO, NAS_SEND_TIMEOUT_MS);
        let deadline = Instant::now() + NAS_POLL;
        while Instant::now() < deadline {
            if gen_of(session_id) != epoch {
                return;
            }
            thread::sleep(Duration::from_millis(200));
        }
    }
}
