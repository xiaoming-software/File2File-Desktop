use std::fs::OpenOptions;
use std::os::windows::io::AsRawHandle;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use tauri::WebviewWindow;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows::Win32::Foundation::{HANDLE, HWND};
use windows::Win32::Graphics::Dwm::{
    DwmExtendFrameIntoClientArea, DwmSetWindowAttribute, DWMWINDOWATTRIBUTE,
};
use windows::Win32::System::Console::{
    FreeConsole, GetConsoleWindow, SetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE,
    STD_OUTPUT_HANDLE,
};
use windows::Win32::UI::Controls::MARGINS;
use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE};

const DWMWA_BORDER_COLOR: DWMWINDOWATTRIBUTE = DWMWINDOWATTRIBUTE(34);
const DWMWA_CAPTION_COLOR: DWMWINDOWATTRIBUTE = DWMWINDOWATTRIBUTE(35);
const DWMWA_WINDOW_CORNER_PREFERENCE: DWMWINDOWATTRIBUTE = DWMWINDOWATTRIBUTE(33);
const DWMWA_COLOR_NONE: u32 = 0xFFFFFFFE;
const DWMWCP_DONOTROUND: u32 = 1;

pub struct ConsoleGuard {
    stop: Arc<AtomicBool>,
}

impl ConsoleGuard {
    pub fn start() -> Self {
        silence_stdio();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let _ = thread::Builder::new()
            .name("hide-console".into())
            .spawn(move || {
                while !flag.load(Ordering::Relaxed) {
                    hide_console_window();
                    thread::sleep(Duration::from_millis(16));
                }
                hide_console_window();
            });
        Self { stop }
    }
}

impl Drop for ConsoleGuard {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        hide_console_window();
    }
}

pub fn silence_stdio() {
    hide_console_window();
    if let Ok(nul) = OpenOptions::new().read(true).write(true).open("NUL") {
        let handle = HANDLE(nul.as_raw_handle() as isize);
        unsafe {
            let _ = SetStdHandle(STD_INPUT_HANDLE, handle);
            let _ = SetStdHandle(STD_OUTPUT_HANDLE, handle);
            let _ = SetStdHandle(STD_ERROR_HANDLE, handle);
        }
        std::mem::forget(nul);
    }
}

pub fn hide_console_window() {
    unsafe {
        let hwnd = GetConsoleWindow();
        if hwnd.0 != 0 {
            let _ = ShowWindow(hwnd, SW_HIDE);
        }
        let _ = FreeConsole();
    }
}

pub fn disable_default_context_menu(window: &WebviewWindow) {
    let _ = window.with_webview(|webview| unsafe {
        let Ok(core) = webview.controller().CoreWebView2() else {
            return;
        };
        let Ok(settings) = core.Settings() else {
            return;
        };
        let _ = settings.SetAreDefaultContextMenusEnabled(false);
    });
}

pub fn apply_transparent_chrome(window: &WebviewWindow) {
    let _ = window.set_shadow(false);
    let _ = window.set_background_color(Some(tauri::window::Color(0, 0, 0, 0)));
    let hwnd = match window.window_handle() {
        Ok(handle) => match handle.as_raw() {
            RawWindowHandle::Win32(win) => HWND(win.hwnd.get()),
            _ => return,
        },
        Err(_) => return,
    };
    let margins = MARGINS {
        cxLeftWidth: -1,
        cxRightWidth: -1,
        cyTopHeight: -1,
        cyBottomHeight: -1,
    };
    unsafe {
        let _ = DwmExtendFrameIntoClientArea(hwnd, &margins);
        let none = DWMWA_COLOR_NONE;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR,
            &none as *const u32 as *const _,
            4,
        );
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_CAPTION_COLOR,
            &none as *const u32 as *const _,
            4,
        );
        let pref = DWMWCP_DONOTROUND;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &pref as *const u32 as *const _,
            4,
        );
    }
}

pub fn show_mouse_cursor() {
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::ShowCursor;
        let mut visible = ShowCursor(true);
        let mut guard = 0;
        while visible < 0 && guard < 32 {
            visible = ShowCursor(true);
            guard += 1;
        }
        while visible > 0 && guard < 48 {
            visible = ShowCursor(false);
            guard += 1;
        }
    }
}
