use crate::storage;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const CHATS_FILE: &str = "saved_chats.json";
static CHATS_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedMessage {
    pub id: String,
    pub from: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub time: i64,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub transferred: u64,
    #[serde(default)]
    pub elapsed_ms: u64,
    #[serde(default)]
    pub speed_bps: u64,
    #[serde(default)]
    pub file_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct SavedChatsFile {
    /// ownerToken -> peerToken -> ordered messages
    #[serde(default)]
    accounts: HashMap<String, HashMap<String, Vec<SavedMessage>>>,
}

fn chats_path() -> Result<PathBuf, String> {
    Ok(storage::app_data_dir()?.join(CHATS_FILE))
}

fn load_file() -> Result<SavedChatsFile, String> {
    let path = chats_path()?;
    if !path.exists() {
        return Ok(SavedChatsFile::default());
    }
    let raw = fs::read_to_string(&path).map_err(|err| format!("读取聊天缓存失败: {err}"))?;
    if raw.trim().is_empty() {
        return Ok(SavedChatsFile::default());
    }
    match serde_json::from_str::<SavedChatsFile>(&raw) {
        Ok(data) => Ok(data),
        Err(first_err) => {
            let mut iter = serde_json::Deserializer::from_str(&raw).into_iter::<SavedChatsFile>();
            match iter.next() {
                Some(Ok(data)) => {
                    let _ = save_file(&data);
                    Ok(data)
                }
                _ => Err(format!("解析聊天缓存失败: {first_err}")),
            }
        }
    }
}

fn save_file(data: &SavedChatsFile) -> Result<(), String> {
    let path = chats_path()?;
    let text = serde_json::to_string_pretty(data).map_err(|err| format!("序列化聊天缓存失败: {err}"))?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, &text).map_err(|err| format!("写入聊天缓存失败: {err}"))?;
    fs::rename(&tmp, &path).map_err(|err| {
        let _ = fs::remove_file(&tmp);
        format!("写入聊天缓存失败: {err}")
    })?;
    Ok(())
}

fn lock_chats() -> Result<std::sync::MutexGuard<'static, ()>, String> {
    Ok(CHATS_LOCK.lock().unwrap_or_else(|err| err.into_inner()))
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn is_success_status(status: &str) -> bool {
    status == "sent" || status == "received"
}

fn can_replace_status(old: &str, new: &str) -> bool {
    if old == new {
        return true;
    }
    if old == "failed" {
        return true;
    }
    if old == "sent" {
        return false;
    }
    if old == "received" {
        return new == "receiving";
    }
    true
}

fn finalize_complete_file(item: &mut SavedMessage) {
    if item.kind == "text" || !is_success_status(&item.status) {
        return;
    }
    if item.size > 0 {
        item.transferred = item.size;
    }
    let elapsed = item.elapsed_ms.max(1);
    if item.elapsed_ms == 0 {
        item.elapsed_ms = elapsed;
    }
    item.speed_bps = item.transferred.saturating_mul(1000) / elapsed;
}

fn apply_complete_if_full(item: &mut SavedMessage) -> bool {
    if item.kind == "text" || item.status == "failed" {
        return false;
    }
    if item.size == 0 || item.transferred < item.size {
        return false;
    }
    let next = if item.from == "me" { "sent" } else { "received" };
    let changed = item.status != next;
    item.status = next.into();
    finalize_complete_file(item);
    changed
}

fn normalize_stale_status(messages: &mut [SavedMessage]) -> bool {
    let mut changed = false;
    for item in messages {
        if apply_complete_if_full(item) {
            changed = true;
            continue;
        }
        if is_success_status(&item.status) {
            let before = (item.transferred, item.elapsed_ms, item.speed_bps);
            finalize_complete_file(item);
            if before != (item.transferred, item.elapsed_ms, item.speed_bps) {
                changed = true;
            }
            continue;
        }
        if item.status == "sending" {
            item.status = "failed".into();
            changed = true;
        }
    }
    changed
}

/// Persist file transfer stats without clobbering an already-successful record.
pub fn persist_transfer(
    owner_token: &str,
    peer_token: &str,
    msg_id: &str,
    from: &str,
    title: &str,
    status: &str,
    size: u64,
    transferred: u64,
    elapsed_ms: u64,
    speed_bps: u64,
    file_path: &str,
) {
    if let Err(err) = persist_transfer_inner(
        owner_token,
        peer_token,
        msg_id,
        from,
        title,
        status,
        size,
        transferred,
        elapsed_ms,
        speed_bps,
        file_path,
    ) {
        eprintln!("chats: persist file transfer failed: {err}");
    }
}

fn persist_transfer_inner(
    owner_token: &str,
    peer_token: &str,
    msg_id: &str,
    from: &str,
    title: &str,
    status: &str,
    size: u64,
    transferred: u64,
    elapsed_ms: u64,
    speed_bps: u64,
    file_path: &str,
) -> Result<(), String> {
    let owner = owner_token.trim();
    let peer = peer_token.trim();
    if owner.is_empty() || peer.is_empty() {
        return Ok(());
    }
    let _guard = lock_chats()?;
    let mut data = load_file()?;
    let list = peer_messages(&mut data, owner, peer);
    let found = if !msg_id.trim().is_empty() {
        list.iter_mut().find(|item| item.id == msg_id)
    } else {
        list.iter_mut().rev().find(|item| {
            item.from == from && item.title == title && item.kind != "text"
        })
    };
    if let Some(item) = found {
        if !can_replace_status(&item.status, status) {
            return Ok(());
        }
        item.status = status.to_string();
        if size > 0 {
            item.size = size;
        }
        item.transferred = transferred;
        item.elapsed_ms = elapsed_ms;
        item.speed_bps = speed_bps;
        if !file_path.is_empty() {
            item.file_path = file_path.to_string();
        }
        if !title.is_empty() && item.title.is_empty() {
            item.title = title.to_string();
        }
        finalize_complete_file(item);
    } else if !msg_id.trim().is_empty() {
        let mut message = SavedMessage {
            id: msg_id.to_string(),
            from: from.to_string(),
            kind: "file".into(),
            content: String::new(),
            title: title.to_string(),
            time: now_ms(),
            status: status.to_string(),
            size,
            transferred,
            elapsed_ms,
            speed_bps,
            file_path: file_path.to_string(),
        };
        finalize_complete_file(&mut message);
        list.push(message);
    } else if from == "peer" && !title.is_empty() {
        let mut message = SavedMessage {
            id: format!("file-in-{title}"),
            from: "peer".into(),
            kind: "file".into(),
            content: String::new(),
            title: title.to_string(),
            time: now_ms(),
            status: status.to_string(),
            size,
            transferred,
            elapsed_ms,
            speed_bps,
            file_path: file_path.to_string(),
        };
        finalize_complete_file(&mut message);
        list.push(message);
    } else {
        return Ok(());
    }
    save_file(&data)
}

fn peer_messages<'a>(
    data: &'a mut SavedChatsFile,
    owner: &str,
    peer: &str,
) -> &'a mut Vec<SavedMessage> {
    data.accounts
        .entry(owner.to_string())
        .or_default()
        .entry(peer.to_string())
        .or_default()
}

fn upsert_message(list: &mut Vec<SavedMessage>, message: SavedMessage) {
    let mut message = message;
    finalize_complete_file(&mut message);
    if let Some(item) = list.iter_mut().find(|item| item.id == message.id) {
        if !can_replace_status(&item.status, &message.status) {
            return;
        }
        *item = message;
        return;
    }
    list.push(message);
}

#[tauri::command]
pub fn saved_chats_load(owner_token: String, peer_token: String) -> Result<Vec<SavedMessage>, String> {
    let owner = owner_token.trim().to_string();
    let peer = peer_token.trim().to_string();
    if owner.is_empty() || peer.is_empty() {
        return Err("token-empty".into());
    }
    let _guard = lock_chats()?;
    let mut data = load_file()?;
    let messages = peer_messages(&mut data, &owner, &peer);
    let changed = normalize_stale_status(messages);
    let result = messages.clone();
    if changed {
        save_file(&data)?;
    }
    Ok(result)
}

fn write_message(owner_token: String, peer_token: String, message: SavedMessage) -> Result<(), String> {
    let owner = owner_token.trim().to_string();
    let peer = peer_token.trim().to_string();
    if owner.is_empty() || peer.is_empty() {
        return Err("token-empty".into());
    }
    if message.id.trim().is_empty() {
        return Err("message-id-empty".into());
    }
    let _guard = lock_chats()?;
    let mut data = load_file()?;
    upsert_message(peer_messages(&mut data, &owner, &peer), message);
    save_file(&data)
}

#[tauri::command]
pub fn saved_chats_append(
    owner_token: String,
    peer_token: String,
    message: SavedMessage,
) -> Result<(), String> {
    write_message(owner_token, peer_token, message)
}

#[tauri::command]
pub fn saved_chats_update(
    owner_token: String,
    peer_token: String,
    message: SavedMessage,
) -> Result<(), String> {
    write_message(owner_token, peer_token, message)
}

#[tauri::command]
pub fn saved_chats_delete_message(
    owner_token: String,
    peer_token: String,
    message_id: String,
) -> Result<(), String> {
    let owner = owner_token.trim().to_string();
    let peer = peer_token.trim().to_string();
    let message_id = message_id.trim().to_string();
    if owner.is_empty() || peer.is_empty() {
        return Err("token-empty".into());
    }
    if message_id.is_empty() {
        return Err("message-id-empty".into());
    }
    let _guard = lock_chats()?;
    let mut data = load_file()?;
    let list = peer_messages(&mut data, &owner, &peer);
    let Some(index) = list.iter().position(|item| item.id == message_id) else {
        return Ok(());
    };
    if list[index].status == "sending" || list[index].status == "receiving" {
        return Err("transfer-in-progress".into());
    }
    let removed = list.remove(index);
    let cache_path = if removed.from != "me"
        && removed.kind != "text"
        && !removed.file_path.trim().is_empty()
        && !list.iter().any(|item| item.file_path == removed.file_path)
    {
        Some(removed.file_path.clone())
    } else {
        None
    };
    save_file(&data)?;
    drop(_guard);
    if let Some(path) = cache_path {
        crate::storage::delete_session_cache_file(&owner, &peer, &path)?;
    }
    Ok(())
}

#[tauri::command]
pub fn saved_chats_clear(owner_token: String, peer_token: String) -> Result<(), String> {
    let owner = owner_token.trim().to_string();
    let peer = peer_token.trim().to_string();
    if owner.is_empty() || peer.is_empty() {
        return Err("token-empty".into());
    }
    let _guard = lock_chats()?;
    let mut data = load_file()?;
    if let Some(peers) = data.accounts.get_mut(&owner) {
        if let Some(list) = peers.get_mut(&peer) {
            list.clear();
        }
    }
    save_file(&data)?;
    drop(_guard);
    crate::storage::delete_session_files(&owner, &peer)
}

#[tauri::command]
pub fn saved_chats_delete(owner_token: String, peer_token: String) -> Result<(), String> {
    let owner = owner_token.trim().to_string();
    let peer = peer_token.trim().to_string();
    if owner.is_empty() || peer.is_empty() {
        return Err("token-empty".into());
    }
    let _guard = lock_chats()?;
    let mut data = load_file()?;
    if let Some(peers) = data.accounts.get_mut(&owner) {
        peers.remove(&peer);
        if peers.is_empty() {
            data.accounts.remove(&owner);
        }
    }
    save_file(&data)?;
    drop(_guard);
    crate::storage::delete_session_files(&owner, &peer)
}
