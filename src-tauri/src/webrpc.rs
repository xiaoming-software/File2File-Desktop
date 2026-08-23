use std::collections::HashMap;
use std::ffi::{c_void, CStr, CString};
use std::fs::OpenOptions;
use std::io::{self, Read, Write};
use std::panic::{self, AssertUnwindSafe};
use std::path::Path;
use std::net::TcpStream;
use std::os::raw::c_char;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, Once, OnceLock};
use std::thread;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::storage;

const LOGIN_TIMEOUT: Duration = Duration::from_secs(15);
const LOGIN_POLL: Duration = Duration::from_millis(200);
const CALLBACK_CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const HANDSHAKE_SEND_TIMEOUT_MS: i64 = 10_000;
const SEND_DATA_TIMEOUT_MS: i64 = 10_000;
const FILE_PROGRESS_INTERVAL: Duration = Duration::from_secs(1);
const FILE_QUERY_TIMEOUT_MS: i64 = 0;
const FILE_COPY_BUF: usize = 64 * 1024;
const MAX_FILE_NAME_LEN: usize = 4096;

struct RecvState {
    bytes: u64,
    size: u64,
    started: Instant,
    xfer_id: String,
    reset_pending: bool,
}

fn new_recv_state(size: u64) -> RecvState {
    RecvState {
        bytes: 0,
        size,
        started: Instant::now(),
        xfer_id: String::new(),
        reset_pending: false,
    }
}

struct SendJob {
    cancel: Arc<AtomicBool>,
    size: u64,
    started: Instant,
    msg_id: String,
    transferred: Arc<AtomicU64>,
}

#[derive(Default)]
struct SessionMonitors {
    gens: HashMap<u32, u64>,
    joins: HashMap<u32, JoinHandle<()>>,
}

static WEBRPC_HANDLE: AtomicUsize = AtomicUsize::new(0);
static CALLBACK_EPOCH: AtomicU64 = AtomicU64::new(0);
static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();
static SIZE_THREAD: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);
static LOGIN_TOKEN: Mutex<String> = Mutex::new(String::new());
static LOGIN_PERMISSION: Mutex<String> = Mutex::new(String::new());

pub struct WebrpcApp;

impl WebrpcApp {
    pub fn new() -> Self {
        Self
    }

    pub fn free(&self) {
        free_current();
    }
}

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
unsafe extern "C" {
    fn WebrpcClient_New(token: *mut c_char, passwd: *mut c_char, permission: *mut c_char) -> usize;
    fn WebrpcClient_LoginStatus(handle: usize) -> i32;
    fn WebrpcClient_GetReceivePort(handle: usize) -> i32;
    fn WebrpcClient_OpenSession(handle: usize, to_token: *mut c_char, permission: *mut c_char) -> u32;
    fn WebrpcClient_CloseSession(handle: usize, session_id: u32) -> i32;
    fn WebrpcClient_SessionSize(handle: usize) -> u16;
    fn WebrpcClient_TarTokenBySession(handle: usize, session_id: u32) -> *mut c_char;
    fn WebrpcClient_SendData(
        handle: usize,
        session_id: u32,
        data: *mut c_char,
        data_len: i32,
        time_out: i64,
    ) -> i32;
    fn WebrpcClient_SendFile(handle: usize, session_id: u32, file_path: *mut c_char) -> i32;
    fn WebrpcClient_Free(handle: usize);
}

extern "C" fn atexit_free_webrpc() {
    let _ = panic::catch_unwind(AssertUnwindSafe(|| {
        let handle = WEBRPC_HANDLE.swap(0, Ordering::SeqCst);
        if handle != 0 {
            unsafe { webrpc_free(handle) };
        }
    }));
}

extern "C" {
    fn atexit(cb: extern "C" fn()) -> i32;
}

pub fn install_exit_hooks() {
    ensure_go_runtime();
    unsafe {
        let _ = atexit(atexit_free_webrpc);
    }
}

/// Go `c-archive` 在 Windows MSVC 链接时不会执行 MinGW 风格的 `.ctors`，
/// 运行时永远起不来，随后所有 SDK 调用都会卡在 `_cgo_wait_runtime_init_done`。
/// macOS / Linux 的动态节构造函数会自动跑，这里不要重复调用以免双初始化。
pub fn ensure_go_runtime() {
    static START: Once = Once::new();
    START.call_once(|| {
        #[cfg(target_os = "windows")]
        crate::win::silence_stdio();
        #[cfg(all(target_os = "windows", target_env = "msvc", target_arch = "x86_64"))]
        {
            unsafe extern "C" {
                fn _cgo_maybe_run_preinit();
                fn _rt0_amd64_windows_lib();
            }
            eprintln!("webrpc: starting Go runtime (_rt0_amd64_windows_lib)");
            unsafe {
                _cgo_maybe_run_preinit();
                _rt0_amd64_windows_lib();
            }
        }
    });
}

pub fn set_app_handle(app: AppHandle) {
    let _ = APP_HANDLE.set(app);
}

fn bump_callback_epoch() {
    CALLBACK_EPOCH.fetch_add(1, Ordering::SeqCst);
}

fn callback_epoch() -> u64 {
    CALLBACK_EPOCH.load(Ordering::SeqCst)
}

fn store_handle(handle: usize) {
    let old = WEBRPC_HANDLE.swap(handle, Ordering::SeqCst);
    if old != 0 && old != handle {
        unsafe { webrpc_free(old) };
    }
}

fn set_login_identity(token: String, permission: String) {
    if let Ok(mut slot) = LOGIN_TOKEN.lock() {
        *slot = token;
    }
    if let Ok(mut slot) = LOGIN_PERMISSION.lock() {
        *slot = permission;
    }
}

fn clear_login_identity() {
    set_login_identity(String::new(), String::new());
}

fn login_identity() -> (String, String) {
    let token = LOGIN_TOKEN.lock().map(|slot| slot.clone()).unwrap_or_default();
    let permission = LOGIN_PERMISSION
        .lock()
        .map(|slot| slot.clone())
        .unwrap_or_default();
    (token, permission)
}

fn free_current() {
    static FREEING: AtomicBool = AtomicBool::new(false);
    if FREEING.swap(true, Ordering::SeqCst) {
        return;
    }
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        bump_callback_epoch();
        stop_session_size_poller();
        crate::nas::stop_all();
        crate::tasks::interrupt_all();
        stop_all_session_monitors();
        recycle_all_file_io();
        crate::voice::shutdown();
        crate::desktop::shutdown();
        clear_login_identity();
        let handle = WEBRPC_HANDLE.swap(0, Ordering::SeqCst);
        if handle != 0 {
            unsafe { webrpc_free(handle) };
        }
    }));
    FREEING.store(false, Ordering::SeqCst);
    if result.is_err() {
        eprintln!("webrpc: free_current panicked, ignored");
    }
}

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
unsafe fn webrpc_free(handle: usize) {
    WebrpcClient_Free(handle);
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
unsafe fn webrpc_free(_handle: usize) {}

fn current_handle() -> usize {
    WEBRPC_HANDLE.load(Ordering::SeqCst)
}

fn recv_tasks() -> &'static Mutex<HashMap<(u32, String), RecvState>> {
    static CELL: OnceLock<Mutex<HashMap<(u32, String), RecvState>>> = OnceLock::new();
    CELL.get_or_init(|| Mutex::new(HashMap::new()))
}

fn open_recv_file(path: &Path, reset: bool) -> io::Result<std::fs::File> {
    let mut opts = OpenOptions::new();
    opts.create(true).write(true);
    if reset {
        opts.truncate(true);
    } else {
        opts.append(true);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        opts.share_mode(0x1);
    }
    opts.open(path)
}

fn copy_tcp(stream: &mut TcpStream, len: u64, mut out: Option<&mut std::fs::File>) -> io::Result<()> {
    let mut buf = [0u8; FILE_COPY_BUF];
    let mut left = len;
    while left > 0 {
        let n = std::cmp::min(left, buf.len() as u64) as usize;
        stream.read_exact(&mut buf[..n])?;
        if let Some(file) = out.as_mut() {
            file.write_all(&buf[..n])?;
        }
        left -= n as u64;
    }
    Ok(())
}

fn send_jobs() -> &'static Mutex<HashMap<(u32, String), SendJob>> {
    static CELL: OnceLock<Mutex<HashMap<(u32, String), SendJob>>> = OnceLock::new();
    CELL.get_or_init(|| Mutex::new(HashMap::new()))
}

fn session_peers() -> &'static Mutex<HashMap<u32, String>> {
    static CELL: OnceLock<Mutex<HashMap<u32, String>>> = OnceLock::new();
    CELL.get_or_init(|| Mutex::new(HashMap::new()))
}

fn lock_peers() -> std::sync::MutexGuard<'static, HashMap<u32, String>> {
    session_peers().lock().unwrap_or_else(|err| err.into_inner())
}

fn remember_peer(session_id: u32, peer_token: &str) {
    if session_id == 0 {
        return;
    }
    let peer = peer_token.trim();
    if peer.is_empty() {
        return;
    }
    lock_peers().insert(session_id, peer.to_string());
}

pub(crate) fn peer_token_of(session_id: u32) -> String {
    if let Some(peer) = lock_peers().get(&session_id).cloned() {
        if !peer.is_empty() {
            return peer;
        }
    }
    let handle = current_handle();
    if handle == 0 {
        return String::new();
    }
    tar_token_by_session(handle, session_id)
}

fn recycle_session_file_io(session_id: u32) {
    lock_peers().remove(&session_id);
    {
        let mut jobs = send_jobs().lock().unwrap_or_else(|err| err.into_inner());
        jobs.retain(|(sid, _), job| {
            if *sid == session_id {
                job.cancel.store(true, Ordering::SeqCst);
                false
            } else {
                true
            }
        });
    }
    recv_tasks()
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .retain(|(sid, _), _| *sid != session_id);
}

fn recycle_all_file_io() {
    lock_peers().clear();
    {
        let mut jobs = send_jobs().lock().unwrap_or_else(|err| err.into_inner());
        for job in jobs.values() {
            job.cancel.store(true, Ordering::SeqCst);
        }
        jobs.clear();
    }
    recv_tasks()
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .clear();
}

fn session_monitors() -> &'static Mutex<SessionMonitors> {
    static CELL: OnceLock<Mutex<SessionMonitors>> = OnceLock::new();
    CELL.get_or_init(|| Mutex::new(SessionMonitors::default()))
}

fn lock_monitors() -> std::sync::MutexGuard<'static, SessionMonitors> {
    match session_monitors().lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    }
}

fn monitor_gen(session_id: u32) -> u64 {
    lock_monitors().gens.get(&session_id).copied().unwrap_or(0)
}

fn stop_session_monitor(session_id: u32) {
    let join = {
        let mut g = lock_monitors();
        *g.gens.entry(session_id).or_insert(0) += 1;
        g.joins.remove(&session_id)
    };
    if let Some(join) = join {
        let _ = join.join();
    }
}

fn stop_all_session_monitors() {
    let joins = {
        let mut g = lock_monitors();
        for gen in g.gens.values_mut() {
            *gen = gen.saturating_add(1);
        }
        std::mem::take(&mut g.joins)
    };
    for (_, join) in joins {
        let _ = join.join();
    }
}

fn start_session_monitor(session_id: u32) {
    if session_id == 0 {
        return;
    }
    stop_session_monitor(session_id);
    let epoch = {
        let mut g = lock_monitors();
        let gen = g.gens.entry(session_id).or_insert(0);
        *gen = gen.saturating_add(1);
        *gen
    };
    let spawned = thread::Builder::new()
        .name(format!("webrpc-watch-{session_id}"))
        .spawn(move || session_watch_loop(session_id, epoch));
    if let Ok(join) = spawned {
        lock_monitors().joins.insert(session_id, join);
    }
}

fn session_has_active_file(session_id: u32) -> bool {
    let sending = send_jobs()
        .lock()
        .map(|jobs| jobs.keys().any(|(sid, _)| *sid == session_id))
        .unwrap_or(false);
    if sending {
        return true;
    }
    if crate::tasks::is_session_busy(session_id) || crate::nas::recv_inflight(session_id) {
        return true;
    }
    recv_tasks()
        .lock()
        .map(|tasks| {
            tasks.iter().any(|((sid, _), state)| {
                *sid == session_id && (state.size == 0 || state.bytes < state.size)
            })
        })
        .unwrap_or(false)
}

fn session_watch_loop(session_id: u32, epoch: u64) {
    let mut empty_hits = 0u32;
    loop {
        if monitor_gen(session_id) != epoch {
            return;
        }
        let handle = current_handle();
        if handle == 0 {
            return;
        }
        let token = tar_token_by_session(handle, session_id);
        if token.is_empty() {
            empty_hits = empty_hits.saturating_add(1);
            let transferring = session_has_active_file(session_id);
            if transferring || empty_hits < 3 {
                eprintln!(
                    "webrpc: session {session_id} token empty hit={empty_hits} transferring={transferring}, skip expire"
                );
            } else {
                if monitor_gen(session_id) != epoch {
                    return;
                }
                eprintln!("webrpc: session {session_id} expired, CloseSession");
                let _ = close_session_blocking(handle, session_id);
                {
                    let mut g = lock_monitors();
                    if g.gens.get(&session_id).copied() == Some(epoch) {
                        g.joins.remove(&session_id);
                    }
                }
                if let Some(app) = APP_HANDLE.get() {
                    let _ = app.emit("webrpc-session-dead", session_id);
                }
                crate::nas::stop_watch(session_id);
                crate::tasks::on_session_dead(session_id);
                crate::voice::on_session_dead(session_id);
                crate::desktop::on_session_dead(session_id);
                recycle_session_file_io(session_id);
                return;
            }
        } else {
            empty_hits = 0;
        }
        for _ in 0..30 {
            if monitor_gen(session_id) != epoch {
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }
    }
}

fn tar_token_by_session(handle: usize, session_id: u32) -> String {
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        let _ = (handle, session_id);
        return String::new();
    }

    #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
    {
        let ptr = unsafe { WebrpcClient_TarTokenBySession(handle, session_id) };
        if ptr.is_null() {
            return String::new();
        }
        let text = unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .trim()
            .to_string();
        unsafe {
            extern "C" {
                fn free(p: *mut c_void);
            }
            free(ptr as *mut c_void);
        }
        text
    }
}

#[tauri::command]
pub async fn webrpc_login(
    token: String,
    password: String,
    passphrase: String,
    _state: tauri::State<'_, WebrpcApp>,
) -> Result<(), String> {
    free_current();
    let (handle, port) = tauri::async_runtime::spawn_blocking(move || {
        login_blocking(token, password, passphrase)
    })
    .await
    .map_err(|err| err.to_string())??;
    store_handle(handle);
    start_callback_reader(port);
    start_session_size_poller();
    Ok(())
}

#[tauri::command]
pub fn webrpc_logout(_state: tauri::State<'_, WebrpcApp>) {
    eprintln!("webrpc: logout, WebrpcClient_Free");
    free_current();
}

#[tauri::command]
pub async fn webrpc_open_session(
    peer_token: String,
    passphrase: String,
    _state: tauri::State<'_, WebrpcApp>,
) -> Result<u32, String> {
    let handle = current_handle();
    if handle == 0 {
        return Err("not-logged-in".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        open_session_blocking(handle, peer_token, passphrase)
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command]
pub async fn webrpc_send_data(
    session_id: u32,
    text: String,
    _state: tauri::State<'_, WebrpcApp>,
) -> Result<bool, String> {
    let handle = current_handle();
    if handle == 0 || session_id == 0 {
        return Ok(false);
    }
    tauri::async_runtime::spawn_blocking(move || send_text_blocking(handle, session_id, text))
        .await
        .map_err(|err| err.to_string())?
}

#[tauri::command]
pub fn webrpc_send_file(
    session_id: u32,
    path: String,
    file_name: String,
    size: u64,
    msg_id: String,
    _state: tauri::State<'_, WebrpcApp>,
) -> Result<(), String> {
    let handle = current_handle();
    if handle == 0 || session_id == 0 {
        return Err("not-logged-in".into());
    }
    let path = path.trim().to_string();
    let file_name = storage::safe_filename(&file_name);
    let msg_id = msg_id.trim().to_string();
    if path.is_empty() || file_name.is_empty() || msg_id.is_empty() {
        return Err("file-args-empty".into());
    }
    start_send_file_job(handle, session_id, path, file_name, size, msg_id);
    Ok(())
}

#[tauri::command]
pub async fn webrpc_close_session(
    session_id: u32,
    _state: tauri::State<'_, WebrpcApp>,
) -> Result<(), String> {
    if session_id == 0 {
        return Ok(());
    }
    tauri::async_runtime::spawn_blocking(move || {
        stop_session_monitor(session_id);
        crate::nas::stop_watch(session_id);
        crate::tasks::on_session_dead(session_id);
        recycle_session_file_io(session_id);
        crate::voice::on_session_dead(session_id);
        crate::desktop::on_session_dead(session_id);
        let handle = current_handle();
        if handle == 0 {
            return Ok(());
        }
        close_session_blocking(handle, session_id)
    })
    .await
    .map_err(|err| err.to_string())?
}

fn close_session_blocking(handle: usize, session_id: u32) -> Result<(), String> {
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        let _ = (handle, session_id);
        recycle_session_file_io(session_id);
        return Ok(());
    }

    #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
    {
        let ret = unsafe { WebrpcClient_CloseSession(handle, session_id) };
        eprintln!("webrpc: CloseSession id={session_id} ret={ret}");
        recycle_session_file_io(session_id);
        Ok(())
    }
}

fn open_session_blocking(handle: usize, peer_token: String, permission: String) -> Result<u32, String> {
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        let _ = (handle, peer_token, permission);
        return Err("unsupported-platform".into());
    }

    #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
    {
        let peer = peer_token.trim().to_string();
        let permission = permission.trim().to_string();
        if peer.is_empty() {
            return Err("peer-token-empty".into());
        }
        let peer_c = CString::new(peer.clone()).map_err(|_| "peer-token-invalid".to_string())?;
        let perm_c = CString::new(permission).map_err(|_| "passphrase-invalid".to_string())?;
        let session_id = unsafe {
            WebrpcClient_OpenSession(
                handle,
                peer_c.as_ptr() as *mut c_char,
                perm_c.as_ptr() as *mut c_char,
            )
        };
        let _hold = (peer_c, perm_c);
        if session_id == 0 {
            eprintln!("webrpc: OpenSession failed");
            return Err("open-session-failed".into());
        }
        eprintln!("webrpc: OpenSession ok, sessionId={session_id}");
        remember_peer(session_id, &peer);
        if let Err(err) = send_hello_blocking(handle, session_id) {
            eprintln!("webrpc: handshake SendData failed, CloseSession");
            let _ = close_session_blocking(handle, session_id);
            return Err(err);
        }
        start_session_monitor(session_id);
        Ok(session_id)
    }
}

#[derive(Serialize)]
struct HelloMessage<'a> {
    #[serde(rename = "type")]
    kind: u8,
    data: HelloData<'a>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HelloData<'a> {
    session_id: u32,
    token: &'a str,
    permission: &'a str,
}

fn send_hello_blocking(handle: usize, session_id: u32) -> Result<(), String> {
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        let _ = (handle, session_id);
        return Err("unsupported-platform".into());
    }

    #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
    {
        let (token, permission) = login_identity();
        if token.is_empty() {
            return Err("handshake-send-failed".into());
        }
        let payload = serde_json::to_string(&HelloMessage {
            kind: 1,
            data: HelloData {
                session_id,
                token: &token,
                permission: &permission,
            },
        })
        .map_err(|_| "handshake-send-failed".to_string())?;
        let data_c = CString::new(payload).map_err(|_| "handshake-send-failed".to_string())?;
        let data_len = data_c.as_bytes().len() as i32;
        let ret = unsafe {
            WebrpcClient_SendData(
                handle,
                session_id,
                data_c.as_ptr() as *mut c_char,
                data_len,
                HANDSHAKE_SEND_TIMEOUT_MS,
            )
        };
        eprintln!("webrpc: handshake SendData sessionId={session_id} ret={ret}");
        if ret != 1 {
            return Err("handshake-send-failed".into());
        }
        Ok(())
    }
}

#[derive(Serialize)]
struct TextMessage<'a> {
    #[serde(rename = "type")]
    kind: u8,
    data: &'a str,
}

fn send_text_blocking(handle: usize, session_id: u32, text: String) -> Result<bool, String> {
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        let _ = (handle, session_id, text);
        return Ok(false);
    }

    #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
    {
        let payload = serde_json::to_string(&TextMessage {
            kind: 2,
            data: &text,
        })
        .map_err(|_| "send-data-failed".to_string())?;
        let data_c = CString::new(payload).map_err(|_| "send-data-failed".to_string())?;
        let data_len = data_c.as_bytes().len() as i32;
        let ret = unsafe {
            WebrpcClient_SendData(
                handle,
                session_id,
                data_c.as_ptr() as *mut c_char,
                data_len,
                SEND_DATA_TIMEOUT_MS,
            )
        };
        eprintln!("webrpc: SendData type=2 sessionId={session_id} ret={ret}");
        Ok(ret == 1)
    }
}

#[derive(Serialize)]
struct FileQueryMessage<'a> {
    #[serde(rename = "type")]
    kind: u8,
    data: FileQueryData<'a>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FileQueryData<'a> {
    file_name: &'a str,
    size: u64,
    #[serde(skip_serializing_if = "str::is_empty")]
    msg_id: &'a str,
}

#[derive(Serialize)]
struct FileReplyMessage<'a> {
    #[serde(rename = "type")]
    kind: u8,
    data: FileReplyData<'a>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FileReplyData<'a> {
    bytes: u64,
    file_name: &'a str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct FileTransferEvent {
    session_id: u32,
    msg_id: String,
    file_name: String,
    from: String,
    transferred: u64,
    size: u64,
    elapsed_ms: u64,
    speed_bps: u64,
    status: String,
    file_path: String,
}

fn emit_file_event(event: FileTransferEvent) {
    if let Some(app) = APP_HANDLE.get() {
        let _ = app.emit("webrpc-file-event", event);
    }
}

fn speed_bps(bytes: u64, started: Instant) -> u64 {
    let ms = started.elapsed().as_millis().max(1) as u64;
    bytes.saturating_mul(1000) / ms
}

fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis() as u64
}

fn complete_stats(size: u64, started: Instant) -> (u64, u64, u64) {
    let elapsed = started.elapsed().as_millis().max(1) as u64;
    (size, elapsed, size.saturating_mul(1000) / elapsed)
}

fn persist_file_event_with(
    owner: &str,
    peer: &str,
    event: &FileTransferEvent,
) {
    crate::chats::persist_transfer(
        owner,
        peer,
        &event.msg_id,
        &event.from,
        &event.file_name,
        &event.status,
        event.size,
        event.transferred,
        event.elapsed_ms,
        event.speed_bps,
        &event.file_path,
    );
}

fn persist_file_event(event: &FileTransferEvent) {
    let (owner, _) = login_identity();
    let peer = peer_token_of(event.session_id);
    persist_file_event_with(&owner, &peer, event);
}

fn persist_file_event_async(event: FileTransferEvent) {
    let _ = thread::Builder::new()
        .name("webrpc-persist-file".into())
        .spawn(move || persist_file_event(&event));
}

fn file_progress_event(
    session_id: u32,
    file_name: String,
    from: &str,
    transferred: u64,
    size: u64,
    started: Instant,
    complete: bool,
    file_path: String,
) -> FileTransferEvent {
    let received = from != "me" && complete;
    let (bytes, elapsed, speed) = if complete && size > 0 {
        complete_stats(size, started)
    } else {
        (
            transferred,
            elapsed_ms(started),
            speed_bps(transferred, started),
        )
    };
    FileTransferEvent {
        session_id,
        msg_id: String::new(),
        file_name,
        from: from.into(),
        transferred: bytes,
        size,
        elapsed_ms: elapsed,
        speed_bps: speed,
        status: if received {
            "received".into()
        } else if from == "me" {
            "sending".into()
        } else {
            "receiving".into()
        },
        file_path,
    }
}

fn sleep_cancel(total: Duration, cancel: &AtomicBool) -> bool {
    let deadline = Instant::now() + total;
    while Instant::now() < deadline {
        if cancel.load(Ordering::SeqCst) {
            return false;
        }
        thread::sleep(Duration::from_millis(100));
    }
    !cancel.load(Ordering::SeqCst)
}

fn send_json_bytes(handle: usize, session_id: u32, payload: &str) -> bool {
    send_json_bytes_timeout(handle, session_id, payload, SEND_DATA_TIMEOUT_MS)
}

fn send_json_bytes_timeout(
    handle: usize,
    session_id: u32,
    payload: &str,
    timeout_ms: i64,
) -> bool {
    send_bytes_raw(handle, session_id, payload.as_bytes(), timeout_ms)
}

pub(crate) fn send_bytes_timeout(session_id: u32, payload: &[u8], timeout_ms: i64) -> bool {
    let handle = current_handle();
    if handle == 0 || session_id == 0 || payload.is_empty() {
        return false;
    }
    send_bytes_raw(handle, session_id, payload, timeout_ms)
}

fn send_bytes_raw(handle: usize, session_id: u32, payload: &[u8], timeout_ms: i64) -> bool {
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        let _ = (handle, session_id, payload, timeout_ms);
        return false;
    }
    #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
    {
        if payload.is_empty() || payload.len() > i32::MAX as usize {
            return false;
        }
        let ret = unsafe {
            WebrpcClient_SendData(
                handle,
                session_id,
                payload.as_ptr() as *mut c_char,
                payload.len() as i32,
                timeout_ms,
            )
        };
        ret == 1
    }
}

pub(crate) fn send_json_timeout(session_id: u32, payload: &str, timeout_ms: i64) -> bool {
    let handle = current_handle();
    if handle == 0 || session_id == 0 {
        return false;
    }
    send_json_bytes_timeout(handle, session_id, payload, timeout_ms)
}

pub(crate) fn rpc_handle() -> usize {
    current_handle()
}

pub(crate) fn app_handle() -> Option<&'static AppHandle> {
    APP_HANDLE.get()
}

fn send_type3(handle: usize, session_id: u32, file_name: &str, size: u64, msg_id: &str) {
    let Ok(payload) = serde_json::to_string(&FileQueryMessage {
        kind: 3,
        data: FileQueryData {
            file_name,
            size,
            msg_id,
        },
    }) else {
        return;
    };
    let ok = send_json_bytes_timeout(handle, session_id, &payload, FILE_QUERY_TIMEOUT_MS);
    eprintln!("webrpc: SendData type=3 sessionId={session_id} file={file_name} ok={ok}");
}

fn send_type4(handle: usize, session_id: u32, file_name: String, bytes: u64) {
    thread::Builder::new()
        .name("webrpc-type4".into())
        .spawn(move || {
            let Ok(payload) = serde_json::to_string(&FileReplyMessage {
                kind: 4,
                data: FileReplyData {
                    bytes,
                    file_name: &file_name,
                },
            }) else {
                return;
            };
            let ok = send_json_bytes(handle, session_id, &payload);
            eprintln!("webrpc: SendData type=4 sessionId={session_id} file={file_name} bytes={bytes} ok={ok}");
        })
        .ok();
}

fn start_send_file_job(
    handle: usize,
    session_id: u32,
    path: String,
    file_name: String,
    size: u64,
    msg_id: String,
) {
    let cancel = Arc::new(AtomicBool::new(false));
    let transferred = Arc::new(AtomicU64::new(0));
    let started = Instant::now();
    {
        let mut jobs = send_jobs().lock().unwrap_or_else(|err| err.into_inner());
        if let Some(old) = jobs.remove(&(session_id, file_name.clone())) {
            old.cancel.store(true, Ordering::SeqCst);
        }
        jobs.insert(
            (session_id, file_name.clone()),
            SendJob {
                cancel: cancel.clone(),
                size,
                started,
                msg_id: msg_id.clone(),
                transferred: transferred.clone(),
            },
        );
    }
    let _ = thread::Builder::new()
        .name(format!("webrpc-sendfile-{session_id}"))
        .spawn(move || {
            send_file_job(
                handle,
                session_id,
                path,
                file_name,
                size,
                msg_id,
                cancel,
                transferred,
                started,
            )
        });
}

fn send_file_job(
    handle: usize,
    session_id: u32,
    path: String,
    file_name: String,
    size: u64,
    msg_id: String,
    cancel: Arc<AtomicBool>,
    transferred: Arc<AtomicU64>,
    started: Instant,
) {
    let (owner, _) = login_identity();
    let peer = peer_token_of(session_id);
    let done = Arc::new(AtomicBool::new(false));
    let poll_cancel = cancel.clone();
    let poll_done = done.clone();
    let poll_name = file_name.clone();
    let poll_msg_id = msg_id.clone();
    let poll = thread::Builder::new()
        .name(format!("webrpc-file-poll-{session_id}"))
        .spawn(move || {
            send_type3(handle, session_id, &poll_name, size, &poll_msg_id);
            while !poll_cancel.load(Ordering::SeqCst) && !poll_done.load(Ordering::SeqCst) {
                if !sleep_cancel(FILE_PROGRESS_INTERVAL, &poll_cancel) {
                    return;
                }
                if current_handle() != handle {
                    return;
                }
                send_type3(handle, session_id, &poll_name, size, &poll_msg_id);
            }
        })
        .ok();

    let ret = send_file_blocking(handle, session_id, &path);
    done.store(true, Ordering::SeqCst);
    if let Some(join) = poll {
        let _ = join.join();
    }

    send_jobs()
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .remove(&(session_id, file_name.clone()));

    let cancelled = cancel.load(Ordering::SeqCst) || current_handle() != handle;
    let last_bytes = transferred.load(Ordering::SeqCst);
    if cancelled && ret != 1 {
        let event = FileTransferEvent {
            session_id,
            msg_id,
            file_name,
            from: "me".into(),
            transferred: last_bytes,
            size,
            elapsed_ms: elapsed_ms(started).max(1),
            speed_bps: speed_bps(last_bytes, started),
            status: "failed".into(),
            file_path: path,
        };
        persist_file_event_with(&owner, &peer, &event);
        emit_file_event(event);
        return;
    }

    send_type3(handle, session_id, &file_name, size, &msg_id);
    if ret != 1 {
        send_type3(handle, session_id, &file_name, size, &msg_id);
    }

    let ok = ret == 1;
    let (done_bytes, done_elapsed, done_speed) = if ok {
        complete_stats(size, started)
    } else {
        (
            last_bytes,
            elapsed_ms(started).max(1),
            speed_bps(last_bytes, started),
        )
    };
    let event = FileTransferEvent {
        session_id,
        msg_id,
        file_name,
        from: "me".into(),
        transferred: done_bytes,
        size,
        elapsed_ms: done_elapsed,
        speed_bps: done_speed,
        status: if ok { "sent".into() } else { "failed".into() },
        file_path: path,
    };
    persist_file_event_with(&owner, &peer, &event);
    emit_file_event(event);
}

fn send_file_blocking(handle: usize, session_id: u32, path: &str) -> i32 {
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        let _ = (handle, session_id, path);
        return 0;
    }
    #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
    {
        let Ok(path_c) = CString::new(path) else {
            return 0;
        };
        let ret = unsafe {
            WebrpcClient_SendFile(handle, session_id, path_c.as_ptr() as *mut c_char)
        };
        eprintln!("webrpc: SendFile sessionId={session_id} ret={ret}");
        ret
    }
}

pub(crate) fn send_local_file(session_id: u32, path: &str) -> i32 {
    let handle = current_handle();
    if handle == 0 || session_id == 0 || path.trim().is_empty() {
        return 0;
    }
    send_file_blocking(handle, session_id, path)
}

fn login_blocking(
    token: String,
    password: String,
    permission: String,
) -> Result<(usize, u16), String> {
    let token = token.trim().to_string();
    let password = password.trim().to_string();
    let permission = permission.trim().to_string();
    if token.is_empty() || password.is_empty() {
        return Err("token-or-password-empty".into());
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        let _ = permission;
        return Err("unsupported-platform".into());
    }

    #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
    {
        #[cfg(target_os = "windows")]
        let _console_guard = crate::win::ConsoleGuard::start();
        ensure_go_runtime();
        let deadline = Instant::now() + LOGIN_TIMEOUT;

        let token_c = CString::new(token.clone()).map_err(|_| "token-invalid".to_string())?;
        let pass_c = CString::new(password).map_err(|_| "password-invalid".to_string())?;
        let perm_c = CString::new(permission.clone()).map_err(|_| "passphrase-invalid".to_string())?;
        // New 若卡住也不能提前释放这些 CString；Arc 让超时返回后线程仍可持有。
        let args = Arc::new((token_c, pass_c, perm_c));
        let args_for_new = Arc::clone(&args);
        let (tx, rx) = mpsc::sync_channel(1);
        let _ = thread::Builder::new()
            .name("webrpc-new".into())
            .spawn(move || {
                #[cfg(target_os = "windows")]
                crate::win::silence_stdio();
                let handle = unsafe {
                    WebrpcClient_New(
                        args_for_new.0.as_ptr() as *mut c_char,
                        args_for_new.1.as_ptr() as *mut c_char,
                        args_for_new.2.as_ptr() as *mut c_char,
                    )
                };
                let _ = tx.send(handle);
            });

        let remaining = deadline.saturating_duration_since(Instant::now());
        let handle = match rx.recv_timeout(remaining) {
            Ok(0) => {
                eprintln!("webrpc: WebrpcClient_New returned 0");
                return Err("sdk-empty-handle".into());
            }
            Ok(handle) => handle,
            Err(_) => {
                eprintln!("webrpc: WebrpcClient_New timed out");
                return Err("login-timeout".into());
            }
        };

        // 轮询期间保持 CString 存活，与官方示例一致。
        let _hold = args;
        loop {
            let status = unsafe { WebrpcClient_LoginStatus(handle) };
            if status != 0 {
                let port = unsafe { WebrpcClient_GetReceivePort(handle) };
                if port <= 0 || port > u16::MAX as i32 {
                    eprintln!("webrpc: invalid callback port {port}, Free");
                    unsafe { WebrpcClient_Free(handle) };
                    return Err("callback-port-invalid".into());
                }
                eprintln!("webrpc: login ok, status={status}, port={port}");
                set_login_identity(token, permission);
                return Ok((handle, port as u16));
            }
            if Instant::now() >= deadline {
                eprintln!("webrpc: login timeout, Free handle");
                unsafe { WebrpcClient_Free(handle) };
                return Err("login-timeout".into());
            }
            thread::sleep(LOGIN_POLL);
        }
    }
}

fn emit_session_size(size: u16) {
    if let Some(app) = APP_HANDLE.get() {
        let _ = app.emit("webrpc-session-size", size);
    }
}

fn session_size(handle: usize) -> u16 {
    #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
    unsafe {
        WebrpcClient_SessionSize(handle)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        let _ = handle;
        0
    }
}

fn stop_session_size_poller() {
    let join = match SIZE_THREAD.lock() {
        Ok(mut slot) => slot.take(),
        Err(poisoned) => poisoned.into_inner().take(),
    };
    if let Some(join) = join {
        let _ = join.join();
    }
}

fn start_session_size_poller() {
    stop_session_size_poller();
    let epoch = callback_epoch();
    let spawned = thread::Builder::new()
        .name("webrpc-session-size".into())
        .spawn(move || session_size_loop(epoch));
    if let Ok(join) = spawned {
        match SIZE_THREAD.lock() {
            Ok(mut slot) => *slot = Some(join),
            Err(poisoned) => *poisoned.into_inner() = Some(join),
        }
    }
}

fn session_size_loop(epoch: u64) {
    loop {
        if callback_epoch() != epoch {
            break;
        }
        let handle = current_handle();
        if handle == 0 {
            break;
        }
        let size = session_size(handle);
        emit_session_size(size);
        for _ in 0..10 {
            if callback_epoch() != epoch {
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }
    }
}

fn start_callback_reader(port: u16) {
    let epoch = callback_epoch();
    let _ = thread::Builder::new()
        .name("webrpc-callback".into())
        .spawn(move || callback_loop(port, epoch));
}

fn callback_loop(port: u16, epoch: u64) {
    let Some(mut stream) = connect_callback(port, epoch) else {
        return;
    };
    eprintln!("webrpc: callback connected 127.0.0.1:{port}");
    loop {
        if callback_epoch() != epoch {
            break;
        }
        let session_id = match read_u32_be(&mut stream) {
            Ok(v) => v,
            Err(_) => break,
        };
        let mut kind = [0u8; 1];
        if stream.read_exact(&mut kind).is_err() {
            break;
        }
        let result = match kind[0] {
            2 => match read_data_stream(&mut stream) {
                Ok(payload) => {
                    handle_data_payload(session_id, payload);
                    Ok(())
                }
                Err(err) => Err(err),
            },
            1 => handle_file_stream(session_id, &mut stream),
            other => {
                eprintln!("webrpc: unknown callback type {other}, session={session_id}");
                Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "unknown callback type",
                ))
            }
        };
        if result.is_err() {
            break;
        }
        let _ = session_id;
    }
    eprintln!("webrpc: callback reader stopped");
}

fn connect_callback(port: u16, epoch: u64) -> Option<TcpStream> {
    let addr = format!("127.0.0.1:{port}");
    let deadline = Instant::now() + CALLBACK_CONNECT_TIMEOUT;
    loop {
        if callback_epoch() != epoch {
            return None;
        }
        match TcpStream::connect(&addr) {
            Ok(stream) => return Some(stream),
            Err(err) => {
                if Instant::now() >= deadline {
                    eprintln!("webrpc: callback connect failed {addr}: {err}");
                    return None;
                }
                thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

fn read_u32_be(stream: &mut TcpStream) -> io::Result<u32> {
    let mut buf = [0u8; 4];
    stream.read_exact(&mut buf)?;
    Ok(u32::from_be_bytes(buf))
}

fn drain_exact(stream: &mut TcpStream, len: u64) -> io::Result<()> {
    copy_tcp(stream, len, None)
}

fn read_data_stream(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    let len = read_u32_be(stream)? as usize;
    if len == 0 {
        return Ok(Vec::new());
    }
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf)?;
    Ok(buf)
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct InboundHello {
    session_id: u32,
    token: String,
    #[serde(default)]
    permission: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct InboundText {
    session_id: u32,
    text: String,
}

fn handle_data_payload(frame_session_id: u32, payload: Vec<u8>) {
    if payload.len() >= 3 && payload[0] == 0 && payload[1] == 0 && payload[2] == 0 {
        crate::desktop::on_video_binary(frame_session_id, &payload);
        return;
    }
    let value: serde_json::Value = match serde_json::from_slice(&payload) {
        Ok(v) => v,
        Err(_) => return,
    };
    match value.get("type") {
        Some(t) if t.is_string() => match t.as_str().unwrap_or("") {
            "backNasInfo" => crate::nas::emit_back_nas_info(frame_session_id, value.get("data").unwrap_or(&serde_json::Value::Null)),
            "backPath" => crate::nas::emit_back_path(
                frame_session_id,
                value.get("data").unwrap_or(&serde_json::Value::Null),
                value.get("path").and_then(|v| v.as_str()).unwrap_or(""),
            ),
            "backFile" => crate::nas::emit_back_file(
                frame_session_id,
                value.get("fileName").and_then(|v| v.as_str()).unwrap_or(""),
                value.get("filePath").and_then(|v| v.as_str()).unwrap_or("/"),
            ),
            "putFile" | "createFile" | "getNasInfo" | "getPath" | "getFile" | "moveFile" | "moveFiles" | "deleteFile" | "renameFile" | "searchFile" | "zipFile" => {}
            "backCreateFile" => crate::nas::emit_back_create_file(frame_session_id, &value),
            "backMoveFile" | "backMoveFiles" => crate::nas::emit_back_move_file(frame_session_id, &value),
            "backDeleteFile" => crate::nas::emit_back_delete_file(frame_session_id, &value),
            "backRenameFile" => crate::nas::emit_back_rename_file(frame_session_id, &value),
            "backSearchFile" => crate::nas::emit_back_search_file(frame_session_id, &value),
            "backZipFile" => crate::nas::emit_back_zip_file(frame_session_id, &value),
            _ => {}
        },
        Some(t) if t.is_number() => {
            let kind = t.as_i64().unwrap_or(0);
            let data = value.get("data").cloned().unwrap_or(serde_json::Value::Null);
            match kind {
                1 => handle_hello_payload(data),
                2 => handle_text_payload(frame_session_id, data),
                3 => handle_file_query(frame_session_id, data),
                4 => handle_file_reply(frame_session_id, data),
                5 => crate::voice::on_audio_frame(frame_session_id, data),
                6 => crate::voice::on_signal(frame_session_id, data),
                7 => crate::desktop::on_video_frame(frame_session_id, data),
                8 => crate::desktop::on_signal(frame_session_id, data),
                9 => crate::desktop::on_input(frame_session_id, data),
                _ => {}
            }
        }
        _ => {}
    }
}

fn handle_hello_payload(data: serde_json::Value) {
    let hello: InboundHello = match serde_json::from_value(data) {
        Ok(v) => v,
        Err(_) => return,
    };
    if hello.token.trim().is_empty() || hello.session_id == 0 {
        return;
    }
    eprintln!(
        "webrpc: peer hello token={} sessionId={}",
        hello.token, hello.session_id
    );
    if let Some(app) = APP_HANDLE.get() {
        let _ = app.emit("webrpc-peer-hello", hello.clone());
    }
    remember_peer(hello.session_id, &hello.token);
    start_session_monitor(hello.session_id);
}

fn handle_text_payload(session_id: u32, data: serde_json::Value) {
    if session_id == 0 {
        return;
    }
    let Some(text) = data.as_str() else {
        return;
    };
    eprintln!(
        "webrpc: peer text sessionId={} bytes={}",
        session_id,
        text.len()
    );
    if let Some(app) = APP_HANDLE.get() {
        let _ = app.emit(
            "webrpc-peer-text",
            InboundText {
                session_id,
                text: text.to_string(),
            },
        );
    }
}

fn handle_file_query(session_id: u32, data: serde_json::Value) {
    if session_id == 0 {
        return;
    }
    let file_name = data
        .get("fileName")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let size = data.get("size").and_then(|v| v.as_u64()).unwrap_or(0);
    let file_name = storage::safe_filename(&file_name);
    if file_name.is_empty() {
        return;
    }
    let msg_id = data
        .get("msgId")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let (bytes, started, complete, dest) = {
        let mut map = recv_tasks().lock().unwrap_or_else(|err| err.into_inner());
        let state = map
            .entry((session_id, file_name.clone()))
            .or_insert_with(|| new_recv_state(size));
        if size > 0 {
            state.size = size;
        }
        if !msg_id.is_empty() {
            if !state.xfer_id.is_empty() && state.xfer_id != msg_id {
                eprintln!(
                    "webrpc: recv restart session={session_id} file={file_name} xfer {} -> {msg_id}",
                    state.xfer_id
                );
                state.reset_pending = true;
            }
            state.xfer_id = msg_id;
        }
        let complete = state.size > 0 && state.bytes >= state.size;
        let bytes = state.bytes;
        let started = state.started;
        let dest = incoming_dest(session_id, &file_name);
        (bytes, started, complete, dest)
    };
    let handle = current_handle();
    if handle != 0 {
        send_type4(handle, session_id, file_name.clone(), bytes);
    }
    let event = file_progress_event(
        session_id,
        file_name,
        "peer",
        bytes,
        size,
        started,
        complete,
        dest,
    );
    persist_file_event_async(event.clone());
    emit_file_event(event);
}

fn handle_file_reply(session_id: u32, data: serde_json::Value) {
    if session_id == 0 {
        return;
    }
    let file_name = data
        .get("fileName")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let bytes = data.get("bytes").and_then(|v| v.as_u64()).unwrap_or(0);
    let file_name = storage::safe_filename(&file_name);
    if file_name.is_empty() {
        return;
    }
    let job = {
        let jobs = send_jobs().lock().unwrap_or_else(|err| err.into_inner());
        jobs.get(&(session_id, file_name.clone())).map(|job| {
            job.transferred.fetch_max(bytes, Ordering::SeqCst);
            (
                job.msg_id.clone(),
                job.size,
                job.started,
                job.cancel.load(Ordering::SeqCst),
                job.transferred.load(Ordering::SeqCst),
            )
        })
    };
    let Some((msg_id, size, started, cancelled, transferred)) = job else {
        crate::tasks::on_upload_ack(session_id, &file_name, bytes);
        return;
    };
    if cancelled {
        return;
    }
    let event = FileTransferEvent {
        session_id,
        msg_id,
        file_name,
        from: "me".into(),
        transferred,
        size,
        elapsed_ms: elapsed_ms(started),
        speed_bps: speed_bps(transferred, started),
        status: "sending".into(),
        file_path: String::new(),
    };
    persist_file_event_async(event.clone());
    emit_file_event(event);
}

fn incoming_dest(session_id: u32, file_name: &str) -> String {
    let (owner, _) = login_identity();
    let peer = peer_token_of(session_id);
    if owner.is_empty() || peer.is_empty() {
        return String::new();
    }
    storage::incoming_file_path(&owner, &peer, file_name)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn handle_file_stream(session_id: u32, stream: &mut TcpStream) -> io::Result<()> {
    let name_len = read_u32_be(stream)? as usize;
    if name_len > MAX_FILE_NAME_LEN {
        drain_exact(stream, name_len as u64)?;
        let chunk_len = read_u32_be(stream)? as u64;
        return drain_exact(stream, chunk_len);
    }
    let mut name_buf = vec![0u8; name_len];
    if name_len > 0 {
        stream.read_exact(&mut name_buf)?;
    }
    let raw_name = String::from_utf8_lossy(&name_buf).into_owned();
    let file_name = storage::safe_filename(&raw_name);
    let chunk_len = read_u32_be(stream)? as u64;

    if crate::nas::is_drive_session(session_id) {
        return handle_nas_file_stream(session_id, &file_name, stream, chunk_len);
    }

    let (owner, _) = login_identity();
    let peer = peer_token_of(session_id);
    if owner.is_empty() || peer.is_empty() || file_name.is_empty() {
        return drain_exact(stream, chunk_len);
    }
    let dest = match storage::incoming_file_path(&owner, &peer, &file_name) {
        Ok(p) => p,
        Err(_) => return drain_exact(stream, chunk_len),
    };

    let key = (session_id, file_name.clone());
    let reset = {
        let mut map = recv_tasks().lock().unwrap_or_else(|err| err.into_inner());
        let state = map
            .entry(key.clone())
            .or_insert_with(|| new_recv_state(0));
        let reset = state.reset_pending
            || state.bytes == 0
            || (state.size > 0 && state.bytes >= state.size);
        if reset {
            if state.bytes > 0 || state.reset_pending {
                eprintln!(
                    "webrpc: recv truncate session={session_id} file={file_name} prev_bytes={}",
                    state.bytes
                );
            }
            state.bytes = 0;
            state.started = Instant::now();
            state.reset_pending = false;
        }
        reset
    };

    let mut out = match open_recv_file(&dest, reset) {
        Ok(file) => file,
        Err(err) => {
            eprintln!("webrpc: open recv file failed {}: {err}", dest.display());
            return drain_exact(stream, chunk_len);
        }
    };
    copy_tcp(stream, chunk_len, Some(&mut out))?;
    out.flush()?;
    drop(out);

    let (bytes, size, started, complete) = {
        let mut map = recv_tasks().lock().unwrap_or_else(|err| err.into_inner());
        let state = map
            .entry(key)
            .or_insert_with(|| new_recv_state(0));
        state.bytes = state.bytes.saturating_add(chunk_len);
        let complete = state.size > 0 && state.bytes >= state.size;
        (state.bytes, state.size, state.started, complete)
    };

    let event = file_progress_event(
        session_id,
        file_name,
        "peer",
        bytes,
        size,
        started,
        complete,
        dest.to_string_lossy().into_owned(),
    );
    if complete {
        persist_file_event_async(event.clone());
    }
    emit_file_event(event);
    Ok(())
}

fn handle_nas_file_stream(
    session_id: u32,
    file_name: &str,
    stream: &mut TcpStream,
    chunk_len: u64,
) -> io::Result<()> {
    let Some(dest) = crate::nas::recv_file_path(session_id, file_name) else {
        return drain_exact(stream, chunk_len);
    };
    if let Some(parent) = dest.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let reset = !dest.exists();
    let mut out = match open_recv_file(&dest, reset) {
        Ok(file) => file,
        Err(err) => {
            eprintln!("webrpc: open nas temp failed {}: {err}", dest.display());
            return drain_exact(stream, chunk_len);
        }
    };
    copy_tcp(stream, chunk_len, Some(&mut out))?;
    out.flush()?;
    drop(out);
    if let Ok(meta) = std::fs::metadata(&dest) {
        crate::tasks::on_download_bytes(session_id, meta.len());
    }
    eprintln!(
        "webrpc: nas file chunk session={session_id} file={} bytes={chunk_len} dest={}",
        file_name,
        dest.display()
    );
    Ok(())
}
