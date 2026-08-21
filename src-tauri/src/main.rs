#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod accounts;
mod chats;
mod sessions;
mod screenshot;
mod storage;
mod voice;
mod webrpc;

use tauri::Manager;

const WINDOW_CORNER_RADIUS: f64 = 12.0;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(webrpc::WebrpcApp::new())
        .manage(screenshot::ScreenshotState::default())
        .invoke_handler(tauri::generate_handler![
            webrpc::webrpc_login,
            webrpc::webrpc_logout,
            webrpc::webrpc_open_session,
            webrpc::webrpc_send_data,
            webrpc::webrpc_send_file,
            webrpc::webrpc_close_session,
            accounts::saved_accounts_list,
            accounts::saved_accounts_upsert,
            accounts::saved_accounts_delete,
            sessions::saved_sessions_list,
            sessions::saved_sessions_create,
            sessions::saved_sessions_update,
            sessions::saved_sessions_delete,
            chats::saved_chats_load,
            chats::saved_chats_append,
            chats::saved_chats_update,
            chats::saved_chats_clear,
            chats::saved_chats_delete,
            chats::saved_chats_delete_message,
            storage::file_stat,
            storage::inspect_paths,
            storage::reveal_in_dir,
            screenshot::screenshot_start,
            screenshot::screenshot_overlay_info,
            screenshot::screenshot_overlay_png,
            screenshot::screenshot_finish,
            screenshot::screenshot_cancel,
            voice::voice_invite,
            voice::voice_accept,
            voice::voice_reject,
            voice::voice_hangup,
            voice::voice_set_mute,
            voice::voice_state
        ])
        .setup(|app| {
            webrpc::install_exit_hooks();
            webrpc::set_app_handle(app.handle().clone());
            let window = app.get_webview_window("main").expect("missing main window");
            let _ = window.set_shadow(true);
            let _ = window.set_background_color(Some(tauri::window::Color(0, 0, 0, 0)));

            #[cfg(target_os = "macos")]
            apply_macos_rounded_corners(&window, WINDOW_CORNER_RADIUS);
            #[cfg(target_os = "macos")]
            install_macos_context_menu_filter();

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while running File2File")
        .run(|app, event| {
            if matches!(
                event,
                tauri::RunEvent::Exit | tauri::RunEvent::ExitRequested { .. }
            ) {
                app.state::<webrpc::WebrpcApp>().free();
            }
        });
}

#[cfg(target_os = "macos")]
fn apply_macos_rounded_corners(window: &tauri::WebviewWindow, radius: f64) {
    let _ = window.with_webview(move |webview| unsafe {
        use objc2::msg_send;
        use objc2::runtime::{AnyClass, AnyObject};

        let ns_window = webview.ns_window() as *mut AnyObject;
        if ns_window.is_null() {
            return;
        }

        let _: () = msg_send![ns_window, setOpaque: false];
        let _: () = msg_send![ns_window, setHasShadow: true];
        if let Some(ns_color) = AnyClass::get(c"NSColor") {
            let clear: *mut AnyObject = msg_send![ns_color, clearColor];
            let _: () = msg_send![ns_window, setBackgroundColor: clear];
        }

        let content_view: *mut AnyObject = msg_send![ns_window, contentView];
        round_ns_view(content_view, radius);

        let wk_webview = webview.inner() as *mut AnyObject;
        round_ns_view(wk_webview, radius);

        if !content_view.is_null() {
            let subviews: *mut AnyObject = msg_send![content_view, subviews];
            if !subviews.is_null() {
                let count: usize = msg_send![subviews, count];
                for i in 0..count {
                    let view: *mut AnyObject = msg_send![subviews, objectAtIndex: i];
                    round_ns_view(view, radius);
                }
            }
        }
    });
}

#[cfg(target_os = "macos")]
unsafe fn round_ns_view(view: *mut objc2::runtime::AnyObject, radius: f64) {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;

    if view.is_null() {
        return;
    }
    let _: () = msg_send![view, setWantsLayer: true];
    let layer: *mut AnyObject = msg_send![view, layer];
    if layer.is_null() {
        return;
    }
    let _: () = msg_send![layer, setCornerRadius: radius];
    let _: () = msg_send![layer, setMasksToBounds: true];
}

#[cfg(target_os = "macos")]
fn install_macos_context_menu_filter() {
    use std::sync::Once;

    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| unsafe {
        use objc2::runtime::{AnyClass, AnyObject, ClassBuilder, Sel};
        use objc2::{msg_send, sel};

        unsafe extern "C" fn menu_did_open(_this: *mut AnyObject, _cmd: Sel, notif: *mut AnyObject) {
            if notif.is_null() {
                return;
            }
            let menu: *mut AnyObject = msg_send![notif, object];
            strip_reload_menu_items(menu);
        }

        let Some(ns_object) = AnyClass::get(c"NSObject") else {
            return;
        };
        let cls = if let Some(existing) = AnyClass::get(c"F2FContextMenuFilter") {
            existing
        } else {
            let Some(mut builder) = ClassBuilder::new(c"F2FContextMenuFilter", ns_object) else {
                return;
            };
            builder.add_method(
                sel!(menuDidOpen:),
                menu_did_open as unsafe extern "C" fn(*mut AnyObject, Sel, *mut AnyObject),
            );
            builder.register()
        };

        let observer: *mut AnyObject = msg_send![cls, new];
        if observer.is_null() {
            return;
        }

        let Some(center_cls) = AnyClass::get(c"NSNotificationCenter") else {
            return;
        };
        let center: *mut AnyObject = msg_send![center_cls, defaultCenter];
        let name = ns_string("NSMenuDidBeginTrackingNotification");
        if center.is_null() || name.is_null() {
            return;
        }
        let _: () = msg_send![
            center,
            addObserver: observer,
            selector: sel!(menuDidOpen:),
            name: name,
            object: std::ptr::null::<AnyObject>()
        ];
    });
}

#[cfg(target_os = "macos")]
unsafe fn ns_string(text: &str) -> *mut objc2::runtime::AnyObject {
    use objc2::msg_send;
    use objc2::runtime::{AnyClass, AnyObject};
    use std::ffi::CString;

    let Ok(c_text) = CString::new(text) else {
        return std::ptr::null_mut();
    };
    let Some(cls) = AnyClass::get(c"NSString") else {
        return std::ptr::null_mut();
    };
    let s: *mut AnyObject = msg_send![cls, stringWithUTF8String: c_text.as_ptr()];
    s
}

#[cfg(target_os = "macos")]
unsafe fn strip_reload_menu_items(menu: *mut objc2::runtime::AnyObject) {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;

    if menu.is_null() {
        return;
    }
    let count: isize = msg_send![menu, numberOfItems];
    if count <= 0 {
        return;
    }
    for i in (0..count).rev() {
        let item: *mut AnyObject = msg_send![menu, itemAtIndex: i];
        if item.is_null() {
            continue;
        }
        if menu_item_is_reload(item) {
            let _: () = msg_send![menu, removeItem: item];
        }
    }
}

#[cfg(target_os = "macos")]
unsafe fn menu_item_is_reload(item: *mut objc2::runtime::AnyObject) -> bool {
    use objc2::runtime::{AnyObject, Sel};
    use objc2::{msg_send, sel};
    use std::ffi::CStr;

    let action: Sel = msg_send![item, action];
    if action == sel!(reload:) || action == sel!(reloadFromOrigin:) {
        return true;
    }

    let title: *mut AnyObject = msg_send![item, title];
    if title.is_null() {
        return false;
    }
    let utf8: *const i8 = msg_send![title, UTF8String];
    if utf8.is_null() {
        return false;
    }
    let Ok(text) = CStr::from_ptr(utf8).to_str() else {
        return false;
    };
    let lower = text.to_ascii_lowercase();
    lower == "reload"
        || lower.contains("reload")
        || text.contains("重新加载")
        || text.contains("重新載入")
}
