use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

const APP_DATA_DIR_NAME: &str = "file2file_data";

pub fn user_workspace_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Some(path) = std::env::var_os("USERPROFILE") {
            return PathBuf::from(path);
        }
        if let Some(drive) = std::env::var_os("HOMEDRIVE") {
            if let Some(home_path) = std::env::var_os("HOMEPATH") {
                return PathBuf::from(format!(
                    "{}{}",
                    drive.to_string_lossy(),
                    home_path.to_string_lossy()
                ));
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Some(path) = std::env::var_os("HOME") {
            return PathBuf::from(path);
        }
    }

    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

pub fn app_data_dir() -> Result<PathBuf, String> {
    let dir = user_workspace_dir().join(APP_DATA_DIR_NAME);
    fs::create_dir_all(&dir).map_err(|err| format!("创建 {APP_DATA_DIR_NAME} 失败: {err}"))?;
    Ok(dir)
}

pub fn sanitize_component(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "_".into();
    }
    let mut out = String::new();
    for byte in trimmed.as_bytes() {
        let ch = *byte as char;
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '@') {
            out.push(ch);
        } else {
            out.push_str(&format!("_{:02x}", byte));
        }
    }
    if out.is_empty() {
        "_".into()
    } else {
        out
    }
}

pub fn safe_filename(name: &str) -> String {
    let base = Path::new(name)
        .file_name()
        .and_then(|item| item.to_str())
        .unwrap_or("file")
        .replace(['\\', '/', '\0'], "_");
    let cleaned = base.trim();
    if cleaned.is_empty() || cleaned == "." || cleaned == ".." {
        "file".into()
    } else {
        cleaned.to_string()
    }
}

pub fn session_files_dir(owner: &str, peer: &str) -> Result<PathBuf, String> {
    let dir = session_files_dir_path(owner, peer)?;
    fs::create_dir_all(&dir).map_err(|err| format!("创建会话文件目录失败: {err}"))?;
    Ok(dir)
}

pub fn session_files_dir_path(owner: &str, peer: &str) -> Result<PathBuf, String> {
    Ok(app_data_dir()?
        .join("files")
        .join(sanitize_component(owner))
        .join(sanitize_component(peer)))
}

pub fn incoming_file_path(owner: &str, peer: &str, file_name: &str) -> Result<PathBuf, String> {
    Ok(session_files_dir(owner, peer)?.join(safe_filename(file_name)))
}

pub fn delete_session_files(owner: &str, peer: &str) -> Result<(), String> {
    let dir = session_files_dir_path(owner, peer)?;
    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|err| format!("删除会话文件失败: {err}"))?;
    }
    Ok(())
}

pub fn is_session_cache_file(owner: &str, peer: &str, path: &str) -> bool {
    let path = PathBuf::from(path.trim());
    if path.as_os_str().is_empty() {
        return false;
    }
    let Ok(root) = session_files_dir_path(owner, peer) else {
        return false;
    };
    let Ok(root) = root.canonicalize() else {
        return false;
    };
    let Ok(file) = path.canonicalize() else {
        return false;
    };
    file.starts_with(&root) && file != root && file.is_file()
}

pub fn delete_session_cache_file(owner: &str, peer: &str, path: &str) -> Result<(), String> {
    if !is_session_cache_file(owner, peer, path) {
        return Ok(());
    }
    let path = PathBuf::from(path.trim());
    if path.is_file() {
        fs::remove_file(&path).map_err(|err| format!("删除缓存文件失败: {err}"))?;
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileStat {
    pub path: String,
    pub name: String,
    pub size: u64,
}

#[tauri::command]
pub fn file_stat(path: String) -> Result<FileStat, String> {
    let path = PathBuf::from(path.trim());
    let meta = fs::metadata(&path).map_err(|err| format!("读取文件信息失败: {err}"))?;
    if !meta.is_file() {
        return Err("not-a-file".into());
    }
    let name = path
        .file_name()
        .and_then(|item| item.to_str())
        .unwrap_or("file")
        .to_string();
    Ok(FileStat {
        path: path.to_string_lossy().into_owned(),
        name,
        size: meta.len(),
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathInfo {
    pub path: String,
    pub name: String,
    pub kind: String,
    pub size: u64,
}

fn path_name(path: &Path) -> String {
    path.file_name()
        .and_then(|item| item.to_str())
        .unwrap_or("file")
        .to_string()
}

fn decode_drop_path(raw: &str) -> PathBuf {
    let trimmed = raw.trim();
    let as_path = if let Some(rest) = trimmed.strip_prefix("file://") {
        let path_part = if rest.starts_with('/') {
            rest
        } else if let Some(idx) = rest.find('/') {
            &rest[idx..]
        } else {
            rest
        };
        percent_decode(path_part)
    } else {
        trimmed.to_string()
    };
    PathBuf::from(as_path)
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = from_hex(bytes[i + 1]);
            let lo = from_hex(bytes[i + 2]);
            if let (Some(h), Some(l)) = (hi, lo) {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| input.to_string())
}

fn from_hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn inspect_one(raw: &str) -> PathInfo {
    let path = decode_drop_path(raw);
    let name = path_name(&path);
    match fs::metadata(&path) {
        Ok(meta) if meta.is_file() => PathInfo {
            path: path.to_string_lossy().into_owned(),
            name,
            kind: "file".into(),
            size: meta.len(),
        },
        Ok(meta) if meta.is_dir() => PathInfo {
            path: path.to_string_lossy().into_owned(),
            name,
            kind: "directory".into(),
            size: 0,
        },
        Ok(_) => PathInfo {
            path: path.to_string_lossy().into_owned(),
            name,
            kind: "other".into(),
            size: 0,
        },
        Err(_) => PathInfo {
            path: path.to_string_lossy().into_owned(),
            name,
            kind: "missing".into(),
            size: 0,
        },
    }
}

#[tauri::command]
pub fn inspect_paths(paths: Vec<String>) -> Vec<PathInfo> {
    paths
        .into_iter()
        .filter(|item| !item.trim().is_empty())
        .map(|item| inspect_one(&item))
        .collect()
}

#[tauri::command]
pub fn reveal_in_dir(path: String) -> Result<(), String> {
    let path = PathBuf::from(path.trim());
    let meta = fs::metadata(&path).map_err(|_| "file-missing".to_string())?;
    if !meta.is_file() {
        return Err("file-missing".into());
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("-R")
            .arg(&path)
            .spawn()
            .map_err(|err| format!("打开目录失败: {err}"))?;
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(format!("/select,{}", path.display()))
            .spawn()
            .map_err(|err| format!("打开目录失败: {err}"))?;
        return Ok(());
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let dir = path.parent().unwrap_or(path.as_path());
        std::process::Command::new("xdg-open")
            .arg(dir)
            .spawn()
            .map_err(|err| format!("打开目录失败: {err}"))?;
        Ok(())
    }
}
