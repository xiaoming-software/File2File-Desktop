//! Spawn system `mpv` for NAS media preview (HTTP URL or local path).
//! Falls back to the JS SendFile + HTML5 path when mpv is missing or exits early.

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

static MPV_CHILD: Mutex<Option<Child>> = Mutex::new(None);

fn which_mpv() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        for name in ["mpv.exe", "mpv"] {
            if let Ok(out) = Command::new("where").arg(name).output() {
                if out.status.success() {
                    let line = String::from_utf8_lossy(&out.stdout);
                    let trimmed = line.lines().next().unwrap_or("").trim();
                    if !trimmed.is_empty() {
                        return Some(PathBuf::from(trimmed));
                    }
                }
            }
        }
        // Common portable / scoop installs.
        if let Ok(home) = std::env::var("USERPROFILE") {
            let candidates = [
                format!(r"{home}\scoop\apps\mpv\current\mpv.exe"),
                format!(r"{home}\AppData\Local\mpv\mpv.exe"),
            ];
            for c in candidates {
                let p = PathBuf::from(&c);
                if p.is_file() {
                    return Some(p);
                }
            }
        }
        None
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(out) = Command::new("which").arg("mpv").output() {
            if out.status.success() {
                let line = String::from_utf8_lossy(&out.stdout);
                let trimmed = line.lines().next().unwrap_or("").trim();
                if !trimmed.is_empty() {
                    return Some(PathBuf::from(trimmed));
                }
            }
        }
        #[cfg(target_os = "macos")]
        {
            for c in [
                "/opt/homebrew/bin/mpv",
                "/usr/local/bin/mpv",
                "/Applications/mpv.app/Contents/MacOS/mpv",
            ] {
                let p = PathBuf::from(c);
                if p.is_file() {
                    return Some(p);
                }
            }
        }
        None
    }
}

fn stop_locked(guard: &mut Option<Child>) {
    if let Some(mut child) = guard.take() {
        let _ = child.kill();
        let _ = child.wait();
    }
}

#[tauri::command]
pub fn nas_mpv_available() -> bool {
    which_mpv().is_some()
}

#[tauri::command]
pub fn nas_mpv_stop() {
    if let Ok(mut guard) = MPV_CHILD.lock() {
        stop_locked(&mut guard);
    }
}

/// Play `source` (http(s) URL or absolute file path) in an external mpv window.
#[tauri::command]
pub fn nas_mpv_play(source: String) -> Result<(), String> {
    let src = source.trim();
    if src.is_empty() {
        return Err("empty source".into());
    }
    let mpv = which_mpv().ok_or_else(|| "mpv not found".to_string())?;

    let mut guard = MPV_CHILD
        .lock()
        .map_err(|_| "mpv lock poisoned".to_string())?;
    stop_locked(&mut guard);

    let mut child = Command::new(&mpv)
        .args([
            "--force-window=immediate",
            "--keep-open=yes",
            "--no-terminal",
            "--",
            src,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("spawn mpv: {e}"))?;

    // Detect immediate start failure (missing deps / bad URL rejected at open).
    std::thread::sleep(Duration::from_millis(250));
    match child.try_wait() {
        Ok(Some(status)) => {
            return Err(format!("mpv exited early: {status}"));
        }
        Ok(None) => {}
        Err(e) => {
            let _ = child.kill();
            return Err(format!("mpv wait: {e}"));
        }
    }

    *guard = Some(child);
    Ok(())
}
