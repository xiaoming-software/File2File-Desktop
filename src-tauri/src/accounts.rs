use crate::storage;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const ACCOUNTS_FILE: &str = "saved_accounts.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedAccount {
    pub token: String,
    pub password: String,
    #[serde(default)]
    pub passphrase: String,
    #[serde(default)]
    pub remark: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct SavedAccountsFile {
    #[serde(default)]
    accounts: Vec<SavedAccount>,
}

fn accounts_path() -> Result<PathBuf, String> {
    Ok(storage::app_data_dir()?.join(ACCOUNTS_FILE))
}

fn load_file() -> Result<SavedAccountsFile, String> {
    let path = accounts_path()?;
    if !path.exists() {
        return Ok(SavedAccountsFile::default());
    }
    let raw = fs::read_to_string(&path).map_err(|err| format!("读取账号缓存失败: {err}"))?;
    if raw.trim().is_empty() {
        return Ok(SavedAccountsFile::default());
    }
    serde_json::from_str::<SavedAccountsFile>(&raw).map_err(|err| format!("解析账号缓存失败: {err}"))
}

fn save_file(data: &SavedAccountsFile) -> Result<(), String> {
    let path = accounts_path()?;
    let text = serde_json::to_string_pretty(data).map_err(|err| format!("序列化账号缓存失败: {err}"))?;
    fs::write(&path, text).map_err(|err| format!("写入账号缓存失败: {err}"))?;
    Ok(())
}

#[tauri::command]
pub fn saved_accounts_list() -> Result<Vec<SavedAccount>, String> {
    Ok(load_file()?.accounts)
}

#[tauri::command]
pub fn saved_accounts_upsert(
    token: String,
    password: String,
    passphrase: String,
) -> Result<Vec<SavedAccount>, String> {
    let token = token.trim().to_string();
    let password = password.trim().to_string();
    let passphrase = passphrase.trim().to_string();
    if token.is_empty() || password.is_empty() {
        return Err("token-or-password-empty".into());
    }

    let mut data = load_file()?;
    let prev_remark = data
        .accounts
        .iter()
        .find(|item| item.token == token)
        .map(|item| item.remark.clone())
        .unwrap_or_default();
    data.accounts.retain(|item| item.token != token);
    data.accounts.insert(
        0,
        SavedAccount {
            token,
            password,
            passphrase,
            remark: prev_remark,
        },
    );
    save_file(&data)?;
    Ok(data.accounts)
}

#[tauri::command]
pub fn saved_accounts_delete(token: String) -> Result<Vec<SavedAccount>, String> {
    let token = token.trim().to_string();
    let mut data = load_file()?;
    data.accounts.retain(|item| item.token != token);
    save_file(&data)?;
    Ok(data.accounts)
}

#[tauri::command]
pub fn saved_accounts_update_remark(
    token: String,
    remark: String,
) -> Result<Vec<SavedAccount>, String> {
    let token = token.trim().to_string();
    let remark = remark.trim().to_string();
    if token.is_empty() {
        return Err("token-empty".into());
    }

    let mut data = load_file()?;
    let item = data
        .accounts
        .iter_mut()
        .find(|item| item.token == token)
        .ok_or_else(|| "account-not-found".to_string())?;
    item.remark = remark;
    save_file(&data)?;
    Ok(data.accounts)
}
