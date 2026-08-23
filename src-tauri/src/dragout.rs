use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime};

use serde::Serialize;
use tauri::WebviewWindow;

use crate::storage;

const DRAG_PREVIEW_PNG: &[u8] = include_bytes!("../icons/32x32.png");

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DragOutResult {
    pub outcome: String,
    pub dest_dir: String,
    pub in_window: bool,
    pub x: i32,
    pub y: i32,
}

impl DragOutResult {
    fn cancel() -> Self {
        Self {
            outcome: "cancel".into(),
            dest_dir: String::new(),
            in_window: false,
            x: 0,
            y: 0,
        }
    }
}

#[tauri::command]
pub fn nas_start_drag_out(
    window: WebviewWindow,
    file_name: String,
    is_dir: bool,
) -> Result<DragOutResult, String> {
    let name = storage::safe_filename(&file_name);
    if name.is_empty() || name == "." || name == ".." {
        return Err("file-name-empty".into());
    }
    if is_dir {
        return Ok(DragOutResult::cancel());
    }

    let dummy_dir = storage::app_data_subdir("dragout")?
        .join(format!("d-{}-{}", now_ms(), std::process::id()));
    fs::create_dir_all(&dummy_dir).map_err(|err| format!("创建拖出目录失败: {err}"))?;
    let dummy = dummy_dir.join(&name);
    fs::write(&dummy, []).map_err(|err| format!("创建拖出文件失败: {err}"))?;
    let dummy_mtime = fs::metadata(&dummy)
        .and_then(|meta| meta.modified())
        .unwrap_or_else(|_| SystemTime::now());

    let window_for_drag = window.clone();
    let dummy_for_drag = dummy.clone();
    let (tx, rx) = mpsc::channel();
    window
        .run_on_main_thread(move || {
            let result = run_native_drag(&window_for_drag, dummy_for_drag);
            let _ = tx.send(result);
        })
        .map_err(|err| format!("无法开始拖出: {err}"))?;
    let mut result = rx.recv().unwrap_or_else(|_| Ok(DragOutResult::cancel()))?;

    result.in_window = cursor_in_window(&window, result.x, result.y);
    if result.outcome == "dropped" && !result.in_window {
        thread::sleep(Duration::from_millis(220));
        if let Some(dir) = find_recent_copy(&name, &dummy, dummy_mtime) {
            result.dest_dir = dir;
        } else if let Some(dir) = folder_at_point(result.x, result.y) {
            result.dest_dir = dir;
        }
    }

    let _ = fs::remove_file(&dummy);
    let _ = fs::remove_dir(&dummy_dir);
    Ok(result)
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|item| item.as_millis())
        .unwrap_or(0)
}

fn run_native_drag(window: &WebviewWindow, dummy: PathBuf) -> Result<DragOutResult, String> {
    let item = drag::DragItem::Files(vec![dummy]);
    let image = drag::Image::Raw(DRAG_PREVIEW_PNG.to_vec());
    let slot = Arc::new(Mutex::new(DragOutResult::cancel()));
    let slot_cb = slot.clone();
    let on_drop = move |result: drag::DragResult, cursor: drag::CursorPosition| {
        if let Ok(mut g) = slot_cb.lock() {
            g.outcome = match result {
                drag::DragResult::Dropped => "dropped",
                _ => "cancel",
            }
            .into();
            g.x = cursor.x;
            g.y = cursor.y;
        }
    };

    #[cfg(target_os = "linux")]
    {
        let gtk_window = window
            .gtk_window()
            .map_err(|err| format!("无法获取窗口: {err}"))?;
        drag::start_drag(
            &gtk_window,
            item,
            image,
            on_drop,
            drag::Options::default(),
        )
        .map_err(|err| format!("开始拖出失败: {err}"))?;
    }

    #[cfg(not(target_os = "linux"))]
    {
        drag::start_drag(window, item, image, on_drop, drag::Options::default())
            .map_err(|err| format!("开始拖出失败: {err}"))?;
    }

    Ok(slot.lock().map(|g| g.clone()).unwrap_or_else(|_| DragOutResult::cancel()))
}

fn cursor_in_window(window: &WebviewWindow, x: i32, y: i32) -> bool {
    let Ok(pos) = window.outer_position() else {
        return false;
    };
    let Ok(size) = window.outer_size() else {
        return false;
    };
    let scale = window.scale_factor().unwrap_or(1.0);
    let physical = [
        (x, y),
        (
            (x as f64 * scale).round() as i32,
            (y as f64 * scale).round() as i32,
        ),
        (
            (x as f64 / scale).round() as i32,
            (y as f64 / scale).round() as i32,
        ),
    ];
    let left = pos.x;
    let top = pos.y;
    let right = left.saturating_add(size.width as i32);
    let bottom = top.saturating_add(size.height as i32);
    physical.iter().any(|(px, py)| {
        *px >= left && *py >= top && *px < right && *py < bottom
    })
}

fn find_recent_copy(name: &str, dummy: &Path, dummy_mtime: SystemTime) -> Option<String> {
    let dummy_canon = dummy.canonicalize().unwrap_or_else(|_| dummy.to_path_buf());
    let cutoff = SystemTime::now()
        .checked_sub(Duration::from_secs(20))
        .unwrap_or(dummy_mtime);
    let min_mtime = dummy_mtime
        .checked_sub(Duration::from_secs(2))
        .unwrap_or(dummy_mtime);
    let mut best: Option<(SystemTime, PathBuf)> = None;
    for root in search_roots() {
        walk_recent(
            &root,
            name,
            &dummy_canon,
            cutoff,
            min_mtime,
            0,
            4,
            &mut best,
        );
    }
    best.map(|(_, path)| {
        path.parent()
            .unwrap_or(&path)
            .to_string_lossy()
            .into_owned()
    })
}

fn search_roots() -> Vec<PathBuf> {
    let home = storage::user_workspace_dir();
    let mut roots = vec![
        home.join("Desktop"),
        home.join("desktop"),
        home.join("Downloads"),
        home.join("downloads"),
        home.join("Documents"),
        home.join("documents"),
        home.join("Pictures"),
        home.join("Videos"),
        home.join("Music"),
        home.clone(),
    ];
    if let Some(dir) = xdg_user_dir("DESKTOP") {
        roots.push(dir);
    }
    if let Some(dir) = xdg_user_dir("DOWNLOAD") {
        roots.push(dir);
    }
    if let Some(dir) = xdg_user_dir("DOCUMENTS") {
        roots.push(dir);
    }
    #[cfg(target_os = "macos")]
    {
        roots.push(PathBuf::from("/Volumes"));
    }
    #[cfg(target_os = "linux")]
    {
        roots.push(PathBuf::from("/media"));
        roots.push(PathBuf::from("/mnt"));
        roots.push(PathBuf::from("/run/media"));
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(profile) = std::env::var_os("USERPROFILE") {
            let profile = PathBuf::from(profile);
            roots.push(profile.join("OneDrive").join("Desktop"));
            roots.push(profile.join("OneDrive").join("Downloads"));
        }
        if let Some(pubdesk) = std::env::var_os("PUBLIC") {
            roots.push(PathBuf::from(pubdesk).join("Desktop"));
        }
    }
    roots
}

fn xdg_user_dir(kind: &str) -> Option<PathBuf> {
    let out = std::process::Command::new("xdg-user-dir")
        .arg(kind)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if path.is_empty() {
        None
    } else {
        Some(PathBuf::from(path))
    }
}

fn walk_recent(
    root: &Path,
    name: &str,
    dummy: &Path,
    cutoff: SystemTime,
    min_mtime: SystemTime,
    depth: u32,
    max_depth: u32,
    best: &mut Option<(SystemTime, PathBuf)>,
) {
    if depth > max_depth {
        return;
    }
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let file_name = entry.file_name();
        let name_str = file_name.to_string_lossy();
        if name_str.starts_with('.')
            || name_str == "Library"
            || name_str == "AppData"
            || name_str == "node_modules"
            || name_str == "file2file_data"
            || name_str.eq_ignore_ascii_case("windows")
        {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if meta.is_dir() {
            walk_recent(
                &path,
                name,
                dummy,
                cutoff,
                min_mtime,
                depth + 1,
                if depth == 0 && root == &storage::user_workspace_dir() {
                    1
                } else {
                    max_depth
                },
                best,
            );
            continue;
        }
        if !meta.is_file() || name_str != name || meta.len() > 8 {
            continue;
        }
        let Ok(canon) = path.canonicalize() else {
            continue;
        };
        if canon == dummy {
            continue;
        }
        let mtime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        if mtime < cutoff || mtime < min_mtime {
            continue;
        }
        if best.as_ref().map(|(t, _)| mtime > *t).unwrap_or(true) {
            *best = Some((mtime, path));
        }
    }
}

fn folder_at_point(x: i32, y: i32) -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        return folder_at_point_macos(x, y);
    }
    #[cfg(target_os = "windows")]
    {
        return folder_at_point_windows(x, y);
    }
    #[cfg(target_os = "linux")]
    {
        return folder_at_point_linux(x, y);
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = (x, y);
        None
    }
}

#[cfg(target_os = "macos")]
fn folder_at_point_macos(_x: i32, _y: i32) -> Option<String> {
    let out = std::process::Command::new("osascript")
        .args([
            "-e",
            "tell application \"Finder\" to get POSIX path of (insertion location as alias)",
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if path.is_empty() {
        None
    } else {
        Some(path)
    }
}

#[cfg(target_os = "linux")]
fn folder_at_point_linux(_x: i32, _y: i32) -> Option<String> {
    None
}

#[cfg(target_os = "windows")]
fn folder_at_point_windows(x: i32, y: i32) -> Option<String> {
    windows_folder_at_point(x, y)
}

#[cfg(target_os = "windows")]
fn windows_folder_at_point(x: i32, y: i32) -> Option<String> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::WindowFromPoint;

    unsafe {
        let hwnd = WindowFromPoint(POINT { x, y });
        if hwnd.0 == 0 {
            return None;
        }
        if window_class_is_desktop(hwnd) {
            return std::env::var_os("USERPROFILE").map(|item| {
                PathBuf::from(item)
                    .join("Desktop")
                    .to_string_lossy()
                    .into_owned()
            });
        }
        explorer_path_from_hwnd(hwnd)
    }
}

#[cfg(target_os = "windows")]
unsafe fn explorer_path_from_hwnd(
    hwnd: windows::Win32::Foundation::HWND,
) -> Option<String> {
    use windows::core::ComInterface;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::System::Variant::{VARIANT, VT_I4};
    use windows::Win32::UI::Shell::{IShellWindows, IWebBrowserApp, ShellWindows};

    let explorer = find_class_root(hwnd, &["CabinetWClass", "ExploreWClass"])?;
    let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    let windows: IShellWindows = CoCreateInstance(&ShellWindows, None, CLSCTX_ALL).ok()?;
    let count = windows.Count().ok()?;
    for i in 0..count {
        let mut index = VARIANT::default();
        {
            let inner = &mut *index.Anonymous.Anonymous;
            inner.vt = VT_I4;
            inner.Anonymous.lVal = i;
        }
        let item = windows.Item(index).ok()?;
        let browser: IWebBrowserApp = item.cast().ok()?;
        let browser_hwnd = HWND(browser.HWND().ok()?.0);
        if browser_hwnd == explorer || is_ancestor(hwnd, browser_hwnd) {
            let url = browser.LocationURL().ok()?.to_string();
            return file_url_to_path(&url);
        }
    }
    None
}

#[cfg(target_os = "windows")]
unsafe fn window_class_is_desktop(hwnd: windows::Win32::Foundation::HWND) -> bool {
    let mut cur = hwnd;
    for _ in 0..12 {
        if cur.0 == 0 {
            break;
        }
        let class = window_class(cur);
        if class == "Progman" || class == "WorkerW" {
            return true;
        }
        cur = windows::Win32::UI::WindowsAndMessaging::GetParent(cur);
    }
    false
}

#[cfg(target_os = "windows")]
unsafe fn find_class_root(
    hwnd: windows::Win32::Foundation::HWND,
    names: &[&str],
) -> Option<windows::Win32::Foundation::HWND> {
    let mut cur = hwnd;
    for _ in 0..16 {
        if cur.0 == 0 {
            break;
        }
        let class = window_class(cur);
        if names.iter().any(|name| *name == class) {
            return Some(cur);
        }
        cur = windows::Win32::UI::WindowsAndMessaging::GetParent(cur);
    }
    None
}

#[cfg(target_os = "windows")]
unsafe fn is_ancestor(
    child: windows::Win32::Foundation::HWND,
    ancestor: windows::Win32::Foundation::HWND,
) -> bool {
    let mut cur = child;
    for _ in 0..16 {
        if cur.0 == 0 {
            break;
        }
        if cur == ancestor {
            return true;
        }
        cur = windows::Win32::UI::WindowsAndMessaging::GetParent(cur);
    }
    false
}

#[cfg(target_os = "windows")]
unsafe fn window_class(hwnd: windows::Win32::Foundation::HWND) -> String {
    let mut buf = [0u16; 256];
    let n = windows::Win32::UI::WindowsAndMessaging::GetClassNameW(hwnd, &mut buf);
    if n <= 0 {
        String::new()
    } else {
        String::from_utf16_lossy(&buf[..n as usize])
    }
}

#[cfg(target_os = "windows")]
fn file_url_to_path(url: &str) -> Option<String> {
    let trimmed = url.trim();
    let rest = trimmed.strip_prefix("file:///")?;
    let decoded = rest.replace('/', "\\");
    let decoded = decoded.replace("%20", " ");
    if decoded.is_empty() {
        None
    } else {
        Some(decoded)
    }
}
