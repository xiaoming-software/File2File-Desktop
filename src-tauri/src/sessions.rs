use crate::storage;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

const SESSIONS_FILE: &str = "saved_sessions.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedSession {
    pub peer_token: String,
    #[serde(default)]
    pub peer_pass: String,
    #[serde(default)]
    pub remark: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct SavedSessionsFile {
    #[serde(default)]
    accounts: HashMap<String, Vec<SavedSession>>,
}

fn sessions_path() -> Result<PathBuf, String> {
    Ok(storage::app_data_dir()?.join(SESSIONS_FILE))
}

fn load_file() -> Result<SavedSessionsFile, String> {
    let path = sessions_path()?;
    if !path.exists() {
        return Ok(SavedSessionsFile::default());
    }
    let raw = fs::read_to_string(&path).map_err(|err| format!("读取会话缓存失败: {err}"))?;
    if raw.trim().is_empty() {
        return Ok(SavedSessionsFile::default());
    }
    serde_json::from_str::<SavedSessionsFile>(&raw).map_err(|err| format!("解析会话缓存失败: {err}"))
}

fn save_file(data: &SavedSessionsFile) -> Result<(), String> {
    let path = sessions_path()?;
    let text = serde_json::to_string_pretty(data).map_err(|err| format!("序列化会话缓存失败: {err}"))?;
    fs::write(&path, text).map_err(|err| format!("写入会话缓存失败: {err}"))?;
    Ok(())
}

fn account_sessions<'a>(data: &'a mut SavedSessionsFile, owner: &str) -> &'a mut Vec<SavedSession> {
    data.accounts.entry(owner.to_string()).or_default()
}

fn list_for(data: &SavedSessionsFile, owner: &str) -> Vec<SavedSession> {
    data.accounts.get(owner).cloned().unwrap_or_default()
}

#[tauri::command]
pub fn saved_sessions_list(owner_token: String) -> Result<Vec<SavedSession>, String> {
    let owner = owner_token.trim().to_string();
    if owner.is_empty() {
        return Err("owner-token-empty".into());
    }
    Ok(list_for(&load_file()?, &owner))
}

#[tauri::command]
pub fn saved_sessions_create(
    owner_token: String,
    peer_token: String,
    peer_pass: String,
    remark: String,
) -> Result<Vec<SavedSession>, String> {
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
    let sessions = account_sessions(&mut data, &owner);
    if sessions.iter().any(|item| item.peer_token == peer) {
        return Err("session-exists".into());
    }
    sessions.insert(
        0,
        SavedSession {
            peer_token: peer,
            peer_pass,
            remark,
        },
    );
    let result = sessions.clone();
    save_file(&data)?;
    Ok(result)
}

#[tauri::command]
pub fn saved_sessions_update(
    owner_token: String,
    peer_token: String,
    peer_pass: String,
    remark: String,
) -> Result<Vec<SavedSession>, String> {
    let owner = owner_token.trim().to_string();
    let peer = peer_token.trim().to_string();
    let peer_pass = peer_pass.trim().to_string();
    let remark = remark.trim().to_string();
    if owner.is_empty() || peer.is_empty() {
        return Err("token-empty".into());
    }

    let mut data = load_file()?;
    let sessions = account_sessions(&mut data, &owner);
    let Some(item) = sessions.iter_mut().find(|item| item.peer_token == peer) else {
        return Err("session-missing".into());
    };
    item.peer_pass = peer_pass;
    item.remark = remark;
    let result = sessions.clone();
    save_file(&data)?;
    Ok(result)
}

#[tauri::command]
pub fn saved_sessions_delete(
    owner_token: String,
    peer_token: String,
) -> Result<Vec<SavedSession>, String> {
    let owner = owner_token.trim().to_string();
    let peer = peer_token.trim().to_string();
    if owner.is_empty() || peer.is_empty() {
        return Err("token-empty".into());
    }

    let mut data = load_file()?;
    let sessions = account_sessions(&mut data, &owner);
    sessions.retain(|item| item.peer_token != peer);
    let result = sessions.clone();
    save_file(&data)?;
    Ok(result)
}
