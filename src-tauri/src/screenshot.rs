use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[cfg(target_os = "macos")]
use std::process::Command;

use serde::Serialize;
use tauri::{
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, PhysicalPosition, PhysicalSize,
    State, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};

use crate::storage;

const OVERLAY_PREFIX: &str = "screenshot-";

#[derive(Default)]
pub struct ScreenshotState {
    overlays: Mutex<HashMap<String, OverlayMeta>>,
    restore_main: Mutex<bool>,
}

struct OverlayMeta {
    image_path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayInfo {
    pub image_path: String,
}

#[cfg(target_os = "macos")]
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
    fn CGRequestScreenCaptureAccess() -> bool;
}

fn request_permission_best_effort(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    {
        let (tx, rx) = std::sync::mpsc::channel();
        let scheduled = app.run_on_main_thread(move || {
            unsafe {
                if !CGPreflightScreenCaptureAccess() {
                    let _ = CGRequestScreenCaptureAccess();
                }
            }
            let _ = tx.send(());
        });
        if scheduled.is_ok() {
            let _ = rx.recv_timeout(Duration::from_secs(90));
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
    }
}

fn set_restore_main(app: &AppHandle, value: bool) {
    if let Some(state) = app.try_state::<ScreenshotState>() {
        if let Ok(mut flag) = state.restore_main.lock() {
            *flag = value;
        }
    }
}

fn restore_main_window(app: &AppHandle) {
    let restore = if let Some(state) = app.try_state::<ScreenshotState>() {
        state
            .restore_main
            .lock()
            .map(|mut flag| {
                let value = *flag;
                *flag = false;
                value
            })
            .unwrap_or(false)
    } else {
        false
    };
    if let Some(main) = app.get_webview_window("main") {
        if restore {
            let _ = main.show();
            let _ = main.unminimize();
        }
        let _ = main.set_focus();
    }
}

fn hide_main_window(app: &AppHandle) -> bool {
    let Some(main) = app.get_webview_window("main") else {
        return false;
    };
    if main.hide().is_err() {
        return false;
    }
    true
}

fn capture_dir() -> Result<PathBuf, String> {
    let dir = storage::app_data_dir()?.join("screenshots");
    fs::create_dir_all(&dir).map_err(|err| format!("创建截图目录失败: {err}"))?;
    Ok(dir)
}

fn close_overlays(app: &AppHandle) {
    let labels: Vec<String> = app
        .webview_windows()
        .into_keys()
        .filter(|label| label.starts_with(OVERLAY_PREFIX))
        .collect();
    for label in labels {
        if let Some(win) = app.get_webview_window(&label) {
            let _ = win.close();
        }
    }
    if let Some(state) = app.try_state::<ScreenshotState>() {
        if let Ok(mut map) = state.overlays.lock() {
            for meta in map.values() {
                let _ = fs::remove_file(&meta.image_path);
            }
            map.clear();
        }
    }
}

fn elevate_overlay(window: &WebviewWindow) {
    let _ = window.set_always_on_top(true);
    let _ = window.set_skip_taskbar(true);

    #[cfg(target_os = "macos")]
    {
        let _ = window.with_webview(|webview| unsafe {
            use objc2::msg_send;
            use objc2::runtime::AnyObject;

            let ns_window = webview.ns_window() as *mut AnyObject;
            if ns_window.is_null() {
                return;
            }
            // NSScreenSaverWindowLevel = 1000, covers dock and menu bar.
            let _: () = msg_send![ns_window, setLevel: 1000i64];
            // canJoinAllSpaces | stationary | ignoresCycle | fullScreenAuxiliary
            let behavior: u64 = (1 << 0) | (1 << 4) | (1 << 6) | (1 << 8);
            let _: () = msg_send![ns_window, setCollectionBehavior: behavior];
            let _: () = msg_send![ns_window, setHidesOnDeactivate: false];
            let _: () = msg_send![ns_window, setOpaque: true];
        });
    }
}

struct CapturedScreen {
    label: String,
    image_path: PathBuf,
    logical_x: f64,
    logical_y: f64,
    logical_w: f64,
    logical_h: f64,
    physical_x: i32,
    physical_y: i32,
    physical_w: u32,
    physical_h: u32,
}

fn capture_screens(window: &WebviewWindow) -> Result<Vec<CapturedScreen>, String> {
    let xcap_result = capture_with_xcap(window);
    if let Ok(shots) = &xcap_result {
        if !shots.is_empty() {
            return xcap_result;
        }
    }

    #[cfg(target_os = "macos")]
    {
        return match capture_with_screencapture(window) {
            Ok(shots) if !shots.is_empty() => Ok(shots),
            Ok(_) => Err(match xcap_result {
                Err(err) => err,
                Ok(_) => "截屏失败，没有捕获到屏幕内容。".into(),
            }),
            Err(fallback_err) => Err(match xcap_result {
                Err(err) => err,
                Ok(_) => fallback_err,
            }),
        };
    }

    #[cfg(not(target_os = "macos"))]
    {
        match xcap_result {
            Ok(shots) if !shots.is_empty() => Ok(shots),
            Ok(_) => Err("截屏失败，没有捕获到屏幕内容。".into()),
            Err(err) => Err(err),
        }
    }
}

fn capture_with_xcap(window: &WebviewWindow) -> Result<Vec<CapturedScreen>, String> {
    let tauri_monitors = window
        .available_monitors()
        .map_err(|err| format!("读取显示器失败: {err}"))?;
    if tauri_monitors.is_empty() {
        return Err("没有可用的显示器".into());
    }

    let xcap_monitors =
        xcap::Monitor::all().map_err(|err| format!("初始化截屏失败: {err}"))?;
    if xcap_monitors.is_empty() {
        return Err("没有可用的显示器".into());
    }

    let dir = capture_dir()?;
    let mut used = HashSet::new();
    let mut shots = Vec::new();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|item| item.as_millis())
        .unwrap_or(0);

    for (index, xmon) in xcap_monitors.iter().enumerate() {
        let image = xmon
            .capture_image()
            .map_err(|err| format!("截屏失败: {err}"))?;
        if image.width() == 0 || image.height() == 0 {
            return Err("截屏失败: 捕获结果为空".into());
        }

        let img_w = image.width();
        let img_h = image.height();
        let tmon = match_monitor(&tauri_monitors, &mut used, img_w, img_h, index);
        let label = format!("{OVERLAY_PREFIX}{index}");
        let image_path = dir.join(format!("capture-{stamp}-{index}.png"));
        image
            .save(&image_path)
            .map_err(|err| format!("保存截屏失败: {err}"))?;
        shots.push(shot_from_monitor(label, image_path, tmon));
    }

    Ok(shots)
}

#[cfg(target_os = "macos")]
fn capture_with_screencapture(window: &WebviewWindow) -> Result<Vec<CapturedScreen>, String> {
    let tauri_monitors = window
        .available_monitors()
        .map_err(|err| format!("读取显示器失败: {err}"))?;
    if tauri_monitors.is_empty() {
        return Err("没有可用的显示器".into());
    }

    let dir = capture_dir()?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|item| item.as_millis())
        .unwrap_or(0);
    let mut shots = Vec::new();
    let total = tauri_monitors.len();

    for (index, tmon) in tauri_monitors.iter().enumerate() {
        let label = format!("{OVERLAY_PREFIX}{index}");
        let image_path = dir.join(format!("capture-{stamp}-{index}.png"));
        capture_one_display(&image_path, index, total)?;
        shots.push(shot_from_monitor(label, image_path, tmon));
    }

    Ok(shots)
}

#[cfg(target_os = "macos")]
fn capture_one_display(path: &std::path::Path, index: usize, total: usize) -> Result<(), String> {
    let path_str = path.to_str().ok_or_else(|| "截图路径无效".to_string())?;
    let mut cmd = Command::new("screencapture");
    cmd.arg("-x");
    if total > 1 {
        cmd.arg("-D").arg((index + 1).to_string());
    }
    let status = cmd
        .arg(path_str)
        .status()
        .map_err(|err| format!("调用系统截图失败: {err}"))?;
    if !status.success() || !path.exists() {
        return Err(
            "截屏失败。请在「系统设置 → 隐私与安全性 → 屏幕录制」中允许 File2File，然后重新打开应用再试。"
                .into(),
        );
    }
    Ok(())
}

fn shot_from_monitor(label: String, image_path: PathBuf, tmon: &tauri::Monitor) -> CapturedScreen {
    let scale = tmon.scale_factor().max(0.1);
    let pos = tmon.position();
    let size = tmon.size();
    CapturedScreen {
        label,
        image_path,
        logical_x: pos.x as f64 / scale,
        logical_y: pos.y as f64 / scale,
        logical_w: size.width as f64 / scale,
        logical_h: size.height as f64 / scale,
        physical_x: pos.x,
        physical_y: pos.y,
        physical_w: size.width,
        physical_h: size.height,
    }
}

fn match_monitor<'a>(
    monitors: &'a [tauri::Monitor],
    used: &mut HashSet<usize>,
    img_w: u32,
    img_h: u32,
    fallback: usize,
) -> &'a tauri::Monitor {
    let size_match = monitors.iter().enumerate().find(|(i, mon)| {
        !used.contains(i) && mon.size().width == img_w && mon.size().height == img_h
    });
    if let Some((i, mon)) = size_match {
        used.insert(i);
        return mon;
    }
    let unused = monitors.iter().enumerate().find(|(i, _)| !used.contains(i));
    if let Some((i, mon)) = unused {
        used.insert(i);
        return mon;
    }
    let i = fallback.min(monitors.len().saturating_sub(1));
    used.insert(i);
    &monitors[i]
}

fn open_overlays(app: &AppHandle, shots: Vec<CapturedScreen>) -> Result<(), String> {
    {
        let state = app.state::<ScreenshotState>();
        let mut map = state
            .overlays
            .lock()
            .map_err(|_| "截图状态被占用".to_string())?;
        map.clear();
        for shot in &shots {
            map.insert(
                shot.label.clone(),
                OverlayMeta {
                    image_path: shot.image_path.clone(),
                },
            );
        }
    }

    for shot in shots {
        let win = WebviewWindowBuilder::new(
            app,
            &shot.label,
            WebviewUrl::App("screenshot.html".into()),
        )
        .title("截图")
        .decorations(false)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .closable(true)
        .transparent(false)
        .shadow(false)
        .always_on_top(true)
        .visible(true)
        .focused(true)
        .skip_taskbar(true)
        .accept_first_mouse(true)
        .inner_size(shot.logical_w.max(1.0), shot.logical_h.max(1.0))
        .position(shot.logical_x, shot.logical_y)
        .build()
        .map_err(|err| {
            close_overlays(app);
            format!("打开截图窗口失败: {err}")
        })?;

        let _ = win.set_size(LogicalSize::new(shot.logical_w.max(1.0), shot.logical_h.max(1.0)));
        let _ = win.set_position(LogicalPosition::new(shot.logical_x, shot.logical_y));
        let _ = win.set_size(PhysicalSize::new(shot.physical_w.max(1), shot.physical_h.max(1)));
        let _ = win.set_position(PhysicalPosition::new(shot.physical_x, shot.physical_y));
        elevate_overlay(&win);
        #[cfg(target_os = "windows")]
        crate::win::disable_default_context_menu(&win);
        pin_overlay_topmost(&win);
        let _ = win.show();
        let _ = win.set_focus();
        let _ = win.set_ignore_cursor_events(false);
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn pin_overlay_topmost(window: &WebviewWindow) {
    let Ok(hwnd) = window.hwnd() else {
        return;
    };
    extern "system" {
        fn SetWindowPos(
            hwnd: isize,
            insert_after: isize,
            x: i32,
            y: i32,
            cx: i32,
            cy: i32,
            flags: u32,
        ) -> i32;
    }
    const HWND_TOPMOST: isize = -1;
    const SWP_NOMOVE: u32 = 0x0002;
    const SWP_NOSIZE: u32 = 0x0001;
    const SWP_SHOWWINDOW: u32 = 0x0040;
    let raw = hwnd.0 as isize;
    unsafe {
        SetWindowPos(
            raw,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
        );
    }
}

#[cfg(not(target_os = "windows"))]
fn pin_overlay_topmost(_window: &WebviewWindow) {}

#[tauri::command]
pub async fn screenshot_start(
    app: AppHandle,
    window: WebviewWindow,
    hide_app: Option<bool>,
) -> Result<(), String> {
    request_permission_best_effort(&app);
    close_overlays(&app);

    let hide_app = hide_app.unwrap_or(false);
    set_restore_main(&app, false);
    if hide_app {
        if !hide_main_window(&app) {
            return Err("无法隐藏当前窗口".into());
        }
        set_restore_main(&app, true);
    }

    let capture_window = window.clone();
    let shots = tauri::async_runtime::spawn_blocking(move || {
        if hide_app {
            std::thread::sleep(Duration::from_millis(360));
        }
        capture_screens(&capture_window)
    })
    .await
    .map_err(|err| format!("截屏任务失败: {err}"))?;

    let shots = match shots {
        Ok(items) => items,
        Err(err) => {
            restore_main_window(&app);
            return Err(err);
        }
    };
    if shots.is_empty() {
        restore_main_window(&app);
        return Err("截屏失败，没有捕获到屏幕内容。".into());
    }
    if let Err(err) = open_overlays(&app, shots) {
        restore_main_window(&app);
        return Err(err);
    }
    Ok(())
}

#[tauri::command]
pub fn screenshot_overlay_info(
    window: WebviewWindow,
    state: State<ScreenshotState>,
) -> Result<OverlayInfo, String> {
    let image_path = {
        let map = state
            .overlays
            .lock()
            .map_err(|_| "截图状态被占用".to_string())?;
        let meta = map
            .get(window.label())
            .ok_or_else(|| "截图窗口已关闭".to_string())?;
        meta.image_path.clone()
    };
    Ok(OverlayInfo {
        image_path: image_path.to_string_lossy().into_owned(),
    })
}

#[tauri::command]
pub fn screenshot_overlay_png(
    window: WebviewWindow,
    state: State<ScreenshotState>,
) -> Result<Vec<u8>, String> {
    let image_path = {
        let map = state
            .overlays
            .lock()
            .map_err(|_| "截图状态被占用".to_string())?;
        let meta = map
            .get(window.label())
            .ok_or_else(|| "截图窗口已关闭".to_string())?;
        meta.image_path.clone()
    };
    fs::read(&image_path).map_err(|err| format!("读取截屏失败: {err}"))
}

#[tauri::command]
pub fn screenshot_finish(
    app: AppHandle,
    window: WebviewWindow,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    overlay_png: Option<Vec<u8>>,
) -> Result<(), String> {
    if width == 0 || height == 0 {
        return Err("请先框选要截取的区域".into());
    }
    let image_path = {
        let state = app.state::<ScreenshotState>();
        let map = state
            .overlays
            .lock()
            .map_err(|_| "截图状态被占用".to_string())?;
        let meta = map
            .get(window.label())
            .ok_or_else(|| "截图窗口已关闭".to_string())?;
        meta.image_path.clone()
    };

    let mut img = image::open(&image_path)
        .map_err(|err| format!("读取截屏失败: {err}"))?
        .to_rgba8();
    let img_w = img.width();
    let img_h = img.height();
    let x = x.min(img_w.saturating_sub(1));
    let y = y.min(img_h.saturating_sub(1));
    let width = width.min(img_w.saturating_sub(x));
    let height = height.min(img_h.saturating_sub(y));
    let mut cropped = image::imageops::crop(&mut img, x, y, width, height).to_image();
    if let Some(overlay) = overlay_png {
        if !overlay.is_empty() {
            let over = image::load_from_memory(&overlay)
                .map_err(|err| format!("读取标注失败: {err}"))?
                .to_rgba8();
            image::imageops::overlay(&mut cropped, &over, 0, 0);
        }
    }

    let dir = capture_dir()?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|item| item.as_secs())
        .unwrap_or(0);
    let path = dir.join(format!("截图_{stamp}.png"));
    cropped
        .save(&path)
        .map_err(|err| format!("保存截图失败: {err}"))?;
    let path_str = path.to_string_lossy().into_owned();
    close_overlays(&app);
    restore_main_window(&app);
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.emit("screenshot-done", path_str);
        let _ = main.set_focus();
    }
    Ok(())
}

#[tauri::command]
pub fn screenshot_cancel(app: AppHandle) -> Result<(), String> {
    close_overlays(&app);
    restore_main_window(&app);
    Ok(())
}
