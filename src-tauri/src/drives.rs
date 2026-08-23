use crate::storage;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

const DRIVES_FILE: &str = "saved_drives.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedDrive {
    pub peer_token: String,
    #[serde(default)]
    pub peer_pass: String,
    #[serde(default)]
    pub remark: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct SavedDrivesFile {
    #[serde(default)]
    accounts: HashMap<String, Vec<SavedDrive>>,
}

fn drives_path() -> Result<PathBuf, String> {
    Ok(storage::app_data_subdir("drives")?.join(DRIVES_FILE))
}

fn load_file() -> Result<SavedDrivesFile, String> {
    let path = drives_path()?;
    if !path.exists() {
        return Ok(SavedDrivesFile::default());
    }
    let raw = fs::read_to_string(&path).map_err(|err| format!("读取网盘缓存失败: {err}"))?;
    if raw.trim().is_empty() {
        return Ok(SavedDrivesFile::default());
    }
    serde_json::from_str::<SavedDrivesFile>(&raw).map_err(|err| format!("解析网盘缓存失败: {err}"))
}

fn save_file(data: &SavedDrivesFile) -> Result<(), String> {
    let path = drives_path()?;
    let text = serde_json::to_string_pretty(data).map_err(|err| format!("序列化网盘缓存失败: {err}"))?;
    fs::write(&path, text).map_err(|err| format!("写入网盘缓存失败: {err}"))?;
    Ok(())
}

fn account_drives<'a>(data: &'a mut SavedDrivesFile, owner: &str) -> &'a mut Vec<SavedDrive> {
    data.accounts.entry(owner.to_string()).or_default()
}

fn list_for(data: &SavedDrivesFile, owner: &str) -> Vec<SavedDrive> {
    data.accounts.get(owner).cloned().unwrap_or_default()
}

#[tauri::command]
pub fn saved_drives_list(owner_token: String) -> Result<Vec<SavedDrive>, String> {
    let owner = owner_token.trim().to_string();
    if owner.is_empty() {
        return Err("owner-token-empty".into());
    }
    Ok(list_for(&load_file()?, &owner))
}

#[tauri::command]
pub fn saved_drives_create(
    owner_token: String,
    peer_token: String,
    peer_pass: String,
    remark: String,
) -> Result<Vec<SavedDrive>, String> {
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
    let drives = account_drives(&mut data, &owner);
    if drives.iter().any(|item| item.peer_token == peer) {
        return Err("drive-exists".into());
    }
    drives.insert(
        0,
        SavedDrive {
            peer_token: peer,
            peer_pass,
            remark,
        },
    );
    let result = drives.clone();
    save_file(&data)?;
    Ok(result)
}

#[tauri::command]
pub fn saved_drives_update(
    owner_token: String,
    peer_token: String,
    peer_pass: String,
    remark: String,
) -> Result<Vec<SavedDrive>, String> {
    let owner = owner_token.trim().to_string();
    let peer = peer_token.trim().to_string();
    let peer_pass = peer_pass.trim().to_string();
    let remark = remark.trim().to_string();
    if owner.is_empty() || peer.is_empty() {
        return Err("token-empty".into());
    }

    let mut data = load_file()?;
    let drives = account_drives(&mut data, &owner);
    let Some(item) = drives.iter_mut().find(|item| item.peer_token == peer) else {
        return Err("drive-missing".into());
    };
    item.peer_pass = peer_pass;
    item.remark = remark;
    let result = drives.clone();
    save_file(&data)?;
    Ok(result)
}

#[tauri::command]
pub fn saved_drives_delete(owner_token: String, peer_token: String) -> Result<Vec<SavedDrive>, String> {
    let owner = owner_token.trim().to_string();
    let peer = peer_token.trim().to_string();
    if owner.is_empty() || peer.is_empty() {
        return Err("token-empty".into());
    }

    let mut data = load_file()?;
    let drives = account_drives(&mut data, &owner);
    drives.retain(|item| item.peer_token != peer);
    let result = drives.clone();
    save_file(&data)?;
    Ok(result)
}
