use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

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

pub fn app_data_subdir(name: &str) -> Result<PathBuf, String> {
    let dir = app_data_dir()?.join(name);
    fs::create_dir_all(&dir).map_err(|err| format!("创建 {APP_DATA_DIR_NAME}/{name} 失败: {err}"))?;
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

pub fn is_dragout_path(path: &Path) -> bool {
    let Ok(root) = app_data_dir() else {
        return false;
    };
    let root = root.join("dragout");
    let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let root = root.canonicalize().unwrap_or(root);
    path.starts_with(&root)
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

fn drive_path_key(file_path: &str) -> String {
    let cleaned = file_path.trim().replace('\\', "/");
    let key = if cleaned.is_empty() || cleaned == "/" {
        "root".to_string()
    } else {
        cleaned
    };
    sanitize_component(&key)
}

pub fn drive_temp_root() -> Result<PathBuf, String> {
    Ok(app_data_subdir("drives")?.join("tem"))
}

pub fn drive_temp_session_dir(session_id: u32) -> Result<PathBuf, String> {
    Ok(drive_temp_root()?.join(session_id.to_string()))
}

pub fn drive_temp_file(session_id: u32, file_path: &str, file_name: &str) -> Result<PathBuf, String> {
    let dir = drive_temp_session_dir(session_id)?.join(drive_path_key(file_path));
    fs::create_dir_all(&dir).map_err(|err| format!("创建网盘临时目录失败: {err}"))?;
    Ok(dir.join(safe_filename(file_name)))
}

pub fn drive_out_file(session_id: u32, file_path: &str, file_name: &str) -> Result<PathBuf, String> {
    let rel = {
        let dir = file_path.trim().replace('\\', "/");
        let name = safe_filename(file_name);
        if dir.is_empty() || dir == "/" {
            name
        } else {
            format!("{}/{}", dir.trim_start_matches('/'), name)
        }
    };
    let dest = drive_temp_session_dir(session_id)?.join("nas-out").join(rel);
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("创建上传目录失败: {err}"))?;
    }
    Ok(dest)
}

pub fn delete_drive_temp_file(session_id: u32, file_path: &str, file_name: &str) {
    if let Ok(path) = drive_temp_file(session_id, file_path, file_name) {
        let _ = fs::remove_file(path);
    }
}

pub fn delete_drive_temp_session(session_id: u32) {
    if let Ok(dir) = drive_temp_session_dir(session_id) {
        let _ = fs::remove_dir_all(dir);
    }
}

pub fn delete_all_drive_temp() {
    if let Ok(dir) = drive_temp_root() {
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::create_dir_all(&dir);
    }
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
    pub modified_ms: u64,
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
        modified_ms: system_time_ms(meta.modified().ok()),
    })
}

fn system_time_ms(time: Option<SystemTime>) -> u64 {
    time.and_then(|item| item.duration_since(UNIX_EPOCH).ok())
        .map(|item| item.as_millis() as u64)
        .unwrap_or(0)
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum OfficeKind {
    Word,
    Sheet,
    Slide,
    Pdf,
}

fn office_kind_of(path: &Path) -> OfficeKind {
    let ext = path
        .extension()
        .and_then(|item| item.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "xls" | "xlsx" | "xlsm" | "csv" | "ods" | "et" => OfficeKind::Sheet,
        "ppt" | "pptx" | "pptm" | "odp" | "dps" => OfficeKind::Slide,
        "pdf" => OfficeKind::Pdf,
        _ => OfficeKind::Word,
    }
}

#[tauri::command]
pub fn office_open_file(path: String) -> Result<(), String> {
    let path = PathBuf::from(path.trim());
    if !path.is_file() {
        return Err("file-missing".into());
    }
    let kind = office_kind_of(&path);
    match pick_office_app(kind) {
        Some(app) => spawn_office_app(&app, &path),
        None => Err("no-office-app".into()),
    }
}

#[tauri::command]
pub fn office_file_busy(path: String) -> Result<bool, String> {
    let path = PathBuf::from(path.trim());
    if !path.is_file() {
        return Err("file-missing".into());
    }
    match OpenOptions::new().read(true).write(true).open(&path) {
        Ok(_) => Ok(false),
        Err(_) => Ok(true),
    }
}

enum OfficeApp {
    #[cfg(target_os = "macos")]
    Mac(String),
    #[cfg(not(target_os = "macos"))]
    Exe(PathBuf),
}

fn pick_office_app(kind: OfficeKind) -> Option<OfficeApp> {
    #[cfg(target_os = "macos")]
    {
        return pick_macos_office(kind).map(OfficeApp::Mac);
    }
    #[cfg(target_os = "windows")]
    {
        return pick_windows_office(kind).map(OfficeApp::Exe);
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        return pick_linux_office(kind).map(OfficeApp::Exe);
    }
}

fn spawn_office_app(app: &OfficeApp, path: &Path) -> Result<(), String> {
    match app {
        #[cfg(target_os = "macos")]
        OfficeApp::Mac(name) => {
            Command::new("open")
                .arg("-a")
                .arg(name)
                .arg(path)
                .spawn()
                .map_err(|err| format!("打开文件失败: {err}"))?;
            Ok(())
        }
        #[cfg(not(target_os = "macos"))]
        OfficeApp::Exe(exe) => {
            Command::new(exe)
                .arg(path)
                .spawn()
                .map_err(|err| format!("打开文件失败: {err}"))?;
            Ok(())
        }
    }
}

#[cfg(target_os = "macos")]
fn macos_app_exists(name: &str) -> bool {
    let home = user_workspace_dir();
    [
        format!("/Applications/{name}.app"),
        format!("/System/Applications/{name}.app"),
        format!("{}/Applications/{name}.app", home.display()),
    ]
    .iter()
    .any(|item| Path::new(item).is_dir())
}

#[cfg(target_os = "macos")]
fn pick_macos_office(kind: OfficeKind) -> Option<String> {
    for name in ["wpsoffice", "WPS Office", "Kingsoft WPS Office"] {
        if macos_app_exists(name) {
            return Some(name.to_string());
        }
    }
    if kind == OfficeKind::Pdf {
        if macos_app_exists("Preview") {
            return Some("Preview".to_string());
        }
        return None;
    }
    let microsoft = match kind {
        OfficeKind::Word => "Microsoft Word",
        OfficeKind::Sheet => "Microsoft Excel",
        OfficeKind::Slide => "Microsoft PowerPoint",
        OfficeKind::Pdf => "Preview",
    };
    if macos_app_exists(microsoft) {
        return Some(microsoft.to_string());
    }
    if macos_app_exists("LibreOffice") {
        return Some("LibreOffice".to_string());
    }
    let apple = match kind {
        OfficeKind::Word => "Pages",
        OfficeKind::Sheet => "Numbers",
        OfficeKind::Slide => "Keynote",
        OfficeKind::Pdf => "Preview",
    };
    if macos_app_exists(apple) {
        return Some(apple.to_string());
    }
    None
}

#[cfg(target_os = "windows")]
fn pick_windows_office(kind: OfficeKind) -> Option<PathBuf> {
    let wps_name = match kind {
        OfficeKind::Word | OfficeKind::Pdf => "wps.exe",
        OfficeKind::Sheet => "et.exe",
        OfficeKind::Slide => "wpp.exe",
    };
    let mut roots = Vec::new();
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        roots.push(PathBuf::from(local).join("Kingsoft").join("WPS Office"));
    }
    if let Some(pf) = std::env::var_os("ProgramFiles") {
        roots.push(PathBuf::from(&pf).join("Kingsoft").join("WPS Office"));
        roots.push(PathBuf::from(&pf).join("Microsoft Office"));
        roots.push(PathBuf::from(&pf).join("LibreOffice"));
    }
    if let Some(pf86) = std::env::var_os("ProgramFiles(x86)") {
        roots.push(PathBuf::from(&pf86).join("Kingsoft").join("WPS Office"));
        roots.push(PathBuf::from(&pf86).join("Microsoft Office"));
        roots.push(PathBuf::from(&pf86).join("LibreOffice"));
    }
    for root in &roots {
        if let Some(found) = find_named_file(root, &[wps_name, "wps.exe"], 4) {
            return Some(found);
        }
    }
    if kind == OfficeKind::Pdf {
        if let Some(pf) = std::env::var_os("ProgramFiles") {
            roots.push(PathBuf::from(&pf).join("Adobe"));
            roots.push(PathBuf::from(&pf).join("Foxit Software"));
            roots.push(PathBuf::from(&pf).join("Microsoft").join("Edge"));
        }
        if let Some(pf86) = std::env::var_os("ProgramFiles(x86)") {
            roots.push(PathBuf::from(&pf86).join("Adobe"));
            roots.push(PathBuf::from(&pf86).join("Foxit Software"));
            roots.push(PathBuf::from(&pf86).join("Microsoft").join("Edge"));
        }
        for root in &roots {
            if let Some(found) = find_named_file(
                root,
                &[
                    "Acrobat.exe",
                    "Acrobat.exe",
                    "AcroRd32.exe",
                    "FoxitPDFReader.exe",
                    "FoxitReader.exe",
                    "msedge.exe",
                ],
                5,
            ) {
                return Some(found);
            }
        }
        return None;
    }
    let office = match kind {
        OfficeKind::Word => "WINWORD.EXE",
        OfficeKind::Sheet => "EXCEL.EXE",
        OfficeKind::Slide => "POWERPNT.EXE",
        OfficeKind::Pdf => "msedge.exe",
    };
    for root in &roots {
        if let Some(found) = find_named_file(root, &[office], 5) {
            return Some(found);
        }
    }
    for root in &roots {
        if let Some(found) = find_named_file(root, &["soffice.exe"], 4) {
            return Some(found);
        }
    }
    None
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn pick_linux_office(kind: OfficeKind) -> Option<PathBuf> {
    let preferred: &[&str] = match kind {
        OfficeKind::Word => &["wps", "libreoffice", "soffice", "onlyoffice-desktopeditors"],
        OfficeKind::Sheet => &["et", "wps", "libreoffice", "soffice", "onlyoffice-desktopeditors"],
        OfficeKind::Slide => &["wpp", "wps", "libreoffice", "soffice", "onlyoffice-desktopeditors"],
        OfficeKind::Pdf => &["wps", "evince", "okular", "atril", "xdg-open"],
    };
    for name in preferred {
        if let Some(path) = which_cmd(name) {
            return Some(path);
        }
    }
    None
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn which_cmd(name: &str) -> Option<PathBuf> {
    let out = Command::new("which").arg(name).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let line = String::from_utf8_lossy(&out.stdout);
    let trimmed = line.lines().next().unwrap_or("").trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(PathBuf::from(trimmed))
    }
}

#[cfg(target_os = "windows")]
fn find_named_file(root: &Path, names: &[&str], depth: u8) -> Option<PathBuf> {
    if depth == 0 || !root.is_dir() {
        return None;
    }
    let entries = fs::read_dir(root).ok()?;
    let mut dirs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if let Some(name) = path.file_name().and_then(|item| item.to_str()) {
                if names.iter().any(|item| item.eq_ignore_ascii_case(name)) {
                    return Some(path);
                }
            }
        } else if path.is_dir() {
            dirs.push(path);
        }
    }
    for dir in dirs {
        if let Some(found) = find_named_file(&dir, names, depth - 1) {
            return Some(found);
        }
    }
    None
}
