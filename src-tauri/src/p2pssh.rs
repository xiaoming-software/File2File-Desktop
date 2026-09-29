//! P2P SSH client: webrpc magic-4 tunnel.
//! - In-app terminal: remote PTY shell channel (no local ssh / no sshd required)
//! - Optional local TCP port: forwards to remote sshd for external `ssh -p`

use crate::storage;
use crate::webrpc;
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};

const SSH_FILE: &str = "saved_p2pssh.json";
const MAGIC: [u8; 4] = [0, 0, 0, 4];
const OP_OPEN: u8 = 1;
const OP_OPEN_ACK: u8 = 2;
const OP_OPEN_ERR: u8 = 3;
const OP_DATA: u8 = 4;
const OP_CLOSE: u8 = 5;
const OP_RESIZE: u8 = 6;
const SEND_TIMEOUT_MS: i64 = 5000;
const OPEN_ACK_TIMEOUT: Duration = Duration::from_secs(15);
const OPEN_SHELL: &[u8] = b"shell";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedP2pSsh {
    pub id: String,
    pub peer_token: String,
    #[serde(default)]
    pub peer_pass: String,
    #[serde(default)]
    pub remark: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct SavedFile {
    #[serde(default)]
    accounts: HashMap<String, Vec<SavedP2pSsh>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct P2pSshConnectInfo {
    pub id: String,
    pub local_port: u16,
    pub ssh_command: String,
    pub webrpc_session_id: u32,
}

struct ChannelState {
    stream: TcpStream,
    closed: AtomicBool,
    next_out: AtomicU32,
    next_in: Mutex<u32>,
    pending: Mutex<HashMap<u32, Vec<u8>>>,
}

struct ShellChannel {
    term_id: String,
    closed: AtomicBool,
    next_out: AtomicU32,
    next_in: Mutex<u32>,
    pending: Mutex<HashMap<u32, Vec<u8>>>,
}

struct TunnelRuntime {
    entry_id: String,
    webrpc_session: u32,
    local_port: u16,
    stop: Arc<AtomicBool>,
    channels: Mutex<HashMap<u32, Arc<ChannelState>>>,
    shells: Mutex<HashMap<u32, Arc<ShellChannel>>>,
    next_channel: AtomicU32,
}

struct TermRuntime {
    entry_id: String,
    channel: u32,
    webrpc_session: u32,
    stop: Arc<AtomicBool>,
}

struct GlobalState {
    tunnels: Mutex<HashMap<String, Arc<TunnelRuntime>>>,
    by_session: Mutex<HashMap<u32, String>>,
    terms: Mutex<HashMap<String, Arc<TermRuntime>>>,
    app: Mutex<Option<AppHandle>>,
}

fn state() -> &'static GlobalState {
    static S: OnceLock<GlobalState> = OnceLock::new();
    S.get_or_init(|| GlobalState {
        tunnels: Mutex::new(HashMap::new()),
        by_session: Mutex::new(HashMap::new()),
        terms: Mutex::new(HashMap::new()),
        app: Mutex::new(None),
    })
}

pub fn set_app_handle(app: AppHandle) {
    if let Ok(mut slot) = state().app.lock() {
        *slot = Some(app);
    }
}

fn emit_event<T: Serialize + Clone>(event: &str, payload: T) {
    if let Ok(slot) = state().app.lock() {
        if let Some(app) = slot.as_ref() {
            let _ = app.emit(event, payload);
        }
    }
}

fn now_id(prefix: &str) -> String {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("{prefix}-{ms}-{:04}", (ms % 9973) as u32)
}

fn ssh_path() -> Result<PathBuf, String> {
    Ok(storage::app_data_subdir("p2pssh")?.join(SSH_FILE))
}

fn load_file() -> Result<SavedFile, String> {
    let path = ssh_path()?;
    if !path.exists() {
        return Ok(SavedFile::default());
    }
    let raw = fs::read_to_string(&path).map_err(|e| format!("读取 P2P SSH 缓存失败: {e}"))?;
    if raw.trim().is_empty() {
        return Ok(SavedFile::default());
    }
    serde_json::from_str(&raw).map_err(|e| format!("解析 P2P SSH 缓存失败: {e}"))
}

fn save_file(data: &SavedFile) -> Result<(), String> {
    let path = ssh_path()?;
    let text = serde_json::to_string_pretty(data).map_err(|e| format!("序列化失败: {e}"))?;
    fs::write(&path, text).map_err(|e| format!("写入失败: {e}"))?;
    Ok(())
}

fn list_for(data: &SavedFile, owner: &str) -> Vec<SavedP2pSsh> {
    data.accounts.get(owner).cloned().unwrap_or_default()
}

fn encode_frame(op: u8, channel: u32, seq: u32, payload: &[u8]) -> Vec<u8> {
    let mut b = Vec::with_capacity(17 + payload.len());
    b.extend_from_slice(&MAGIC);
    b.push(op);
    b.extend_from_slice(&channel.to_le_bytes());
    b.extend_from_slice(&seq.to_le_bytes());
    b.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    b.extend_from_slice(payload);
    b
}

fn decode_frame(raw: &[u8]) -> Option<(u8, u32, u32, &[u8])> {
    if raw.len() < 17 || raw[0..4] != MAGIC {
        return None;
    }
    let op = raw[4];
    let channel = u32::from_le_bytes(raw[5..9].try_into().ok()?);
    let seq = u32::from_le_bytes(raw[9..13].try_into().ok()?);
    let plen = u32::from_le_bytes(raw[13..17].try_into().ok()?) as usize;
    if raw.len() < 17 + plen {
        return None;
    }
    Some((op, channel, seq, &raw[17..17 + plen]))
}

fn send_frame(session_id: u32, frame: &[u8]) -> bool {
    webrpc::send_bytes_timeout(session_id, frame, SEND_TIMEOUT_MS)
}

pub(crate) fn is_p2pssh_session(session_id: u32) -> bool {
    state()
        .by_session
        .lock()
        .map(|m| m.contains_key(&session_id))
        .unwrap_or(false)
}

pub(crate) fn on_binary(session_id: u32, payload: &[u8]) {
    if decode_frame(payload).is_none() {
        return;
    }
    let entry_id = match state().by_session.lock() {
        Ok(m) => m.get(&session_id).cloned(),
        Err(_) => None,
    };
    let Some(entry_id) = entry_id else { return };
    let tunnel = match state().tunnels.lock() {
        Ok(m) => m.get(&entry_id).cloned(),
        Err(_) => None,
    };
    let Some(tunnel) = tunnel else { return };
    let Some((op, channel, seq, body)) = decode_frame(payload) else {
        return;
    };
    match op {
        OP_OPEN_ACK | OP_OPEN_ERR => {
            // accept loop waits via channel events stored in pending map keyed specially
            notify_open_result(&tunnel, channel, op == OP_OPEN_ACK, body);
        }
        OP_DATA => on_channel_data(&tunnel, channel, seq, body),
        OP_CLOSE => on_remote_close(&tunnel, channel),
        _ => {}
    }
}

struct OpenWait {
    done: AtomicBool,
    ok: AtomicBool,
    err: Mutex<String>,
}

fn open_waits() -> &'static Mutex<HashMap<(String, u32), Arc<OpenWait>>> {
    static W: OnceLock<Mutex<HashMap<(String, u32), Arc<OpenWait>>>> = OnceLock::new();
    W.get_or_init(|| Mutex::new(HashMap::new()))
}

fn notify_open_result(tunnel: &TunnelRuntime, channel: u32, ok: bool, body: &[u8]) {
    let key = (tunnel.entry_id.clone(), channel);
    if let Ok(mut map) = open_waits().lock() {
        if let Some(w) = map.remove(&key) {
            if !ok {
                if let Ok(mut e) = w.err.lock() {
                    *e = String::from_utf8_lossy(body).to_string();
                }
            }
            w.ok.store(ok, Ordering::SeqCst);
            w.done.store(true, Ordering::SeqCst);
        }
    }
}

fn register_open_wait(entry_id: &str, channel: u32) -> Arc<OpenWait> {
    let wait = Arc::new(OpenWait {
        done: AtomicBool::new(false),
        ok: AtomicBool::new(false),
        err: Mutex::new(String::new()),
    });
    if let Ok(mut map) = open_waits().lock() {
        map.insert((entry_id.to_string(), channel), wait.clone());
    }
    wait
}

fn clear_open_wait(entry_id: &str, channel: u32) {
    if let Ok(mut map) = open_waits().lock() {
        map.remove(&(entry_id.to_string(), channel));
    }
}

fn wait_open_ack(wait: &OpenWait, entry_id: &str, channel: u32) -> Result<(), String> {
    let deadline = Instant::now() + OPEN_ACK_TIMEOUT;
    while Instant::now() < deadline {
        if wait.done.load(Ordering::SeqCst) {
            if wait.ok.load(Ordering::SeqCst) {
                return Ok(());
            }
            let err = wait
                .err
                .lock()
                .map(|e| e.clone())
                .unwrap_or_else(|_| "open failed".into());
            return Err(if err.is_empty() {
                "remote open failed".into()
            } else {
                err
            });
        }
        thread::sleep(Duration::from_millis(20));
    }
    clear_open_wait(entry_id, channel);
    Err("open ack timeout".into())
}

fn on_channel_data(tunnel: &TunnelRuntime, channel: u32, seq: u32, payload: &[u8]) {
    if let Some(shell) = tunnel
        .shells
        .lock()
        .ok()
        .and_then(|m| m.get(&channel).cloned())
    {
        on_shell_data(tunnel, channel, &shell, seq, payload);
        return;
    }
    let ch = {
        let map = match tunnel.channels.lock() {
            Ok(m) => m,
            Err(_) => return,
        };
        match map.get(&channel) {
            Some(c) => c.clone(),
            None => return,
        }
    };
    if ch.closed.load(Ordering::SeqCst) {
        return;
    }
    let mut next_in = match ch.next_in.lock() {
        Ok(n) => n,
        Err(_) => return,
    };
    let mut pending = match ch.pending.lock() {
        Ok(p) => p,
        Err(_) => return,
    };
    pending.insert(seq, payload.to_vec());
    while let Some(buf) = pending.remove(&*next_in) {
        *next_in += 1;
        drop(next_in);
        drop(pending);
        if !buf.is_empty() {
            let mut stream = match ch.stream.try_clone() {
                Ok(s) => s,
                Err(_) => {
                    close_channel(tunnel, channel, true);
                    return;
                }
            };
            if stream.write_all(&buf).is_err() {
                close_channel(tunnel, channel, true);
                return;
            }
        }
        next_in = match ch.next_in.lock() {
            Ok(n) => n,
            Err(_) => return,
        };
        pending = match ch.pending.lock() {
            Ok(p) => p,
            Err(_) => return,
        };
    }
}

fn on_shell_data(
    tunnel: &TunnelRuntime,
    channel: u32,
    shell: &ShellChannel,
    seq: u32,
    payload: &[u8],
) {
    if shell.closed.load(Ordering::SeqCst) {
        return;
    }
    let mut next_in = match shell.next_in.lock() {
        Ok(n) => n,
        Err(_) => return,
    };
    let mut pending = match shell.pending.lock() {
        Ok(p) => p,
        Err(_) => return,
    };
    pending.insert(seq, payload.to_vec());
    while let Some(buf) = pending.remove(&*next_in) {
        *next_in += 1;
        let term_id = shell.term_id.clone();
        drop(next_in);
        drop(pending);
        if !buf.is_empty() {
            let data = B64.encode(&buf);
            emit_event(
                "p2pssh-term-data",
                serde_json::json!({ "termId": term_id, "data": data }),
            );
        }
        next_in = match shell.next_in.lock() {
            Ok(n) => n,
            Err(_) => return,
        };
        pending = match shell.pending.lock() {
            Ok(p) => p,
            Err(_) => return,
        };
    }
    let _ = tunnel; // silence when unused in some builds
    let _ = channel;
}

fn on_remote_close(tunnel: &TunnelRuntime, channel: u32) {
    if close_shell_channel(tunnel, channel, false) {
        return;
    }
    close_channel(tunnel, channel, false);
}

fn close_shell_channel(tunnel: &TunnelRuntime, channel: u32, notify: bool) -> bool {
    let shell = {
        let mut map = match tunnel.shells.lock() {
            Ok(m) => m,
            Err(_) => return false,
        };
        map.remove(&channel)
    };
    let Some(shell) = shell else {
        return false;
    };
    if !shell.closed.swap(true, Ordering::SeqCst) {
        if notify {
            let frame = encode_frame(OP_CLOSE, channel, 0, &[]);
            let _ = send_frame(tunnel.webrpc_session, &frame);
        }
        let term_id = shell.term_id.clone();
        if let Ok(mut map) = state().terms.lock() {
            map.remove(&term_id);
        }
        emit_event("p2pssh-term-exit", serde_json::json!({ "termId": term_id }));
    }
    true
}

fn close_channel(tunnel: &TunnelRuntime, channel: u32, notify: bool) {
    let ch = {
        let mut map = match tunnel.channels.lock() {
            Ok(m) => m,
            Err(_) => return,
        };
        map.remove(&channel)
    };
    let Some(ch) = ch else { return };
    if !ch.closed.swap(true, Ordering::SeqCst) {
        let _ = ch.stream.shutdown(Shutdown::Both);
    }
    if notify {
        let frame = encode_frame(OP_CLOSE, channel, 0, &[]);
        let _ = send_frame(tunnel.webrpc_session, &frame);
    }
}

fn pipe_local_to_remote(tunnel: Arc<TunnelRuntime>, channel: u32, ch: Arc<ChannelState>) {
    let mut stream = match ch.stream.try_clone() {
        Ok(s) => s,
        Err(_) => {
            close_channel(&tunnel, channel, true);
            return;
        }
    };
    let mut buf = [0u8; 32 * 1024];
    loop {
        if tunnel.stop.load(Ordering::SeqCst) || ch.closed.load(Ordering::SeqCst) {
            break;
        }
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                let seq = ch.next_out.fetch_add(1, Ordering::SeqCst);
                let frame = encode_frame(OP_DATA, channel, seq, &buf[..n]);
                if !send_frame(tunnel.webrpc_session, &frame) {
                    break;
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(5));
            }
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
    close_channel(&tunnel, channel, true);
}

fn accept_loop(tunnel: Arc<TunnelRuntime>, listener: TcpListener) {
    let _ = listener.set_nonblocking(false);
    while !tunnel.stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => {
                if tunnel.stop.load(Ordering::SeqCst) {
                    let _ = stream.shutdown(Shutdown::Both);
                    break;
                }
                let _ = stream.set_nodelay(true);
                let channel = tunnel.next_channel.fetch_add(1, Ordering::SeqCst);
                // Register waiter before SendData so a fast OPEN_ACK cannot be dropped.
                let wait = register_open_wait(&tunnel.entry_id, channel);
                let frame = encode_frame(OP_OPEN, channel, 0, &[]);
                if !send_frame(tunnel.webrpc_session, &frame) {
                    clear_open_wait(&tunnel.entry_id, channel);
                    let _ = stream.shutdown(Shutdown::Both);
                    eprintln!("p2pssh: open channel {channel} SendData failed");
                    continue;
                }
                if let Err(err) = wait_open_ack(&wait, &tunnel.entry_id, channel) {
                    eprintln!(
                        "p2pssh: open channel {channel} failed: {err} (check p2pssh-server and remote sshd)"
                    );
                    let _ = stream.shutdown(Shutdown::Both);
                    continue;
                }
                eprintln!(
                    "p2pssh: channel {channel} open ok (webrpc session {} local :{})",
                    tunnel.webrpc_session, tunnel.local_port
                );
                let ch = Arc::new(ChannelState {
                    stream,
                    closed: AtomicBool::new(false),
                    next_out: AtomicU32::new(0),
                    next_in: Mutex::new(0),
                    pending: Mutex::new(HashMap::new()),
                });
                if let Ok(mut map) = tunnel.channels.lock() {
                    map.insert(channel, ch.clone());
                }
                let t2 = tunnel.clone();
                thread::Builder::new()
                    .name(format!("p2pssh-ch-{channel}"))
                    .spawn(move || pipe_local_to_remote(t2, channel, ch))
                    .ok();
            }
            Err(_) => {
                if tunnel.stop.load(Ordering::SeqCst) {
                    break;
                }
                thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

fn stop_tunnel_inner(entry_id: &str, close_webrpc: bool) {
    // close terms for this entry
    let term_ids: Vec<String> = state()
        .terms
        .lock()
        .map(|m| {
            m.iter()
                .filter(|(_, t)| t.entry_id == entry_id)
                .map(|(id, _)| id.clone())
                .collect()
        })
        .unwrap_or_default();
    for id in term_ids {
        let _ = term_close_inner(&id);
    }

    let tunnel = {
        let mut map = match state().tunnels.lock() {
            Ok(m) => m,
            Err(_) => return,
        };
        map.remove(entry_id)
    };
    let Some(tunnel) = tunnel else { return };
    tunnel.stop.store(true, Ordering::SeqCst);
    if let Ok(mut by) = state().by_session.lock() {
        by.remove(&tunnel.webrpc_session);
    }
    // wake accept by connecting once
    let _ = TcpStream::connect(("127.0.0.1", tunnel.local_port));
    let channels: Vec<u32> = tunnel
        .channels
        .lock()
        .map(|m| m.keys().copied().collect())
        .unwrap_or_default();
    for ch in channels {
        close_channel(&tunnel, ch, false);
    }
    let shells: Vec<u32> = tunnel
        .shells
        .lock()
        .map(|m| m.keys().copied().collect())
        .unwrap_or_default();
    for ch in shells {
        close_shell_channel(&tunnel, ch, false);
    }
    if close_webrpc {
        let sid = tunnel.webrpc_session;
        thread::spawn(move || {
            webrpc::close_session_best_effort(sid);
        });
    }
    emit_event(
        "p2pssh-disconnected",
        serde_json::json!({ "id": entry_id }),
    );
}

pub(crate) fn on_session_dead(session_id: u32) {
    let entry_id = state()
        .by_session
        .lock()
        .ok()
        .and_then(|m| m.get(&session_id).cloned());
    if let Some(id) = entry_id {
        stop_tunnel_inner(&id, false);
    }
}

pub fn shutdown() {
    let ids: Vec<String> = state()
        .tunnels
        .lock()
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default();
    for id in ids {
        stop_tunnel_inner(&id, false);
    }
}

fn ssh_command_for(port: u16) -> String {
    format!("ssh -p {port} user@127.0.0.1")
}

// ---- persistence commands ----

#[tauri::command]
pub fn saved_p2pssh_list(owner_token: String) -> Result<Vec<SavedP2pSsh>, String> {
    let owner = owner_token.trim().to_string();
    if owner.is_empty() {
        return Err("owner-token-empty".into());
    }
    Ok(list_for(&load_file()?, &owner))
}

#[tauri::command]
pub fn saved_p2pssh_create(
    owner_token: String,
    peer_token: String,
    peer_pass: String,
    remark: String,
) -> Result<Vec<SavedP2pSsh>, String> {
    let owner = owner_token.trim().to_string();
    let peer = peer_token.trim().to_string();
    let peer_pass = peer_pass.trim().to_string();
    let remark = remark.trim().to_string();
    if owner.is_empty() {
        return Err("owner-token-empty".into());
    }
    if peer.is_empty() {
        return Err("peer-token-empty".into());
    }
    let mut data = load_file()?;
    let list = data.accounts.entry(owner.clone()).or_default();
    if list.iter().any(|i| i.peer_token == peer) {
        return Err("p2pssh-exists".into());
    }
    list.insert(
        0,
        SavedP2pSsh {
            id: now_id("ssh"),
            peer_token: peer,
            peer_pass,
            remark,
        },
    );
    let result = list.clone();
    save_file(&data)?;
    Ok(result)
}

#[tauri::command]
pub fn saved_p2pssh_update(
    owner_token: String,
    id: String,
    peer_token: String,
    peer_pass: String,
    remark: String,
) -> Result<Vec<SavedP2pSsh>, String> {
    let owner = owner_token.trim().to_string();
    let id = id.trim().to_string();
    let peer = peer_token.trim().to_string();
    let peer_pass = peer_pass.trim().to_string();
    let remark = remark.trim().to_string();
    if owner.is_empty() || id.is_empty() || peer.is_empty() {
        return Err("args-empty".into());
    }
    let mut data = load_file()?;
    let list = data.accounts.entry(owner).or_default();
    let Some(item) = list.iter_mut().find(|i| i.id == id) else {
        return Err("p2pssh-missing".into());
    };
    item.peer_token = peer;
    item.peer_pass = peer_pass;
    item.remark = remark;
    let result = list.clone();
    save_file(&data)?;
    Ok(result)
}

#[tauri::command]
pub fn saved_p2pssh_delete(owner_token: String, id: String) -> Result<Vec<SavedP2pSsh>, String> {
    let owner = owner_token.trim().to_string();
    let id = id.trim().to_string();
    if owner.is_empty() || id.is_empty() {
        return Err("args-empty".into());
    }
    stop_tunnel_inner(&id, true);
    let mut data = load_file()?;
    let list = data.accounts.entry(owner).or_default();
    list.retain(|i| i.id != id);
    let result = list.clone();
    save_file(&data)?;
    Ok(result)
}

#[tauri::command]
pub async fn p2pssh_connect(
    owner_token: String,
    id: String,
    peer_pass: Option<String>,
) -> Result<P2pSshConnectInfo, String> {
    let owner = owner_token.trim().to_string();
    let id = id.trim().to_string();
    if owner.is_empty() || id.is_empty() {
        return Err("args-empty".into());
    }
    if state()
        .tunnels
        .lock()
        .map(|m| m.contains_key(&id))
        .unwrap_or(false)
    {
        let t = state().tunnels.lock().ok().and_then(|m| m.get(&id).cloned());
        if let Some(t) = t {
            return Ok(P2pSshConnectInfo {
                id: id.clone(),
                local_port: t.local_port,
                ssh_command: ssh_command_for(t.local_port),
                webrpc_session_id: t.webrpc_session,
            });
        }
    }

    let item = {
        let data = load_file()?;
        list_for(&data, &owner)
            .into_iter()
            .find(|i| i.id == id)
            .ok_or_else(|| "p2pssh-missing".to_string())?
    };
    let pass = peer_pass
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| item.peer_pass.clone());

    let peer = item.peer_token.clone();
    let session_id = webrpc::open_session_async(peer, pass).await?;
    if session_id == 0 {
        return Err("open-session-failed".into());
    }

    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| format!("bind failed: {e}"))?;
    let local_port = listener
        .local_addr()
        .map_err(|e| format!("local_addr: {e}"))?
        .port();
    let _ = listener.set_nonblocking(false);

    let tunnel = Arc::new(TunnelRuntime {
        entry_id: id.clone(),
        webrpc_session: session_id,
        local_port,
        stop: Arc::new(AtomicBool::new(false)),
        channels: Mutex::new(HashMap::new()),
        shells: Mutex::new(HashMap::new()),
        next_channel: AtomicU32::new(1),
    });

    if let Ok(mut map) = state().tunnels.lock() {
        map.insert(id.clone(), tunnel.clone());
    }
    if let Ok(mut by) = state().by_session.lock() {
        by.insert(session_id, id.clone());
    }

    let t2 = tunnel.clone();
    thread::Builder::new()
        .name(format!("p2pssh-accept-{local_port}"))
        .spawn(move || accept_loop(t2, listener))
        .map_err(|e| format!("spawn accept: {e}"))?;

    Ok(P2pSshConnectInfo {
        id,
        local_port,
        ssh_command: ssh_command_for(local_port),
        webrpc_session_id: session_id,
    })
}

#[tauri::command]
pub fn p2pssh_disconnect(id: String) -> Result<(), String> {
    let id = id.trim().to_string();
    if id.is_empty() {
        return Ok(());
    }
    stop_tunnel_inner(&id, true);
    Ok(())
}

#[tauri::command]
pub fn p2pssh_status(id: String) -> Result<serde_json::Value, String> {
    let id = id.trim().to_string();
    let connected = state()
        .tunnels
        .lock()
        .map(|m| m.contains_key(&id))
        .unwrap_or(false);
    if let Ok(map) = state().tunnels.lock() {
        if let Some(t) = map.get(&id) {
            return Ok(serde_json::json!({
                "id": id,
                "connected": true,
                "localPort": t.local_port,
                "sshCommand": ssh_command_for(t.local_port),
                "webrpcSessionId": t.webrpc_session,
            }));
        }
    }
    Ok(serde_json::json!({
        "id": id,
        "connected": connected,
        "localPort": 0,
        "sshCommand": "",
        "webrpcSessionId": 0,
    }))
}

// ---- in-app terminal (remote PTY shell over webrpc) ----

fn encode_resize(cols: u16, rows: u16) -> Vec<u8> {
    let mut b = Vec::with_capacity(4);
    b.extend_from_slice(&cols.to_le_bytes());
    b.extend_from_slice(&rows.to_le_bytes());
    b
}

#[tauri::command]
pub fn p2pssh_term_open(id: String, cols: u16, rows: u16) -> Result<String, String> {
    let id = id.trim().to_string();
    let tunnel = state()
        .tunnels
        .lock()
        .ok()
        .and_then(|m| m.get(&id).cloned())
        .ok_or_else(|| "not-connected".to_string())?;

    let cols = cols.max(20);
    let rows = rows.max(8);
    let channel = tunnel.next_channel.fetch_add(1, Ordering::SeqCst);
    eprintln!(
        "p2pssh: shell_open entry={id} channel={channel} session={} size={cols}x{rows}",
        tunnel.webrpc_session
    );

    let wait = register_open_wait(&tunnel.entry_id, channel);
    let frame = encode_frame(OP_OPEN, channel, 0, OPEN_SHELL);
    if !send_frame(tunnel.webrpc_session, &frame) {
        clear_open_wait(&tunnel.entry_id, channel);
        return Err("open-send-failed".into());
    }
    if let Err(err) = wait_open_ack(&wait, &tunnel.entry_id, channel) {
        eprintln!("p2pssh: shell_open channel={channel} failed: {err}");
        return Err(format!("remote shell open failed: {err}"));
    }

    let term_id = now_id("term");
    let shell = Arc::new(ShellChannel {
        term_id: term_id.clone(),
        closed: AtomicBool::new(false),
        next_out: AtomicU32::new(0),
        next_in: Mutex::new(0),
        pending: Mutex::new(HashMap::new()),
    });
    if let Ok(mut map) = tunnel.shells.lock() {
        map.insert(channel, shell);
    }

    let runtime = Arc::new(TermRuntime {
        entry_id: id,
        channel,
        webrpc_session: tunnel.webrpc_session,
        stop: Arc::new(AtomicBool::new(false)),
    });
    if let Ok(mut map) = state().terms.lock() {
        map.insert(term_id.clone(), runtime);
    }

    let resize = encode_frame(OP_RESIZE, channel, 0, &encode_resize(cols, rows));
    let _ = send_frame(tunnel.webrpc_session, &resize);
    eprintln!("p2pssh: shell_open ok term={term_id} channel={channel}");
    Ok(term_id)
}

#[tauri::command]
pub fn p2pssh_term_write(term_id: String, data: String) -> Result<(), String> {
    let term_id = term_id.trim().to_string();
    let bytes = B64
        .decode(data.trim().as_bytes())
        .map_err(|e| format!("b64: {e}"))?;
    if bytes.is_empty() {
        return Ok(());
    }
    let term = state()
        .terms
        .lock()
        .ok()
        .and_then(|m| m.get(&term_id).cloned())
        .ok_or_else(|| "term-missing".to_string())?;
    if term.stop.load(Ordering::SeqCst) {
        return Err("term-closed".into());
    }
    let tunnel = state()
        .tunnels
        .lock()
        .ok()
        .and_then(|m| m.get(&term.entry_id).cloned())
        .ok_or_else(|| "not-connected".to_string())?;
    let shell = tunnel
        .shells
        .lock()
        .ok()
        .and_then(|m| m.get(&term.channel).cloned())
        .ok_or_else(|| "shell-missing".to_string())?;
    if shell.closed.load(Ordering::SeqCst) {
        return Err("shell-closed".into());
    }
    let seq = shell.next_out.fetch_add(1, Ordering::SeqCst);
    let frame = encode_frame(OP_DATA, term.channel, seq, &bytes);
    if !send_frame(term.webrpc_session, &frame) {
        return Err("send-failed".into());
    }
    Ok(())
}

#[tauri::command]
pub fn p2pssh_term_resize(term_id: String, cols: u16, rows: u16) -> Result<(), String> {
    let term_id = term_id.trim().to_string();
    let cols = cols.max(20);
    let rows = rows.max(8);
    let term = state()
        .terms
        .lock()
        .ok()
        .and_then(|m| m.get(&term_id).cloned())
        .ok_or_else(|| "term-missing".to_string())?;
    if term.stop.load(Ordering::SeqCst) {
        return Ok(());
    }
    let frame = encode_frame(OP_RESIZE, term.channel, 0, &encode_resize(cols, rows));
    if !send_frame(term.webrpc_session, &frame) {
        return Err("resize-send-failed".into());
    }
    Ok(())
}

#[tauri::command]
pub fn p2pssh_term_close(term_id: String) -> Result<(), String> {
    term_close_inner(term_id.trim())
}

fn term_close_inner(term_id: &str) -> Result<(), String> {
    let term = {
        let mut map = state().terms.lock().map_err(|_| "lock".to_string())?;
        map.remove(term_id)
    };
    let Some(term) = term else {
        return Ok(());
    };
    term.stop.store(true, Ordering::SeqCst);
    if let Some(tunnel) = state()
        .tunnels
        .lock()
        .ok()
        .and_then(|m| m.get(&term.entry_id).cloned())
    {
        close_shell_channel(&tunnel, term.channel, true);
    }
    Ok(())
}
