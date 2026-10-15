//! Native clipboard helpers for in-app terminals (xterm).
//! Uses OS clipboard APIs so Windows WebView2 / Linux WebKit don't block copy-paste.

#[tauri::command]
pub fn clipboard_write_text(text: String) -> Result<(), String> {
    let mut cb = arboard::Clipboard::new().map_err(|err| format!("clipboard-open: {err}"))?;
    cb.set_text(text)
        .map_err(|err| format!("clipboard-write: {err}"))
}

#[tauri::command]
pub fn clipboard_read_text() -> Result<String, String> {
    let mut cb = arboard::Clipboard::new().map_err(|err| format!("clipboard-open: {err}"))?;
    cb.get_text()
        .map_err(|err| format!("clipboard-read: {err}"))
}
