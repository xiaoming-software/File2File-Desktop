//! Local HTTP media proxy for drive video/audio preview.
//!
//! Lifecycle: start after first successful backPath; stop when the drive session closes.
//! webrpc may reconnect underneath without tearing down the listener.
//! Requests use requestId matching because webrpc SendData is unordered.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use serde::Serialize;
use tauri::Emitter;

use crate::webrpc;

const READ_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_CHUNK: u64 = 1 << 20;
const LISTEN_HOST: &str = "127.0.0.1";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaProxyInfo {
    pub session_id: u32,
    pub port: u16,
    pub token: String,
    pub base_url: String,
}

struct PendingReply {
    tx: mpsc::Sender<serde_json::Value>,
}

struct ProxyState {
    port: u16,
    token: String,
    stop: Arc<AtomicBool>,
    join: Option<thread::JoinHandle<()>>,
}

#[derive(Default)]
struct ProxyHub {
    by_session: HashMap<u32, ProxyState>,
    pending: HashMap<String, PendingReply>,
    req_seq: AtomicU64,
}

static HUB: OnceLock<Mutex<ProxyHub>> = OnceLock::new();

fn hub() -> std::sync::MutexGuard<'static, ProxyHub> {
    HUB.get_or_init(|| Mutex::new(ProxyHub::default()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

fn new_token(session_id: u32) -> String {
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!(
        "{:08x}{:08x}{:08x}",
        session_id,
        t.as_secs() as u32,
        t.subsec_nanos() ^ session_id.wrapping_mul(0x85eb_ca6b)
    )
}

fn next_request_id(session_id: u32) -> String {
    let n = hub().req_seq.fetch_add(1, Ordering::Relaxed);
    format!("r{session_id}-{n}-{}", Instant::now().elapsed().as_nanos())
}

/// Start proxy after first successful directory listing (idempotent).
pub fn ensure_started(session_id: u32) {
    if session_id == 0 {
        return;
    }
    {
        let g = hub();
        if g.by_session.contains_key(&session_id) {
            return;
        }
    }
    let listener = match TcpListener::bind((LISTEN_HOST, 0)) {
        Ok(l) => l,
        Err(err) => {
            eprintln!("nas media proxy bind failed session={session_id}: {err}");
            return;
        }
    };
    let port = match listener.local_addr() {
        Ok(a) => a.port(),
        Err(err) => {
            eprintln!("nas media proxy addr failed session={session_id}: {err}");
            return;
        }
    };
    let _ = listener.set_nonblocking(false);
    let token = new_token(session_id);
    let stop = Arc::new(AtomicBool::new(false));
    let stop_t = stop.clone();
    let token_t = token.clone();
    let join = thread::Builder::new()
        .name(format!("nas-media-proxy-{session_id}"))
        .spawn(move || serve_loop(session_id, listener, token_t, stop_t))
        .ok();
    let info = MediaProxyInfo {
        session_id,
        port,
        token: token.clone(),
        base_url: format!("http://{LISTEN_HOST}:{port}"),
    };
    {
        let mut g = hub();
        g.by_session.insert(
            session_id,
            ProxyState {
                port,
                token,
                stop,
                join,
            },
        );
    }
    eprintln!(
        "nas media proxy up session={session_id} port={port} url={}",
        info.base_url
    );
    if let Some(app) = crate::webrpc::app_handle() {
        let _ = app.emit("webrpc-nas-media-proxy", &info);
    }
}

/// Stop and recycle when the drive session closes.
pub fn stop(session_id: u32) {
    if session_id == 0 {
        return;
    }
    let state = {
        let mut g = hub();
        // Drop pending waiters for this session.
        g.pending.retain(|k, _| !k.starts_with(&format!("r{session_id}-")));
        g.by_session.remove(&session_id)
    };
    let Some(state) = state else {
        return;
    };
    state.stop.store(true, Ordering::SeqCst);
    // Unblock accept with a dummy connect.
    let _ = TcpStream::connect((LISTEN_HOST, state.port));
    if let Some(j) = state.join {
        let _ = j.join();
    }
    eprintln!("nas media proxy stopped session={session_id}");
}

pub fn info(session_id: u32) -> Option<MediaProxyInfo> {
    let g = hub();
    g.by_session.get(&session_id).map(|s| MediaProxyInfo {
        session_id,
        port: s.port,
        token: s.token.clone(),
        base_url: format!("http://{LISTEN_HOST}:{}", s.port),
    })
}

pub fn stream_url(session_id: u32, remote_dir: &str, file_name: &str) -> Option<String> {
    let info = info(session_id)?;
    let path = join_nas_path(remote_dir, file_name);
    let enc = encode_path_segments(&path);
    Some(format!(
        "{}/media/{}/{}",
        info.base_url.trim_end_matches('/'),
        info.token,
        enc
    ))
}

#[tauri::command]
pub fn nas_media_proxy_info(session_id: u32) -> Option<MediaProxyInfo> {
    info(session_id)
}

#[tauri::command]
pub fn nas_media_stream_url(
    session_id: u32,
    remote_dir: String,
    file_name: String,
) -> Option<String> {
    stream_url(session_id, &remote_dir, &file_name)
}

pub fn on_back_stat(session_id: u32, value: &serde_json::Value) {
    fulfill(session_id, value);
}

pub fn on_back_range(session_id: u32, value: &serde_json::Value) {
    fulfill(session_id, value);
}

fn fulfill(_session_id: u32, value: &serde_json::Value) {
    let rid = value
        .get("requestId")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if rid.is_empty() {
        return;
    }
    let pending = {
        let mut g = hub();
        g.pending.remove(&rid)
    };
    if let Some(p) = pending {
        let _ = p.tx.send(value.clone());
    }
}

fn serve_loop(session_id: u32, listener: TcpListener, token: String, stop: Arc<AtomicBool>) {
    let _ = listener.set_nonblocking(false);
    while !stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((mut stream, _)) => {
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                let tok = token.clone();
                thread::Builder::new()
                    .name(format!("nas-media-req-{session_id}"))
                    .spawn(move || {
                        if let Err(err) = handle_client(session_id, &tok, &mut stream) {
                            eprintln!("nas media proxy req err session={session_id}: {err}");
                        }
                    })
                    .ok();
            }
            Err(err) => {
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                eprintln!("nas media proxy accept: {err}");
                thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

fn handle_client(session_id: u32, token: &str, stream: &mut TcpStream) -> Result<(), String> {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(30)));
    let mut buf = vec![0u8; 64 * 1024];
    let n = stream.read(&mut buf).map_err(|e| e.to_string())?;
    if n == 0 {
        return Ok(());
    }
    let raw = String::from_utf8_lossy(&buf[..n]);
    let mut lines = raw.split("\r\n");
    let req_line = lines.next().unwrap_or("");
    let mut parts = req_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_uppercase();
    let path = parts.next().unwrap_or("");
    let mut range_hdr = None::<String>;
    for line in lines {
        if line.is_empty() {
            break;
        }
        let lower = line.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("range:") {
            range_hdr = Some(rest.trim().to_string());
        }
    }

    if method == "OPTIONS" {
        return write_response(
            stream,
            204,
            "No Content",
            &[
                ("Access-Control-Allow-Origin", "*"),
                ("Access-Control-Allow-Methods", "GET, HEAD, OPTIONS"),
                ("Access-Control-Allow-Headers", "Range, Content-Type"),
                ("Access-Control-Expose-Headers", "Content-Range, Accept-Ranges, Content-Length"),
                ("Access-Control-Max-Age", "86400"),
            ],
            &[],
        );
    }

    if method != "GET" && method != "HEAD" {
        return write_response(stream, 405, "Method Not Allowed", &[], b"method");
    }

    let (got_token, nas_path) = match parse_media_path(path) {
        Some(v) => v,
        None => return write_response(stream, 404, "Not Found", &[], b"not found"),
    };
    if got_token != token {
        return write_response(stream, 403, "Forbidden", &[], b"forbidden");
    }
    let (file_path, file_name) = split_nas_path(&nas_path);
    if file_name.is_empty() {
        return write_response(stream, 400, "Bad Request", &[], b"bad path");
    }

    let stat = match rpc_stat(session_id, &file_path, &file_name) {
        Ok(v) => v,
        Err(_) => {
            return write_response(stream, 503, "Service Unavailable", &cors_headers(), b"unavailable")
        }
    };
    if !stat.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
        return write_response(stream, 404, "Not Found", &cors_headers(), b"missing");
    }
    let size = stat.get("size").and_then(|v| v.as_u64()).unwrap_or(0);
    let mime = stat
        .get("mime")
        .and_then(|v| v.as_str())
        .unwrap_or("application/octet-stream")
        .to_string();

    let (start, end_incl) = match parse_range(range_hdr.as_deref(), size) {
        Ok(v) => v,
        Err(code) => {
            let headers = cors_headers_owned(vec![
                ("Content-Range".into(), format!("bytes */{size}")),
                ("Accept-Ranges".into(), "bytes".into()),
            ]);
            return write_response_owned(stream, code, "Range Not Satisfiable", &headers, &[]);
        }
    };

    if method == "HEAD" {
        let len = end_incl.saturating_sub(start).saturating_add(1);
        let mut headers = cors_headers_owned(vec![
            ("Accept-Ranges".into(), "bytes".into()),
            ("Content-Type".into(), mime.clone()),
            ("Content-Length".into(), len.to_string()),
        ]);
        let status = if range_hdr.is_some() { 206 } else { 200 };
        let reason = if status == 206 {
            "Partial Content"
        } else {
            "OK"
        };
        if status == 206 {
            headers.push((
                "Content-Range".into(),
                format!("bytes {start}-{end_incl}/{size}"),
            ));
        }
        return write_response_owned(stream, status, reason, &headers, &[]);
    }

    let mut body = Vec::new();
    let mut off = start;
    while off <= end_incl {
        let remain = end_incl - off + 1;
        let chunk = remain.min(MAX_CHUNK);
        match rpc_read(session_id, &file_path, &file_name, off, chunk) {
            Ok(bytes) => {
                if bytes.is_empty() {
                    break;
                }
                let n = bytes.len() as u64;
                body.extend_from_slice(&bytes);
                off = off.saturating_add(n);
                if n < chunk {
                    break;
                }
            }
            Err(_) => {
                if body.is_empty() {
                    return write_response(
                        stream,
                        503,
                        "Service Unavailable",
                        &cors_headers(),
                        b"read failed",
                    );
                }
                break;
            }
        }
    }

    let mut headers = cors_headers_owned(vec![
        ("Accept-Ranges".into(), "bytes".into()),
        ("Content-Type".into(), mime),
        ("Content-Length".into(), body.len().to_string()),
    ]);
    let status = if range_hdr.is_some() { 206 } else { 200 };
    let reason = if status == 206 {
        "Partial Content"
    } else {
        "OK"
    };
    if status == 206 {
        let end_actual = start.saturating_add(body.len() as u64).saturating_sub(1);
        headers.push((
            "Content-Range".into(),
            format!("bytes {start}-{end_actual}/{size}"),
        ));
    }
    write_response_owned(stream, status, reason, &headers, &body)
}

fn rpc_stat(session_id: u32, file_path: &str, file_name: &str) -> Result<serde_json::Value, String> {
    let rid = next_request_id(session_id);
    let (tx, rx) = mpsc::channel();
    {
        let mut g = hub();
        g.pending.insert(rid.clone(), PendingReply { tx });
    }
    let payload = serde_json::json!({
        "type": "statFile",
        "fileName": file_name,
        "filePath": file_path,
        "requestId": rid,
    });
    let ok = webrpc::send_json_timeout(
        session_id,
        &payload.to_string(),
        READ_TIMEOUT.as_millis() as i64,
    );
    if !ok {
        let mut g = hub();
        g.pending.remove(&rid);
        return Err("send failed".into());
    }
    match rx.recv_timeout(READ_TIMEOUT) {
        Ok(v) => Ok(v),
        Err(_) => {
            let mut g = hub();
            g.pending.remove(&rid);
            Err("timeout".into())
        }
    }
}

fn rpc_read(
    session_id: u32,
    file_path: &str,
    file_name: &str,
    offset: u64,
    length: u64,
) -> Result<Vec<u8>, String> {
    let rid = next_request_id(session_id);
    let (tx, rx) = mpsc::channel();
    {
        let mut g = hub();
        g.pending.insert(rid.clone(), PendingReply { tx });
    }
    let payload = serde_json::json!({
        "type": "readRange",
        "fileName": file_name,
        "filePath": file_path,
        "offset": offset,
        "length": length,
        "requestId": rid,
    });
    let ok = webrpc::send_json_timeout(
        session_id,
        &payload.to_string(),
        READ_TIMEOUT.as_millis() as i64,
    );
    if !ok {
        let mut g = hub();
        g.pending.remove(&rid);
        return Err("send failed".into());
    }
    let value = match rx.recv_timeout(READ_TIMEOUT) {
        Ok(v) => v,
        Err(_) => {
            let mut g = hub();
            g.pending.remove(&rid);
            return Err("timeout".into());
        }
    };
    if !value.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
        return Err("not ok".into());
    }
    let data = value.get("data").and_then(|v| v.as_str()).unwrap_or("");
    B64.decode(data).map_err(|e| e.to_string())
}

fn parse_media_path(path: &str) -> Option<(String, String)> {
    // /media/{token}/rest/of/path
    let p = path.split('?').next().unwrap_or(path);
    let mut segs: Vec<&str> = p.split('/').filter(|s| !s.is_empty()).collect();
    if segs.len() < 3 || segs[0] != "media" {
        return None;
    }
    let token = segs[1].to_string();
    segs.drain(0..2);
    let decoded: Vec<String> = segs
        .into_iter()
        .map(|s| percent_decode(s))
        .collect();
    if decoded.is_empty() {
        return None;
    }
    Some((token, decoded.join("/")))
}

fn split_nas_path(path: &str) -> (String, String) {
    let trimmed = path.trim().trim_start_matches('/');
    if trimmed.is_empty() {
        return ("/".into(), String::new());
    }
    if let Some(i) = trimmed.rfind('/') {
        let dir = format!("/{}", &trimmed[..i]);
        let name = trimmed[i + 1..].to_string();
        (if dir == "/" { "/".into() } else { dir }, name)
    } else {
        ("/".into(), trimmed.to_string())
    }
}

fn join_nas_path(dir: &str, name: &str) -> String {
    let d = dir.trim().trim_end_matches('/');
    let n = name.trim().trim_start_matches('/');
    if d.is_empty() || d == "/" {
        n.to_string()
    } else {
        format!("{}/{n}", d.trim_start_matches('/'))
    }
}

fn encode_path_segments(path: &str) -> String {
    path.trim()
        .trim_start_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .map(percent_encode)
        .collect::<Vec<_>>()
        .join("/")
}

fn percent_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.as_bytes() {
        match *b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let h = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
            if let Ok(v) = u8::from_str_radix(h, 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn parse_range(header: Option<&str>, size: u64) -> Result<(u64, u64), u16> {
    if size == 0 {
        return Ok((0, 0));
    }
    let Some(h) = header else {
        return Ok((0, size - 1));
    };
    let h = h.trim();
    let rest = h.strip_prefix("bytes=").unwrap_or(h);
    let (a, b) = match rest.split_once('-') {
        Some(p) => p,
        None => return Err(416),
    };
    if a.is_empty() {
        // bytes=-N suffix
        let n: u64 = match b.parse() {
            Ok(v) => v,
            Err(_) => return Err(416),
        };
        if n == 0 {
            return Err(416);
        }
        let start = size.saturating_sub(n);
        return Ok((start, size - 1));
    }
    let start: u64 = match a.parse() {
        Ok(v) => v,
        Err(_) => return Err(416),
    };
    if start >= size {
        return Err(416);
    }
    let end = if b.is_empty() {
        size - 1
    } else {
        match b.parse::<u64>() {
            Ok(v) => v.min(size - 1),
            Err(_) => return Err(416),
        }
    };
    if end < start {
        return Err(416);
    }
    Ok((start, end))
}

fn cors_headers() -> [(&'static str, &'static str); 4] {
    [
        ("Access-Control-Allow-Origin", "*"),
        ("Access-Control-Allow-Methods", "GET, HEAD, OPTIONS"),
        ("Access-Control-Expose-Headers", "Content-Range, Accept-Ranges, Content-Length"),
        ("Accept-Ranges", "bytes"),
    ]
}

fn cors_headers_owned(extra: Vec<(String, String)>) -> Vec<(String, String)> {
    let mut h = vec![
        ("Access-Control-Allow-Origin".into(), "*".into()),
        ("Access-Control-Allow-Methods".into(), "GET, HEAD, OPTIONS".into()),
        (
            "Access-Control-Expose-Headers".into(),
            "Content-Range, Accept-Ranges, Content-Length".into(),
        ),
    ];
    h.extend(extra);
    h
}

fn write_response(
    stream: &mut TcpStream,
    code: u16,
    reason: &str,
    headers: &[(&str, &str)],
    body: &[u8],
) -> Result<(), String> {
    let owned: Vec<(String, String)> = headers
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect();
    write_response_owned(stream, code, reason, &owned, body)
}

fn write_response_owned(
    stream: &mut TcpStream,
    code: u16,
    reason: &str,
    headers: &[(String, String)],
    body: &[u8],
) -> Result<(), String> {
    let mut out = format!("HTTP/1.1 {code} {reason}\r\nConnection: close\r\n");
    let mut has_len = false;
    for (k, v) in headers {
        if k.eq_ignore_ascii_case("Content-Length") {
            has_len = true;
        }
        out.push_str(k);
        out.push_str(": ");
        out.push_str(v);
        out.push_str("\r\n");
    }
    if !has_len {
        out.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    out.push_str("\r\n");
    stream.write_all(out.as_bytes()).map_err(|e| e.to_string())?;
    if !body.is_empty() {
        stream.write_all(body).map_err(|e| e.to_string())?;
    }
    let _ = stream.flush();
    Ok(())
}
