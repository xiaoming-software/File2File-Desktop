use crate::storage;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::thread;
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

const API_BASE: &str = "https://api.webrpc.cn/webrpc";
const PORTAL_FILE: &str = "portal_accounts.json";
const QUICK_REGISTER_FILE: &str = "quick_register.json";
const QUICK_REGISTER_LIMIT: u32 = 2;
const REGISTER_TOTAL_TIMEOUT: Duration = Duration::from_secs(30);
const CONSOLE_WINDOW_LABEL: &str = "webrpc-console";
const CONSOLE_URL: &str = "https://www.webrpc.cn/controllor.html";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortalLink {
    pub device_token: String,
    pub email: String,
    pub portal_password: String,
    pub token_expire_ms: i64,
    pub auto_registered: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoRegisterResult {
    pub email: String,
    pub portal_password: String,
    pub device_token: String,
    pub device_password: String,
    pub expire_time_ms: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RegisterProgress {
    step: String,
    message: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct PortalFile {
    #[serde(default)]
    links: Vec<PortalLink>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct QuickRegisterState {
    #[serde(default)]
    count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickRegisterQuota {
    pub count: u32,
    pub limit: u32,
    pub remaining: u32,
}

fn portal_path() -> Result<PathBuf, String> {
    Ok(storage::app_data_dir()?.join(PORTAL_FILE))
}

fn load_portal_file() -> Result<PortalFile, String> {
    let path = portal_path()?;
    if !path.exists() {
        return Ok(PortalFile::default());
    }
    let raw = fs::read_to_string(&path).map_err(|err| format!("读取控制台账户失败: {err}"))?;
    if raw.trim().is_empty() {
        return Ok(PortalFile::default());
    }
    serde_json::from_str::<PortalFile>(&raw).map_err(|err| format!("解析控制台账户失败: {err}"))
}

fn save_portal_file(data: &PortalFile) -> Result<(), String> {
    let path = portal_path()?;
    let text =
        serde_json::to_string_pretty(data).map_err(|err| format!("序列化控制台账户失败: {err}"))?;
    fs::write(&path, text).map_err(|err| format!("写入控制台账户失败: {err}"))?;
    Ok(())
}

fn quick_register_path() -> Result<PathBuf, String> {
    Ok(storage::app_data_dir()?.join(QUICK_REGISTER_FILE))
}

fn load_quick_register_state() -> Result<QuickRegisterState, String> {
    let path = quick_register_path()?;
    if !path.exists() {
        return Ok(QuickRegisterState::default());
    }
    let raw = fs::read_to_string(&path).map_err(|err| format!("读取一键注册次数失败: {err}"))?;
    if raw.trim().is_empty() {
        return Ok(QuickRegisterState::default());
    }
    serde_json::from_str::<QuickRegisterState>(&raw)
        .map_err(|err| format!("解析一键注册次数失败: {err}"))
}

fn save_quick_register_state(data: &QuickRegisterState) -> Result<(), String> {
    let path = quick_register_path()?;
    let text =
        serde_json::to_string_pretty(data).map_err(|err| format!("序列化一键注册次数失败: {err}"))?;
    fs::write(&path, text).map_err(|err| format!("写入一键注册次数失败: {err}"))?;
    Ok(())
}

fn quick_register_quota() -> Result<QuickRegisterQuota, String> {
    let count = load_quick_register_state()?.count;
    let remaining = QUICK_REGISTER_LIMIT.saturating_sub(count);
    Ok(QuickRegisterQuota {
        count,
        limit: QUICK_REGISTER_LIMIT,
        remaining,
    })
}

fn bump_quick_register_count() -> Result<(), String> {
    let mut data = load_quick_register_state()?;
    data.count = data.count.saturating_add(1);
    save_quick_register_state(&data)
}

fn ensure_quick_register_allowed() -> Result<(), String> {
    let quota = quick_register_quota()?;
    if quota.remaining == 0 {
        return Err("register-limit-reached".into());
    }
    Ok(())
}

fn emit_progress(app: &AppHandle, step: &str, message: &str) {
    let _ = app.emit(
        "portal-register-progress",
        RegisterProgress {
            step: step.to_string(),
            message: message.to_string(),
        },
    );
}

fn random_chars(len: usize, alphabet: &[u8]) -> String {
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
        ^ ((std::process::id() as u64) << 16);
    let mut x = seed.wrapping_mul(0x9e3779b97f4a7c15);
    let mut out = String::with_capacity(len);
    for _ in 0..len {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        out.push(alphabet[(x as usize) % alphabet.len()] as char);
    }
    out
}

fn gen_email() -> String {
    format!(
        "f2f{}@auto.webrpc",
        random_chars(
            12,
            b"abcdefghijklmnopqrstuvwxyz0123456789",
        )
    )
}

fn gen_password() -> String {
    random_chars(
        16,
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789",
    )
}

fn api_error(json: &Value) -> String {
    json.get("errorMsg")
        .or_else(|| json.get("error_msg"))
        .and_then(|v| v.as_str())
        .unwrap_or("request-failed")
        .to_string()
}

fn post_json(body: &Value, session_token: Option<&str>) -> Result<Value, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(15))
        .build();

    let url = match body.get("__url") {
        Some(Value::String(url)) => url.clone(),
        _ => return Err("missing-url".into()),
    };
    let mut payload = body.clone();
    if let Some(map) = payload.as_object_mut() {
        map.remove("__url");
    }

    let mut req = agent
        .post(&url)
        .set("Content-Type", "application/json; charset=utf-8");
    if let Some(token) = session_token {
        req = req.set("token", token);
    }

    let resp = req
        .send_json(payload)
        .map_err(|err| format!("network-error: {err}"))?;
    let status = resp.status();
    let json: Value = resp
        .into_json()
        .map_err(|err| format!("parse-error: {err}"))?;
    if status >= 400 {
        return Err(api_error(&json));
    }
    if json.get("status").and_then(|v| v.as_i64()) != Some(200) {
        return Err(api_error(&json));
    }
    Ok(json)
}

fn portal_register(email: &str, password: &str) -> Result<(), String> {
    let body = serde_json::json!({
        "__url": format!("{API_BASE}/register"),
        "email": email,
        "password": password,
    });
    post_json(&body, None)?;
    Ok(())
}

fn portal_login(email: &str, password: &str) -> Result<String, String> {
    let body = serde_json::json!({
        "__url": format!("{API_BASE}/login"),
        "email": email,
        "password": password,
    });
    let json = post_json(&body, None)?;
    json.get("data")
        .and_then(|d| d.get("token"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "login-token-missing".into())
}

#[derive(Debug, Clone)]
struct DeviceTokenRow {
    token: String,
    password: String,
    expire_time_ms: i64,
}

fn fetch_device_tokens(session_token: &str) -> Result<Vec<DeviceTokenRow>, String> {
    let body = serde_json::json!({
        "__url": format!("{API_BASE}/myTokens"),
        "page": 1,
        "pageSize": 20,
        "keyword": "",
        "searchMode": "orderNo",
    });
    let json = post_json(&body, Some(session_token))?;
    let list = json
        .get("data")
        .and_then(|d| d.get("list"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut rows = Vec::new();
    for item in list {
        let token = item
            .get("token")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        let password = item
            .get("password")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        let expire_time_ms = item
            .get("expireTime")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        if !token.is_empty() && !password.is_empty() {
            rows.push(DeviceTokenRow {
                token,
                password,
                expire_time_ms,
            });
        }
    }
    Ok(rows)
}

fn wait_for_tokens(session_token: &str, deadline: Instant) -> Result<Vec<DeviceTokenRow>, String> {
    loop {
        let rows = fetch_device_tokens(session_token)?;
        if !rows.is_empty() {
            return Ok(rows);
        }
        if Instant::now() >= deadline {
            return Err("tokens-not-ready".into());
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        thread::sleep(remaining.min(Duration::from_secs(2)));
    }
}

fn upsert_portal_link(link: PortalLink) -> Result<(), String> {
    let mut data = load_portal_file()?;
    data.links
        .retain(|item| item.device_token != link.device_token);
    data.links.insert(0, link);
    save_portal_file(&data)
}

fn find_portal_link(device_token: &str) -> Result<Option<PortalLink>, String> {
    let token = device_token.trim();
    Ok(load_portal_file()?
        .links
        .into_iter()
        .find(|item| item.device_token == token))
}

fn js_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
}

fn console_fresh_url() -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    // Cache-bust HTML entry so Gatekeeper-like WebView caches cannot reuse a stale shell.
    format!("{CONSOLE_URL}?_f2f_cb={ts}")
}

fn console_init_script(session_token: &str, email: &str) -> String {
    format!(
        r#"(function() {{
  try {{
    localStorage.setItem("webrpc_token", {token});
    localStorage.setItem("webrpc_user_email", {email});
  }} catch (e) {{}}
  try {{
    if (navigator.serviceWorker && navigator.serviceWorker.getRegistrations) {{
      navigator.serviceWorker.getRegistrations().then(function (regs) {{
        return Promise.all(regs.map(function (reg) {{ return reg.unregister(); }}));
      }}).catch(function () {{}});
    }}
    if (window.caches && caches.keys) {{
      caches.keys().then(function (keys) {{
        return Promise.all(keys.map(function (key) {{ return caches.delete(key); }}));
      }}).catch(function () {{}});
    }}
  }} catch (e) {{}}
}})();"#,
        token = js_string(session_token),
        email = js_string(email),
    )
}

fn open_console_window(app: &AppHandle, session_token: &str, email: &str) -> Result<(), String> {
    // Always tear down the previous console window so the next load cannot reuse an in-memory
    // document / HTTP cache of an outdated webrpc.cn build.
    if let Some(win) = app.get_webview_window(CONSOLE_WINDOW_LABEL) {
        let _ = win.clear_all_browsing_data();
        let _ = win.destroy();
        thread::sleep(Duration::from_millis(120));
    }

    let fresh = console_fresh_url();
    let url = fresh
        .parse()
        .map_err(|_| "invalid-console-url".to_string())?;

    let mut builder = WebviewWindowBuilder::new(app, CONSOLE_WINDOW_LABEL, WebviewUrl::External(url))
        .title("webrpc Console")
        .inner_size(1180.0, 760.0)
        .min_inner_size(960.0, 640.0)
        .center()
        .initialization_script(&console_init_script(session_token, email));

    // WebView2 only: avoid reusing cached HTML/JS/CSS for the console origin.
    #[cfg(target_os = "windows")]
    {
        builder = builder.additional_browser_args("--disable-http-cache");
    }

    builder
        .build()
        .map_err(|err| format!("open-console-failed: {err}"))?;

    Ok(())
}

fn refresh_expire_for_device(device_token: &str) -> Result<i64, String> {
    let link = find_portal_link(device_token)?
        .filter(|item| item.auto_registered)
        .ok_or_else(|| "portal-link-not-found".to_string())?;

    let session = portal_login(&link.email, &link.portal_password)?;
    let rows = fetch_device_tokens(&session)?;
    let row = rows
        .iter()
        .find(|item| item.token == device_token)
        .ok_or_else(|| "device-token-not-found".to_string())?;

    let mut data = load_portal_file()?;
    if let Some(item) = data
        .links
        .iter_mut()
        .find(|item| item.device_token == device_token)
    {
        item.token_expire_ms = row.expire_time_ms;
    }
    save_portal_file(&data)?;
    Ok(row.expire_time_ms)
}

#[tauri::command]
pub fn portal_quick_register_quota() -> Result<QuickRegisterQuota, String> {
    quick_register_quota()
}

#[tauri::command]
pub fn portal_link_get(device_token: String) -> Result<Option<PortalLink>, String> {
    find_portal_link(device_token.trim())
}

#[tauri::command]
pub fn portal_link_save(link: PortalLink) -> Result<(), String> {
    upsert_portal_link(link)
}

#[tauri::command]
pub fn portal_refresh_expiry(device_token: String) -> Result<i64, String> {
    refresh_expire_for_device(device_token.trim())
}

#[tauri::command]
pub fn portal_open_console(app: AppHandle, device_token: String) -> Result<(), String> {
    let link = find_portal_link(device_token.trim())?
        .filter(|item| item.auto_registered)
        .ok_or_else(|| "portal-link-not-found".to_string())?;

    let session_token = portal_login(&link.email, &link.portal_password)?;
    open_console_window(&app, &session_token, &link.email)
}

#[tauri::command]
pub fn webrpc_auto_register(app: AppHandle) -> Result<AutoRegisterResult, String> {
    ensure_quick_register_allowed()?;
    let (tx, rx) = mpsc::channel();
    let app_worker = app.clone();
    thread::spawn(move || {
        let result = auto_register_inner(&app_worker);
        let _ = tx.send(result);
    });
    match rx.recv_timeout(REGISTER_TOTAL_TIMEOUT) {
        Ok(result) => {
            let result = result?;
            // Best-effort: registration already succeeded; don't fail the UI if count write fails.
            let _ = bump_quick_register_count();
            Ok(result)
        }
        Err(_) => Err("register-timeout".into()),
    }
}

fn auto_register_inner(app: &AppHandle) -> Result<AutoRegisterResult, String> {
    let deadline = Instant::now() + REGISTER_TOTAL_TIMEOUT;
    let email = gen_email();
    let portal_password = gen_password();

    emit_progress(app, "register", "creating-account");
    portal_register(&email, &portal_password)?;

    emit_progress(app, "login", "signing-in");
    let session_token = portal_login(&email, &portal_password)?;

    emit_progress(app, "tokens", "claiming-tokens");
    let rows = wait_for_tokens(&session_token, deadline)?;
    let first = rows
        .first()
        .ok_or_else(|| "tokens-not-ready".to_string())?;

    let result = AutoRegisterResult {
        email: email.clone(),
        portal_password: portal_password.clone(),
        device_token: first.token.clone(),
        device_password: first.password.clone(),
        expire_time_ms: first.expire_time_ms,
    };

    upsert_portal_link(PortalLink {
        device_token: result.device_token.clone(),
        email,
        portal_password,
        token_expire_ms: result.expire_time_ms,
        auto_registered: true,
    })?;

    emit_progress(app, "done", "complete");
    Ok(result)
}
