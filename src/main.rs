#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use std::ffi::{CStr, CString, c_char};
use std::fs;
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use ab_glyph::{FontArc, PxScale};
use chrono::Local;
use eframe::egui;
use image::{Rgba, RgbaImage};
use image::ImageReader;
use imageproc::drawing::{
    draw_hollow_circle_mut, draw_hollow_rect_mut, draw_line_segment_mut, draw_text_mut,
};
use imageproc::rect::Rect;
#[cfg(any(target_os = "macos", target_os = "windows"))]
use global_hotkey::GlobalHotKeyManager;
#[cfg(any(target_os = "macos", target_os = "windows"))]
use global_hotkey::hotkey::{Code, HotKey, Modifiers};
#[cfg(any(target_os = "macos", target_os = "windows"))]
use screenshots::Screen;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScreenshotTool {
    Crop,
    Rect,
    Circle,
    Arrow,
    Text,
}

#[derive(Debug, Clone)]
enum ScreenshotAction {
    Rect { start: egui::Pos2, end: egui::Pos2 },
    Circle { start: egui::Pos2, end: egui::Pos2 },
    Arrow { start: egui::Pos2, end: egui::Pos2 },
    Text { pos: egui::Pos2, text: String },
}

struct ScreenshotEditorState {
    source_image: RgbaImage,
    texture: Option<egui::TextureHandle>,
    crop_rect: Option<(egui::Pos2, egui::Pos2)>,
    actions: Vec<ScreenshotAction>,
    tool: ScreenshotTool,
    pending_drag_start: Option<egui::Pos2>,
    pending_drag_now: Option<egui::Pos2>,
    text_input: String,
    selection_done: bool,
}

/// 登录页固定内容区宽度（像素）
const EMBEDDED_LOGO_BYTES: &[u8] = include_bytes!("../assets/file2file_logo.png");
const EMBEDDED_ICON_BYTES: &[u8] = include_bytes!("../assets/file2file_icon.ico");
const TOPBAR_LOGO_SIZE: [f32; 2] = [170.0, 30.0];
const APP_DATA_DIR_NAME: &str = "file2file_data";
/// 会话聊天记录目录名（按「本地 Token / 对端 Token」定向存储，A→B 与 B→A 互不覆盖）。
const CHAT_HISTORY_DIR: &str = "chat_history";
const LOGIN_ALPHA: u8 = 179;
const SESSION_ALPHA: u8 = 222;

fn user_workspace_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Some(path) = std::env::var_os("USERPROFILE") {
            return PathBuf::from(path);
        }
        if let Some(drive) = std::env::var_os("HOMEDRIVE")
            && let Some(home_path) = std::env::var_os("HOMEPATH")
        {
            return PathBuf::from(format!(
                "{}{}",
                drive.to_string_lossy(),
                home_path.to_string_lossy()
            ));
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

fn main() -> eframe::Result<()> {
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([480.0, 520.0])
        .with_min_inner_size([480.0, 520.0])
        .with_transparent(true)
        .with_resizable(false);
    #[cfg(target_os = "macos")]
    {
        viewport = viewport
            .with_fullsize_content_view(true)
            .with_titlebar_shown(false)
            .with_title_shown(false);
    }
    if let Some(icon_data) = load_app_icon_data() {
        viewport = viewport.with_icon(icon_data);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "File2File",
        options,
        Box::new(|cc| {
            install_cjk_fonts(&cc.egui_ctx);
            Ok(Box::new(File2FileApp::new()))
        }),
    )
}

fn load_app_icon_data() -> Option<egui::IconData> {
    let Ok(reader) =
        ImageReader::new(std::io::Cursor::new(EMBEDDED_ICON_BYTES)).with_guessed_format()
    else {
        return None;
    };
    let Ok(decoded) = reader.decode() else {
        return None;
    };
    let rgba = decoded.to_rgba8();
    Some(egui::IconData {
        rgba: rgba.into_raw(),
        width: decoded.width(),
        height: decoded.height(),
    })
}

/// egui 内置默认字体（Ubuntu-Light 等）不含中文，必须在字体族中追加含 CJK 的字库，否则界面中文会显示为方框/乱码。
fn install_cjk_fonts(ctx: &egui::Context) {
    let Some((bytes, face_index)) = try_load_cjk_font_bytes() else {
        eprintln!(
            "file2file: 未找到中文字体文件，中文可能显示异常。可在运行目录放置 fonts/cjk.ttf（或 .otf/.ttc）。"
        );
        return;
    };

    let mut fonts = egui::FontDefinitions::default();
    let mut font_data = egui::FontData::from_owned(bytes);
    font_data.index = face_index;

    fonts.font_data.insert(
        "file2file_cjk".to_owned(),
        Arc::new(font_data),
    );

    if let Some(list) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        list.push("file2file_cjk".to_owned());
    }
    if let Some(list) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
        list.push("file2file_cjk".to_owned());
    }

    ctx.set_fonts(fonts);
}

/// 优先读取运行目录下的 `fonts/cjk.ttf`（便于打包），否则按平台尝试常见系统字体路径。
fn try_load_cjk_font_bytes() -> Option<(Vec<u8>, u32)> {
    let cwd_candidates = [
        PathBuf::from("fonts/cjk.ttf"),
        PathBuf::from("fonts/cjk.otf"),
        PathBuf::from("fonts/cjk.ttc"),
    ];
    for p in cwd_candidates {
        if let Ok(bytes) = fs::read(&p) {
            return Some((bytes, 0));
        }
    }

    #[cfg(target_os = "macos")]
    {
        const MAC_PATHS: &[&str] = &[
            "/System/Library/Fonts/PingFang.ttc",
            "/System/Library/Fonts/Hiragino Sans GB.ttc",
            "/System/Library/Fonts/Hiragino Sans GB W3.ttc",
            "/System/Library/Fonts/STHeiti Medium.ttc",
            "/System/Library/Fonts/Supplemental/Songti.ttc",
            "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
            "/Library/Fonts/Arial Unicode.ttf",
        ];
        for path in MAC_PATHS {
            if let Ok(bytes) = fs::read(path) {
                return Some((bytes, 0));
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".to_string());
        let base = PathBuf::from(windir).join("Fonts");
        let win_paths = [
            base.join("msyh.ttc"),
            base.join("msyhbd.ttc"),
            base.join("simhei.ttf"),
            base.join("simsun.ttc"),
        ];
        for p in win_paths {
            if let Ok(bytes) = fs::read(&p) {
                return Some((bytes, 0));
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        const LINUX_PATHS: &[&str] = &[
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/opentype/noto/NotoSansSC-Regular.otf",
            "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
            "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
        ];
        for path in LINUX_PATHS {
            if let Ok(bytes) = fs::read(path) {
                return Some((bytes, 0));
            }
        }
    }

    None
}

#[derive(Debug, Clone)]
enum MessageKind {
    Text,
    File,
}

#[derive(Debug, Clone)]
enum OutboundState {
    Sending,
    Sent,
    Failed(String),
}

#[derive(Debug, Clone)]
struct ChatMessage {
    local_id: u64,
    is_me: bool,
    content: String,
    timestamp: String,
    kind: MessageKind,
    file_name: Option<String>,
    file_path: Option<String>,
    file_size_bytes: Option<u64>,
    transferred_bytes: Option<u64>,
    send_started_at: Option<Instant>,
    send_speed_bps: Option<f64>,
    recv_speed_bps: Option<f64>,
    outbound: Option<OutboundState>,
}

#[derive(Debug, Clone)]
enum FileTransferSignal {
    Start { name: String, size_bytes: u64 },
    Progress {
        name: String,
        size_bytes: u64,
        transferred_bytes: u64,
    },
    End { name: String, size_bytes: u64, ok: bool },
}

/// 与 webrpc `OpenSession` 对应的本地会话；`id == None` 表示仅本地历史、未连接 SDK。
#[derive(Debug, Clone)]
struct WebrpcChatSession {
    /// `WebrpcClient_OpenSession` 返回的会话 ID；未绑定 SDK 时为 `None`。
    id: Option<u32>,
    peer_token: String,
    permission: String,
    messages: Vec<ChatMessage>,
    /// UI 是否视为已连接（绿点、可发送）。对端先入站时仅绑定 SDK，需用户确认后才为 true。
    ui_connected: bool,
    /// 用户自定义备注（如对方姓名/公司），便于识别会话。
    remark: String,
}

#[derive(Debug)]
enum InboundUiEvent {
    PeerText { session_id: u32, text: String },
    PeerFile {
        session_id: u32,
        name: String,
        path: PathBuf,
        size_bytes: u64,
    },
    PeerFileProgress {
        session_id: u32,
        name: String,
        size_bytes: u64,
        received_bytes: u64,
    },
    SendResult {
        session_id: u32,
        local_id: u64,
        ok: bool,
        detail: String,
    },
    /// SendFile 阻塞期间对端 PROGRESS 往往进不来；由发送线程每秒推送保守估算，供气泡刷新。
    OutboundSendProgressTick {
        session_id: u32,
        local_id: u64,
        transferred_estimate: u64,
    },
}

/// 单个 Token 对应的本地登录缓存（按 token 区分，互不覆盖）。
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedLoginProfile {
    token: String,
    password: String,
    permission: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct PersistedData {
    #[serde(default)]
    login_profiles: Vec<CachedLoginProfile>,
    /// 最近一次成功登录的 token，用于登录页默认选中。
    #[serde(default)]
    last_login_token: Option<String>,
    /// 旧版仅保存 token；加载时迁移到 `login_profiles` 后清空。
    #[serde(default)]
    saved_token: Option<String>,
}

/// 磁盘上的单条聊天消息（不含运行时 `Instant` 等字段）。
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedChatMessage {
    local_id: u64,
    is_me: bool,
    content: String,
    timestamp: String,
    kind: PersistedMessageKind,
    file_name: Option<String>,
    file_path: Option<String>,
    file_size_bytes: Option<u64>,
    transferred_bytes: Option<u64>,
    send_speed_bps: Option<f64>,
    recv_speed_bps: Option<f64>,
    outbound: Option<PersistedOutboundState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum PersistedMessageKind {
    Text,
    File,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum PersistedOutboundState {
    Sending,
    Sent,
    Failed { detail: String },
}

/// 一对 Token（本地登录方 + 对端）对应的完整会话记录。
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedSessionHistory {
    local_token: String,
    peer_token: String,
    permission: String,
    #[serde(default)]
    remark: String,
    next_local_msg_id: u64,
    messages: Vec<PersistedChatMessage>,
}

struct File2FileApp {
    data: PersistedData,
    state_file: PathBuf,
    current_user: Option<String>,
    /// 当前选中的会话在 `chat_sessions` 中的下标
    selected_session: Option<usize>,
    /// webrpc 会话列表（登录成功后使用）
    chat_sessions: Vec<WebrpcChatSession>,
    login_token: String,
    login_password: String,
    login_permission: String,
    active_login_permission: String,
    show_login_password: bool,
    show_login_permission: bool,
    remember_token: bool,
    /// 登录页「已保存账号」下拉当前选中下标（`login_profiles`）。
    selected_cached_profile: Option<usize>,
    login_message: String,
    login_error: bool,
    style_initialized: bool,
    composer_input: String,
    pending_file_path: Option<String>,
    status: String,
    logo_texture: Option<egui::TextureHandle>,
    client_handle: Option<usize>,
    is_logging_in: bool,
    /// 异步登录：后台线程完成后通过 channel 通知 UI
    login_rx: Option<mpsc::Receiver<Result<(usize, i32), String>>>,
    /// 回调 TCP 收到的消息注入 UI
    inbound_rx: Option<mpsc::Receiver<InboundUiEvent>>,
    inbound_tx: Option<mpsc::Sender<InboundUiEvent>>,
    /// 新建会话弹窗
    show_new_session_modal: bool,
    modal_peer_token: String,
    modal_permission: String,
    modal_error: String,
    open_session_busy: bool,
    open_session_rx: Option<mpsc::Receiver<Result<(u32, String, String), String>>>,
    /// 异步 OpenSession 要升级/填入的 `chat_sessions` 下标（新建或重连）。
    open_session_target_index: Option<usize>,
    show_reconnect_confirm: bool,
    reconnect_confirm_index: Option<usize>,
    session_connect_error: Option<String>,
    show_session_remark_modal: bool,
    remark_edit_index: Option<usize>,
    remark_edit_draft: String,
    screenshot_rx: Option<mpsc::Receiver<Result<PathBuf, String>>>,
    screenshot_in_progress: bool,
    screenshot_editor: Option<ScreenshotEditorState>,
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    screenshot_hotkey_manager: Option<GlobalHotKeyManager>,
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    screenshot_hotkey_id: Option<u32>,
    inbound_file_start_marks: HashMap<String, Instant>,
    inbound_file_speed_cache: HashMap<String, f64>,
    /// 接收端「累计已接收字节」：按 `(session|name)` 记录实时累计值，避免被后续事件覆盖。
    inbound_received_bytes: HashMap<String, u64>,
    /// 接收端「一次文件传输」UI 路由键：`session_id|规范化文件名`（不用字节大小，避免与流长度不一致导致重复气泡）
    inbound_active_file_row: HashMap<String, u64>,
    outbound_file_msg_index: HashMap<String, u64>,
    next_local_msg_id: u64,
    ui_lang: UiLanguage,
    last_sdk_session_sync: Option<Instant>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UiLanguage {
    Zh,
    En,
}

impl File2FileApp {
    fn new() -> Self {
        let app_root = Self::ensure_app_root();
        let state_file = app_root.join("state.json");
        let data = Self::load_or_create_data(&state_file).unwrap_or_default();

        let mut app = Self {
            data,
            state_file,
            current_user: None,
            selected_session: None,
            chat_sessions: Vec::new(),
            login_token: String::new(),
            login_password: String::new(),
            login_permission: String::new(),
            active_login_permission: String::new(),
            show_login_password: false,
            show_login_permission: false,
            remember_token: true,
            selected_cached_profile: None,
            login_message: String::new(),
            login_error: false,
            style_initialized: false,
            composer_input: String::new(),
            pending_file_path: None,
            status: String::new(),
            logo_texture: None,
            client_handle: None,
            is_logging_in: false,
            login_rx: None,
            inbound_rx: None,
            inbound_tx: None,
            show_new_session_modal: false,
            modal_peer_token: String::new(),
            modal_permission: String::new(),
            modal_error: String::new(),
            open_session_busy: false,
            open_session_rx: None,
            open_session_target_index: None,
            show_reconnect_confirm: false,
            reconnect_confirm_index: None,
            session_connect_error: None,
            show_session_remark_modal: false,
            remark_edit_index: None,
            remark_edit_draft: String::new(),
            screenshot_rx: None,
            screenshot_in_progress: false,
            screenshot_editor: None,
            #[cfg(any(target_os = "macos", target_os = "windows"))]
            screenshot_hotkey_manager: None,
            #[cfg(any(target_os = "macos", target_os = "windows"))]
            screenshot_hotkey_id: None,
            inbound_file_start_marks: HashMap::new(),
            inbound_file_speed_cache: HashMap::new(),
            inbound_received_bytes: HashMap::new(),
            inbound_active_file_row: HashMap::new(),
            outbound_file_msg_index: HashMap::new(),
            next_local_msg_id: 1,
            ui_lang: UiLanguage::Zh,
            last_sdk_session_sync: None,
        };
        app.init_screenshot_hotkey();
        app
    }

    fn tr<'a>(&self, zh: &'a str, en: &'a str) -> &'a str {
        match self.ui_lang {
            UiLanguage::Zh => zh,
            UiLanguage::En => en,
        }
    }

    /// 侧栏/标题区会话连接状态：绿点=已连接，灰点=未连接。
    fn session_primary_label(remark: &str, peer_token: &str, fallback: &str) -> String {
        let remark = remark.trim();
        if !remark.is_empty() {
            return remark.to_string();
        }
        let peer = peer_token.trim();
        if !peer.is_empty() {
            return peer.to_string();
        }
        fallback.to_string()
    }

    fn session_subtitle_token(remark: &str, peer_token: &str) -> Option<String> {
        let remark = remark.trim();
        let peer = peer_token.trim();
        if !remark.is_empty() && !peer.is_empty() {
            Some(peer.to_string())
        } else {
            None
        }
    }

    fn open_session_remark_editor(&mut self, index: usize) {
        let draft = self
            .chat_sessions
            .get(index)
            .map(|s| s.remark.clone())
            .unwrap_or_default();
        self.remark_edit_index = Some(index);
        self.remark_edit_draft = draft;
        self.show_session_remark_modal = true;
    }

    fn save_session_remark(&mut self) {
        let Some(index) = self.remark_edit_index else {
            self.show_session_remark_modal = false;
            return;
        };
        if index < self.chat_sessions.len() {
            self.chat_sessions[index].remark = self.remark_edit_draft.trim().to_string();
            self.persist_session_at_index(index);
            self.status = self.tr("备注已保存", "Remark saved").to_string();
        }
        self.show_session_remark_modal = false;
        self.remark_edit_index = None;
    }

    fn paint_session_connection_dot(ui: &egui::Ui, center: egui::Pos2, connected: bool) {
        const RADIUS: f32 = 5.0;
        let painter = ui.painter();
        if connected {
            painter.circle_filled(
                center,
                RADIUS + 2.5,
                egui::Color32::from_rgba_unmultiplied(66, 210, 118, 50),
            );
            painter.circle_filled(center, RADIUS, egui::Color32::from_rgb(66, 210, 118));
            painter.circle_stroke(
                center,
                RADIUS,
                egui::Stroke::new(1.0, egui::Color32::from_rgb(150, 255, 185)),
            );
        } else {
            painter.circle_filled(center, RADIUS, egui::Color32::from_rgb(130, 138, 152));
            painter.circle_stroke(
                center,
                RADIUS,
                egui::Stroke::new(1.0, egui::Color32::from_rgb(88, 96, 110)),
            );
        }
    }

    fn alloc_local_msg_id(&mut self) -> u64 {
        let id = self.next_local_msg_id;
        self.next_local_msg_id = self.next_local_msg_id.saturating_add(1);
        id
    }

    fn file_timing_key(session_id: u32, file_name: &str, size_bytes: u64) -> String {
        format!("{session_id}|{file_name}|{size_bytes}")
    }

    /// 接收端「同一会话 + 同一文件名」唯一键（一次传输一条气泡；大小以事件为准更新到消息体）。
    fn inbound_row_key(session_id: u32, name: &str) -> String {
        format!("{session_id}|{}", normalize_transfer_file_name(name))
    }

    /// 仅按「会话 + 文件名」在聊天记录里找最近一条对端文件气泡（不比较 size，避免与流长度不一致）。
    fn find_inbound_file_message_local_id_by_name(
        &self,
        session_id: u32,
        name: &str,
    ) -> Option<u64> {
        let s = self
            .chat_sessions
            .iter()
            .find(|s| s.id == Some(session_id))?;
        for m in s.messages.iter().rev() {
            if m.is_me || !matches!(m.kind, MessageKind::File) {
                continue;
            }
            let name_ok = m
                .file_path
                .as_deref()
                .and_then(|p| Path::new(p).file_name())
                .and_then(|f| f.to_str())
                .map(|f| f == name)
                .unwrap_or(false)
                || m.content.contains(name);
            if name_ok {
                return Some(m.local_id);
            }
        }
        None
    }

    /// START 或首次进度/落盘：保证该 `(session, name)` 只有一条接收气泡并登记到 `inbound_active_file_row`。
    fn ensure_inbound_active_file_row(
        &mut self,
        session_id: u32,
        name: &str,
        size_bytes: u64,
    ) -> u64 {
        let row_key = Self::inbound_row_key(session_id, name);
        if let Some(&id) = self.inbound_active_file_row.get(&row_key) {
            return id;
        }
        let local_id = self.alloc_local_msg_id();
        let i = self.ensure_session_for_inbound(session_id);
        let content = self.format_received_file_content(name, size_bytes, None);
        self.chat_sessions[i].messages.push(ChatMessage {
            local_id,
            is_me: false,
            content,
            timestamp: now_str(),
            kind: MessageKind::File,
            file_name: Some(name.to_string()),
            file_path: None,
            file_size_bytes: Some(size_bytes),
            transferred_bytes: Some(0),
            send_started_at: None,
            send_speed_bps: None,
            recv_speed_bps: None,
            outbound: None,
        });
        self.inbound_active_file_row.insert(row_key, local_id);
        local_id
    }

    /// 当前传输对应的气泡 `local_id`：活跃表 → 已有消息 → 新建（仅最后一种会 push）。
    fn resolve_inbound_file_local_id(
        &mut self,
        session_id: u32,
        name: &str,
        size_bytes: u64,
    ) -> u64 {
        let row_key = Self::inbound_row_key(session_id, name);
        if let Some(&id) = self.inbound_active_file_row.get(&row_key) {
            return id;
        }
        if let Some(id) = self.find_inbound_file_message_local_id_by_name(session_id, name) {
            self.inbound_active_file_row.insert(row_key, id);
            return id;
        }
        self.ensure_inbound_active_file_row(session_id, name, size_bytes)
    }

    fn format_received_file_content(
        &self,
        name: &str,
        size_bytes: u64,
        _speed_bps: Option<f64>,
    ) -> String {
        let recv_file_text = self
            .tr("对端发来文件", "Received file from peer")
            .to_string();
        let size_text = format_file_size(size_bytes);
        format!("{recv_file_text}: {name} ({size_text})")
    }

    fn apply_inbound_file_end_signal(
        &mut self,
        session_id: u32,
        name: &str,
        size_bytes: u64,
        ok: bool,
    ) {
        let row_key = Self::inbound_row_key(session_id, name);
        if !ok {
            self.inbound_file_start_marks.remove(&row_key);
            self.inbound_received_bytes.remove(&row_key);
            self.inbound_active_file_row.remove(&row_key);
            return;
        }
        let Some(started_at) = self.inbound_file_start_marks.remove(&row_key) else {
            self.inbound_received_bytes.remove(&row_key);
            self.inbound_active_file_row.remove(&row_key);
            return;
        };
        // 结束态必须给出最终速度，避免文件已完成仍显示“计算中”。
        let elapsed_secs = started_at.elapsed().as_secs_f64().max(0.001);
        let final_received = self.inbound_received_bytes.get(&row_key).copied().unwrap_or(size_bytes);
        let speed_bps = Some(final_received as f64 / elapsed_secs);
        let content = self.format_received_file_content(name, size_bytes, speed_bps);
        let local_id_opt = self.inbound_active_file_row.remove(&row_key);
        let local_id = local_id_opt
            .or_else(|| self.find_inbound_file_message_local_id_by_name(session_id, name));
        if let Some(local_id) = local_id {
            if let Some(s) = self
                .chat_sessions
                .iter_mut()
                .find(|s| s.id == Some(session_id))
                && let Some(msg) = s.messages.iter_mut().find(|m| m.local_id == local_id)
            {
                msg.recv_speed_bps = speed_bps;
                msg.content = content;
                msg.file_size_bytes = Some(size_bytes);
                return;
            }
        }
        if let Some(spd) = speed_bps {
            self.inbound_file_speed_cache.insert(row_key, spd);
        }
    }

    fn apply_outbound_file_progress_signal(
        &mut self,
        session_id: u32,
        name: &str,
        size_bytes: u64,
        transferred_bytes: u64,
    ) {
        let key = Self::file_timing_key(session_id, name, size_bytes);
        let Some(local_id) = self.outbound_file_msg_index.get(&key).copied() else {
            return;
        };
        if let Some(s) = self
            .chat_sessions
            .iter_mut()
            .find(|s| s.id == Some(session_id))
            && let Some(msg) = s.messages.iter_mut().find(|m| m.local_id == local_id)
            && let Some(started_at) = msg.send_started_at
        {
            let cap = msg.file_size_bytes.unwrap_or(transferred_bytes);
            let prev = msg.transferred_bytes.unwrap_or(0);
            let merged = prev.max(transferred_bytes).min(cap);
            let elapsed_secs = started_at.elapsed().as_secs_f64().max(0.001);
            msg.send_speed_bps = Some(merged as f64 / elapsed_secs);
            msg.transferred_bytes = Some(merged);
        }
    }

    fn apply_outbound_send_progress_tick(
        &mut self,
        session_id: u32,
        local_id: u64,
        transferred_estimate: u64,
    ) {
        if let Some(s) = self
            .chat_sessions
            .iter_mut()
            .find(|s| s.id == Some(session_id))
            && let Some(msg) = s.messages.iter_mut().find(|m| m.local_id == local_id)
            && matches!(msg.outbound, Some(OutboundState::Sending))
            && let Some(started_at) = msg.send_started_at
        {
            let cap = msg.file_size_bytes.unwrap_or(transferred_estimate);
            let prev = msg.transferred_bytes.unwrap_or(0);
            let merged = prev.max(transferred_estimate).min(cap);
            let elapsed_secs = started_at.elapsed().as_secs_f64().max(0.001);
            msg.transferred_bytes = Some(merged);
            msg.send_speed_bps = Some(merged as f64 / elapsed_secs);
        }
    }

    fn apply_inbound_file_progress(
        &mut self,
        session_id: u32,
        name: &str,
        size_bytes: u64,
        received_bytes: u64,
    ) {
        let row_key = Self::inbound_row_key(session_id, name);
        let started_at = self
            .inbound_file_start_marks
            .entry(row_key.clone())
            .or_insert_with(Instant::now);
        let elapsed_secs = started_at.elapsed().as_secs_f64();
        let speed_bps = if elapsed_secs >= 1.0 {
            Some(received_bytes as f64 / elapsed_secs)
        } else {
            None
        };
        let content = self.format_received_file_content(name, size_bytes, speed_bps);
        let tracked_received = {
            let entry = self
                .inbound_received_bytes
                .entry(row_key.clone())
                .or_insert(received_bytes);
            *entry = (*entry).max(received_bytes);
            *entry
        };
        let local_id = self.resolve_inbound_file_local_id(session_id, name, size_bytes);
        let i = self.ensure_session_for_inbound(session_id);
        if let Some(msg) = self.chat_sessions[i]
            .messages
            .iter_mut()
            .find(|m| m.local_id == local_id)
        {
            msg.content = content;
            msg.timestamp = now_str();
            msg.file_name = Some(name.to_string());
            msg.recv_speed_bps = speed_bps.or(msg.recv_speed_bps);
            msg.file_size_bytes = Some(size_bytes);
            msg.transferred_bytes = Some(tracked_received);
        }
    }

    fn find_session_index_by_id(&self, sid: u32) -> Option<usize> {
        self.chat_sessions
            .iter()
            .position(|s| s.id == Some(sid))
    }

    fn find_session_index_by_peer(&self, peer_token: &str) -> Option<usize> {
        let peer = peer_token.trim();
        self.chat_sessions
            .iter()
            .position(|s| s.peer_token.trim() == peer)
    }

    fn peer_token_for_session_info(&self, sid: u32, fallback: &str) -> String {
        let Some(handle) = self.client_handle else {
            return fallback.to_string();
        };
        match webrpc_tar_token_by_session(handle, sid) {
            Ok(token) if !token.trim().is_empty() => token,
            _ => fallback.to_string(),
        }
    }

    /// 将 SDK 入站会话绑定到已有离线历史（按对端 Token），避免对端重连时重复新建会话。
    fn attach_inbound_sdk_session(&mut self, sid: u32) -> usize {
        if let Some(i) = self.find_session_index_by_id(sid) {
            return i;
        }
        let peer_token = self.peer_token_for_session_info(sid, "");
        let peer_trim = peer_token.trim().to_string();
        if !peer_trim.is_empty() {
            if let Some(i) = self.find_session_index_by_peer(&peer_trim) {
                let refill_history = self.chat_sessions[i].messages.is_empty()
                    || self.chat_sessions[i].permission.is_empty()
                    || self.chat_sessions[i].remark.is_empty();
                let history_bundle = if refill_history {
                    Some(self.load_session_history_bundle(&peer_trim))
                } else {
                    None
                };
                let session = &mut self.chat_sessions[i];
                if session.id.is_none() || session.id == Some(sid) {
                    session.id = Some(sid);
                    // 对端主动连入：复用历史会话，UI 保持未连接，待用户确认。
                    if session.peer_token.trim().is_empty() {
                        session.peer_token = peer_token.clone();
                    }
                    if let Some((messages, permission, remark)) = history_bundle {
                        if session.messages.is_empty() && !messages.is_empty() {
                            session.messages = messages;
                        }
                        if session.permission.is_empty() && !permission.is_empty() {
                            session.permission = permission;
                        }
                        if session.remark.is_empty() && !remark.is_empty() {
                            session.remark = remark;
                        }
                    }
                    self.persist_session_at_index(i);
                    return i;
                }
            }
        }
        let (messages, permission, remark) = if peer_trim.is_empty() {
            (Vec::new(), String::new(), String::new())
        } else {
            self.load_session_history_bundle(&peer_trim)
        };
        let new_index = self.chat_sessions.len();
        self.chat_sessions.push(WebrpcChatSession {
            id: Some(sid),
            peer_token: peer_token.clone(),
            permission,
            messages,
            ui_connected: false,
            remark,
        });
        if !peer_trim.is_empty() {
            self.persist_session_at_index(new_index);
        }
        self.dedupe_sessions_by_peer();
        new_index
    }

    fn ensure_session_for_inbound(&mut self, sid: u32) -> usize {
        self.attach_inbound_sdk_session(sid)
    }

    /// 合并同一对端 Token 的重复会话项（保留有 SDK id 或消息更多的条目）。
    fn dedupe_sessions_by_peer(&mut self) {
        let mut keep_index_by_peer: HashMap<String, usize> = HashMap::new();
        let mut i = 0usize;
        while i < self.chat_sessions.len() {
            let peer = self.chat_sessions[i].peer_token.trim().to_string();
            if peer.is_empty() {
                i += 1;
                continue;
            }
            if let Some(&keep_i) = keep_index_by_peer.get(&peer) {
                let dup = self.chat_sessions.remove(i);
                let keep = &mut self.chat_sessions[keep_i];
                if keep.id.is_none() {
                    keep.id = dup.id;
                }
                keep.ui_connected = keep.ui_connected || dup.ui_connected;
                if dup.messages.len() > keep.messages.len() {
                    keep.messages = dup.messages;
                }
                if keep.permission.is_empty() {
                    keep.permission = dup.permission;
                }
                if keep.remark.is_empty() {
                    keep.remark = dup.remark;
                }
                continue;
            }
            keep_index_by_peer.insert(peer, i);
            i += 1;
        }
    }

    /// 对端主动 OpenSession 时，周期性把 SDK 会话挂到本地历史会话上（无需等首条消息）。
    fn sync_passive_sdk_sessions(&mut self) {
        let Some(handle) = self.client_handle else {
            return;
        };
        if webrpc_session_size(handle) == 0 {
            return;
        }
        for sid in 1..=2048u32 {
            if self.find_session_index_by_id(sid).is_some() {
                continue;
            }
            let peer = match webrpc_tar_token_by_session(handle, sid) {
                Ok(token) if !token.trim().is_empty() => token,
                _ => continue,
            };
            if self.find_session_index_by_peer(&peer).is_some() {
                self.attach_inbound_sdk_session(sid);
            }
        }
        self.dedupe_sessions_by_peer();
    }

    fn maybe_sync_passive_sdk_sessions(&mut self) {
        let due = self
            .last_sdk_session_sync
            .map(|t| t.elapsed() >= Duration::from_millis(400))
            .unwrap_or(true);
        if !due {
            return;
        }
        self.last_sdk_session_sync = Some(Instant::now());
        self.sync_passive_sdk_sessions();
    }

    fn list_saved_sessions_for_local(local_token: &str) -> Vec<PersistedSessionHistory> {
        let local = local_token.trim();
        if local.is_empty() {
            return Vec::new();
        }
        let dir = Self::ensure_app_root()
            .join(CHAT_HISTORY_DIR)
            .join(Self::sanitize_token_for_path(local));
        let Ok(entries) = fs::read_dir(&dir) else {
            return Vec::new();
        };
        let mut histories = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let Ok(raw) = fs::read_to_string(&path) else {
                continue;
            };
            if let Ok(history) = serde_json::from_str::<PersistedSessionHistory>(&raw) {
                if !history.peer_token.trim().is_empty() {
                    histories.push(history);
                }
            }
        }
        histories.sort_by(|a, b| {
            let last_ts = |h: &PersistedSessionHistory| {
                h.messages
                    .last()
                    .map(|m| m.timestamp.clone())
                    .unwrap_or_default()
            };
            last_ts(b).cmp(&last_ts(a))
        });
        histories
    }

    fn restore_offline_sessions_after_login(&mut self) {
        let Some(local) = self.current_user.clone() else {
            return;
        };
        for history in Self::list_saved_sessions_for_local(&local) {
            let peer = history.peer_token.trim();
            if peer.is_empty() || self.find_session_index_by_peer(peer).is_some() {
                continue;
            }
            let messages: Vec<ChatMessage> = history
                .messages
                .into_iter()
                .map(Self::chat_message_from_persisted)
                .collect();
            self.bump_next_local_msg_id_from_messages(&messages);
            self.next_local_msg_id = self.next_local_msg_id.max(history.next_local_msg_id);
            self.chat_sessions.push(WebrpcChatSession {
                id: None,
                peer_token: history.peer_token,
                permission: history.permission,
                messages,
                ui_connected: false,
                remark: history.remark,
            });
        }
        self.dedupe_sessions_by_peer();
    }

    fn ensure_app_root() -> PathBuf {
        let app_root = user_workspace_dir().join(APP_DATA_DIR_NAME);
        let _ = fs::create_dir_all(&app_root);
        app_root
    }

    fn sanitize_token_for_path(token: &str) -> String {
        token
            .chars()
            .map(|c| match c {
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0' => '_',
                c if c.is_whitespace() => '_',
                c => c,
            })
            .collect()
    }

    /// `chat_history/{local_token}/{local}__{peer}.json`，以双方 Token 为唯一键（含方向）。
    fn session_history_path(local_token: &str, peer_token: &str) -> PathBuf {
        let local = local_token.trim();
        let peer = peer_token.trim();
        let dir = Self::ensure_app_root()
            .join(CHAT_HISTORY_DIR)
            .join(Self::sanitize_token_for_path(local));
        let _ = fs::create_dir_all(&dir);
        let file_name = format!(
            "{}__{}.json",
            Self::sanitize_token_for_path(local),
            Self::sanitize_token_for_path(peer)
        );
        dir.join(file_name)
    }

    fn chat_message_to_persisted(msg: &ChatMessage) -> PersistedChatMessage {
        let kind = match msg.kind {
            MessageKind::Text => PersistedMessageKind::Text,
            MessageKind::File => PersistedMessageKind::File,
        };
        let outbound = msg.outbound.as_ref().map(|o| match o {
            OutboundState::Sending => PersistedOutboundState::Sending,
            OutboundState::Sent => PersistedOutboundState::Sent,
            OutboundState::Failed(d) => PersistedOutboundState::Failed { detail: d.clone() },
        });
        PersistedChatMessage {
            local_id: msg.local_id,
            is_me: msg.is_me,
            content: msg.content.clone(),
            timestamp: msg.timestamp.clone(),
            kind,
            file_name: msg.file_name.clone(),
            file_path: msg.file_path.clone(),
            file_size_bytes: msg.file_size_bytes,
            transferred_bytes: msg.transferred_bytes,
            send_speed_bps: msg.send_speed_bps,
            recv_speed_bps: msg.recv_speed_bps,
            outbound,
        }
    }

    fn chat_message_from_persisted(msg: PersistedChatMessage) -> ChatMessage {
        let kind = match msg.kind {
            PersistedMessageKind::Text => MessageKind::Text,
            PersistedMessageKind::File => MessageKind::File,
        };
        let outbound = msg.outbound.map(|o| match o {
            PersistedOutboundState::Sending => OutboundState::Sent,
            PersistedOutboundState::Sent => OutboundState::Sent,
            PersistedOutboundState::Failed { detail } => OutboundState::Failed(detail),
        });
        ChatMessage {
            local_id: msg.local_id,
            is_me: msg.is_me,
            content: msg.content,
            timestamp: msg.timestamp,
            kind,
            file_name: msg.file_name,
            file_path: msg.file_path,
            file_size_bytes: msg.file_size_bytes,
            transferred_bytes: msg.transferred_bytes,
            send_started_at: None,
            send_speed_bps: msg.send_speed_bps,
            recv_speed_bps: msg.recv_speed_bps,
            outbound,
        }
    }

    fn bump_next_local_msg_id_from_messages(&mut self, messages: &[ChatMessage]) {
        if let Some(max_id) = messages.iter().map(|m| m.local_id).max() {
            self.next_local_msg_id = self.next_local_msg_id.max(max_id.saturating_add(1));
        }
    }

    fn load_session_history(
        local_token: &str,
        peer_token: &str,
    ) -> Option<PersistedSessionHistory> {
        let local = local_token.trim();
        let peer = peer_token.trim();
        if local.is_empty() || peer.is_empty() {
            return None;
        }
        let path = Self::session_history_path(local, peer);
        let raw = fs::read_to_string(&path).ok()?;
        serde_json::from_str(&raw).ok()
    }

    fn load_session_history_bundle(
        &mut self,
        peer_token: &str,
    ) -> (Vec<ChatMessage>, String, String) {
        let Some(local) = self.current_user.as_deref() else {
            return (Vec::new(), String::new(), String::new());
        };
        let Some(history) = Self::load_session_history(local, peer_token) else {
            return (Vec::new(), String::new(), String::new());
        };
        if history.local_token.trim() != local.trim() || history.peer_token.trim() != peer_token.trim()
        {
            return (Vec::new(), String::new(), String::new());
        }
        let messages: Vec<ChatMessage> = history
            .messages
            .into_iter()
            .map(Self::chat_message_from_persisted)
            .collect();
        self.bump_next_local_msg_id_from_messages(&messages);
        self.next_local_msg_id = self.next_local_msg_id.max(history.next_local_msg_id);
        (messages, history.permission, history.remark)
    }

    fn save_session_history(
        local_token: &str,
        peer_token: &str,
        permission: &str,
        remark: &str,
        messages: &[ChatMessage],
        next_local_msg_id: u64,
    ) -> Result<()> {
        let local = local_token.trim();
        let peer = peer_token.trim();
        if local.is_empty() || peer.is_empty() {
            return Ok(());
        }
        let payload = PersistedSessionHistory {
            local_token: local.to_string(),
            peer_token: peer.to_string(),
            permission: permission.to_string(),
            remark: remark.to_string(),
            next_local_msg_id,
            messages: messages.iter().map(Self::chat_message_to_persisted).collect(),
        };
        let path = Self::session_history_path(local, peer);
        let text = serde_json::to_string_pretty(&payload)?;
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, text)?;
        fs::rename(tmp, path)?;
        Ok(())
    }

    fn persist_session_at_index(&mut self, index: usize) {
        let Some(local) = self.current_user.as_deref().map(str::trim).filter(|s| !s.is_empty())
        else {
            return;
        };
        let Some(session) = self.chat_sessions.get(index) else {
            return;
        };
        let peer = session.peer_token.trim();
        if peer.is_empty() {
            return;
        };
        let permission = session.permission.clone();
        let remark = session.remark.clone();
        let messages = session.messages.clone();
        let next_id = self.next_local_msg_id;
        if let Err(err) = Self::save_session_history(
            local, peer, &permission, &remark, &messages, next_id,
        )
        {
            self.status = format!(
                "{}: {err}",
                self.tr("会话记录保存失败", "Failed to save session history")
            );
        }
    }

    fn persist_all_sessions(&mut self) {
        let count = self.chat_sessions.len();
        for i in 0..count {
            self.persist_session_at_index(i);
        }
    }

    fn load_or_create_data(path: &Path) -> Result<PersistedData> {
        if !path.exists() {
            let initial = PersistedData::default();
            let text = serde_json::to_string_pretty(&initial)?;
            fs::write(path, text)?;
            return Ok(initial);
        }

        let raw = fs::read_to_string(path).with_context(|| format!("读取状态文件失败: {path:?}"))?;
        let mut parsed = serde_json::from_str::<PersistedData>(&raw)
            .with_context(|| format!("解析状态文件失败: {path:?}"))?;
        Self::migrate_legacy_persisted(&mut parsed);
        Ok(parsed)
    }

    fn migrate_legacy_persisted(data: &mut PersistedData) {
        if let Some(token) = data.saved_token.take() {
            let token = token.trim().to_string();
            if !token.is_empty()
                && !data
                    .login_profiles
                    .iter()
                    .any(|p| p.token == token)
            {
                data.login_profiles.push(CachedLoginProfile {
                    token,
                    password: String::new(),
                    permission: String::new(),
                });
            }
        }
    }

    fn upsert_login_profile(&mut self, token: &str, password: &str, permission: &str) {
        let token = token.trim().to_string();
        if token.is_empty() {
            return;
        }
        if let Some(existing) = self
            .data
            .login_profiles
            .iter_mut()
            .find(|p| p.token == token)
        {
            existing.password = password.to_string();
            existing.permission = permission.to_string();
        } else {
            self.data.login_profiles.push(CachedLoginProfile {
                token: token.clone(),
                password: password.to_string(),
                permission: permission.to_string(),
            });
        }
        if let Some(pos) = self.data.login_profiles.iter().position(|p| p.token == token) {
            let profile = self.data.login_profiles.remove(pos);
            self.data.login_profiles.insert(0, profile);
            self.selected_cached_profile = Some(0);
        }
        self.data.last_login_token = Some(token);
    }

    fn apply_cached_profile(&mut self, index: usize) {
        let Some(profile) = self.data.login_profiles.get(index).cloned() else {
            return;
        };
        self.login_token = profile.token;
        self.login_password = profile.password;
        self.login_permission = profile.permission;
        self.selected_cached_profile = Some(index);
    }

    fn sync_selected_profile_by_token(&mut self) {
        let token = self.login_token.trim();
        if token.is_empty() {
            self.selected_cached_profile = None;
            return;
        }
        self.selected_cached_profile = self
            .data
            .login_profiles
            .iter()
            .position(|p| p.token == token);
    }

    fn refill_login_from_cache(&mut self) {
        Self::migrate_legacy_persisted(&mut self.data);
        if let Some(token) = self.data.last_login_token.clone() {
            if let Some(idx) = self.data.login_profiles.iter().position(|p| p.token == token) {
                self.apply_cached_profile(idx);
                return;
            }
        }
        if self.login_token.trim().is_empty() {
            if self.data.login_profiles.is_empty() {
                return;
            }
            self.apply_cached_profile(0);
            return;
        }
        self.sync_selected_profile_by_token();
        if let Some(idx) = self.selected_cached_profile {
            self.apply_cached_profile(idx);
        }
    }

    fn save_data(&mut self) {
        match serde_json::to_string_pretty(&self.data) {
            Ok(text) => match fs::write(&self.state_file, text) {
                Ok(()) => {}
                Err(err) => {
                    self.status = format!("保存失败: {err}");
                }
            },
            Err(err) => {
                self.status = format!("序列化失败: {err}");
            }
        }
    }

    fn init_login_defaults(&mut self) {
        Self::migrate_legacy_persisted(&mut self.data);
        if self.login_token.is_empty() && self.login_password.is_empty() {
            self.refill_login_from_cache();
        }
    }

    /// 与 Go 示例一致：后台线程里 `New` + 周期轮询 `LoginStatus` 直到非 0，再取回调端口。
    fn begin_login(&mut self) {
        if self.is_logging_in || self.login_rx.is_some() {
            return;
        }
        let token = self.login_token.trim().to_string();
        let password = self.login_password.trim().to_string();
        let permission = self.login_permission.trim().to_string();
        if token.is_empty() || password.is_empty() {
            self.login_message = self
                .tr("Token 和密码不能为空", "Token and password cannot be empty")
                .to_string();
            self.login_error = true;
            return;
        }

        self.is_logging_in = true;
        self.login_error = false;
        self.login_message = self
            .tr("正在登录，请稍候...", "Logging in, please wait...")
            .to_string();

        let (tx, rx) = mpsc::channel();
        self.login_rx = Some(rx);
        thread::spawn(move || {
            let res = login_worker_blocking(token, password, permission);
            let _ = tx.send(res);
        });
    }

    fn poll_login_worker(&mut self, ctx: &egui::Context) {
        let Some(rx) = self.login_rx.take() else {
            return;
        };
        match rx.try_recv() {
            Ok(Ok((handle, port))) => {
                self.is_logging_in = false;
                self.client_handle = Some(handle);
                let logged_token = self.login_token.trim().to_string();
                let logged_password = self.login_password.trim().to_string();
                let logged_permission = self.login_permission.trim().to_string();
                self.current_user = Some(logged_token.clone());
                self.active_login_permission = logged_permission.clone();
                if self.remember_token {
                    self.upsert_login_profile(
                        &logged_token,
                        &logged_password,
                        &logged_permission,
                    );
                    self.save_data();
                }
                self.login_password.clear();
                self.login_permission.clear();
                self.show_login_password = false;
                self.show_login_permission = false;
                self.login_message = format!(
                    "{}: {port}",
                    self.tr("登录成功。回调 TCP 端口", "Login successful. Callback TCP port")
                );
                self.login_error = false;
                self.status = format!(
                    "{} {port}",
                    self.tr("webrpc 已连接，回调端口", "webrpc connected, callback port")
                );
                self.chat_sessions.clear();
                self.selected_session = None;
                self.open_session_target_index = None;
                self.show_reconnect_confirm = false;
                self.reconnect_confirm_index = None;
                self.session_connect_error = None;
                self.restore_offline_sessions_after_login();
                if self.selected_session.is_none() && !self.chat_sessions.is_empty() {
                    self.selected_session = Some(0);
                }
                self.last_sdk_session_sync = None;
                self.sync_passive_sdk_sessions();
                self.composer_input.clear();
                self.pending_file_path = None;
                self.inbound_file_start_marks.clear();
                self.inbound_file_speed_cache.clear();
                self.inbound_active_file_row.clear();
                self.outbound_file_msg_index.clear();
                self.show_new_session_modal = false;
                let (tx_in, rx_in) = mpsc::channel();
                self.inbound_rx = Some(rx_in);
                self.inbound_tx = Some(tx_in.clone());
                spawn_webrpc_callback_thread(handle, port, tx_in);
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                    1024.0, 680.0,
                )));
                ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(egui::vec2(
                    800.0, 560.0,
                )));
                ctx.request_repaint();
            }
            Ok(Err(err)) => {
                self.is_logging_in = false;
                self.login_message = format!("{}: {err}", self.tr("登录失败", "Login failed"));
                self.login_error = true;
                ctx.request_repaint();
            }
            Err(mpsc::TryRecvError::Empty) => {
                self.login_rx = Some(rx);
                ctx.request_repaint_after(Duration::from_millis(300));
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.is_logging_in = false;
                self.login_message = self
                    .tr("登录线程异常退出", "Login worker exited unexpectedly")
                    .to_string();
                self.login_error = true;
                ctx.request_repaint();
            }
        }
    }

    fn ensure_style(&mut self, ctx: &egui::Context) {
        if self.style_initialized {
            return;
        }
        ctx.set_visuals(egui::Visuals::dark());
        let mut style = (*ctx.style()).clone();
        style.spacing.item_spacing = egui::vec2(10.0, 10.0);
        style.spacing.button_padding = egui::vec2(14.0, 10.0);
        style.visuals.widgets.inactive.corner_radius = 8.0.into();
        style.visuals.widgets.hovered.corner_radius = 8.0.into();
        style.visuals.widgets.active.corner_radius = 8.0.into();
        ctx.set_style(style);
        self.style_initialized = true;
    }

    fn apply_page_alpha_style(&self, ctx: &egui::Context, alpha: u8) {
        let mut style = (*ctx.style()).clone();
        style.visuals.extreme_bg_color = egui::Color32::from_rgba_unmultiplied(10, 34, 52, alpha);
        style.visuals.faint_bg_color = egui::Color32::from_rgba_unmultiplied(8, 26, 40, alpha);
        style.visuals.widgets.noninteractive.bg_fill =
            egui::Color32::from_rgba_unmultiplied(12, 30, 46, alpha);
        style.visuals.widgets.inactive.bg_fill =
            egui::Color32::from_rgba_unmultiplied(12, 46, 68, alpha);
        style.visuals.widgets.hovered.bg_fill =
            egui::Color32::from_rgba_unmultiplied(20, 66, 95, alpha);
        style.visuals.widgets.active.bg_fill =
            egui::Color32::from_rgba_unmultiplied(24, 82, 115, alpha);
        style.visuals.widgets.inactive.bg_stroke =
            egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(110, 208, 244, alpha));
        style.visuals.widgets.hovered.bg_stroke =
            egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(135, 226, 255, alpha));
        style.visuals.widgets.active.bg_stroke =
            egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(150, 236, 255, alpha));
        ctx.set_style(style);
    }

    fn ensure_logo_texture(&mut self, ctx: &egui::Context) {
        if self.logo_texture.is_some() {
            return;
        }
        let Ok(reader) =
            ImageReader::new(std::io::Cursor::new(EMBEDDED_LOGO_BYTES)).with_guessed_format()
        else {
            return;
        };
        let Ok(decoded) = reader.decode() else {
            return;
        };
        let rgba = decoded.to_rgba8();
        let size = [rgba.width() as usize, rgba.height() as usize];
        let image = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
        self.logo_texture =
            Some(ctx.load_texture("file2file_logo", image, egui::TextureOptions::LINEAR));
    }

    fn draw_login_page(&mut self, ctx: &egui::Context) {
        self.init_login_defaults();
        let enter_pressed = ctx.input(|i| i.key_pressed(egui::Key::Enter));
        egui::Area::new(egui::Id::new("login_lang_switch"))
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-12.0, 12.0))
            .show(ctx, |ui| {
                let lang_btn = match self.ui_lang {
                    UiLanguage::Zh => "EN",
                    UiLanguage::En => "中文",
                };
                if ui
                    .add_sized(
                        [64.0, 30.0],
                        egui::Button::new(lang_btn).wrap_mode(egui::TextWrapMode::Extend),
                    )
                    .clicked()
                {
                    self.ui_lang = match self.ui_lang {
                        UiLanguage::Zh => UiLanguage::En,
                        UiLanguage::En => UiLanguage::Zh,
                    };
                }
            });

        egui::CentralPanel::default()
            .frame(
                egui::Frame::default()
                    .fill(egui::Color32::from_rgba_unmultiplied(4, 8, 18, LOGIN_ALPHA))
                    .inner_margin(egui::Margin::same(0)),
            )
            .show(ctx, |ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), ui.available_height().max(240.0)),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        let card_width = ui.available_width().max(320.0);
                        ui.allocate_ui_with_layout(
                            egui::vec2(card_width, ui.available_height().max(240.0)),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            egui::Frame::default()
                                .fill(egui::Color32::from_rgba_unmultiplied(12, 42, 62, LOGIN_ALPHA))
                                .stroke(egui::Stroke::new(
                                    1.5,
                                    egui::Color32::from_rgba_unmultiplied(110, 220, 255, LOGIN_ALPHA),
                                ))
                                .inner_margin(egui::Margin::same(14))
                                .corner_radius(12.0)
                                .show(ui, |ui| {
                                    egui::ScrollArea::vertical()
                                        .auto_shrink([false; 2])
                                        .max_height((ui.available_height() - 4.0).max(220.0))
                                        .show(ui, |ui| {
                                            egui::Frame::default()
                                                .fill(egui::Color32::from_rgba_unmultiplied(
                                                    36, 32, 78, LOGIN_ALPHA,
                                                ))
                                                .stroke(egui::Stroke::new(
                                                    1.0,
                                                    egui::Color32::from_rgba_unmultiplied(
                                                        160, 140, 255, LOGIN_ALPHA,
                                                    ),
                                                ))
                                                .corner_radius(9.0)
                                                .inner_margin(egui::Margin::symmetric(10, 8))
                                                .show(ui, |ui| {
                                                    ui.vertical_centered(|ui| {
                                                        if let Some(logo) = self.logo_texture.as_ref() {
                                                            let tex_size = logo.size_vec2();
                                                            let max_w = 220.0_f32.min(ui.available_width());
                                                            let logo_h = (max_w * tex_size.y / tex_size.x)
                                                                .max(20.0)
                                                                .min(48.0);
                                                            ui.add(
                                                                egui::Image::new((logo.id(), tex_size))
                                                                    .fit_to_exact_size(egui::vec2(
                                                                        max_w, logo_h,
                                                                    )),
                                                            );
                                                        } else {
                                                            ui.label(
                                                                egui::RichText::new("File2File")
                                                                    .strong()
                                                                    .size(24.0),
                                                            );
                                                        }
                                                        ui.add_space(6.0);
                                                        ui.label(
                                                            egui::RichText::new(self.tr(
                                                                "一款基于P2P通信的文件传输软件",
                                                                "A file transfer app powered by P2P",
                                                            ))
                                                                .strong()
                                                                .color(egui::Color32::from_rgb(214, 244, 255)),
                                                        );
                                                        ui.label(
                                                            egui::RichText::new(self.tr(
                                                                "使用 webrpc Token 登录，进入会话与文件传输工作区。",
                                                                "Sign in with your webrpc Token to enter sessions and file transfer workspace.",
                                                            ))
                                                            .small()
                                                            .color(egui::Color32::from_rgb(163, 213, 234)),
                                                        );
                                                    });
                                                });

                                            ui.add_space(14.0);

                                            let label_width = 58.0;
                                            let action_width = 78.0;
                                            let action_gap = 8.0;
                                            let action_safe_padding = 14.0;
                                            let token_hint =
                                                self.tr("请输入 Token", "Please enter Token").to_string();
                                            let pwd_hint = self
                                                .tr("请输入密码", "Please enter password")
                                                .to_string();
                                            let perm_hint = self
                                                .tr("请输入口令（可留空）", "Please enter permission (optional)")
                                                .to_string();
                                            let remember_token_label = self
                                                .tr("保存登录信息", "Save login info")
                                                .to_string();

                                            if self.data.login_profiles.len() > 1 {
                                                let account_label = self
                                                    .tr("已保存账号", "Saved accounts")
                                                    .to_string();
                                                ui.horizontal(|ui| {
                                                    ui.add_sized(
                                                        [label_width, 36.0],
                                                        egui::Label::new(
                                                            egui::RichText::new(&account_label)
                                                                .strong()
                                                                .color(egui::Color32::from_rgb(
                                                                    194, 231, 247,
                                                                )),
                                                        ),
                                                    );
                                                    let combo_w = ui.available_width().max(80.0);
                                                    let mut selected = self
                                                        .selected_cached_profile
                                                        .unwrap_or(0)
                                                        .min(
                                                            self.data
                                                                .login_profiles
                                                                .len()
                                                                .saturating_sub(1),
                                                        );
                                                    let selected_text = self
                                                        .data
                                                        .login_profiles
                                                        .get(selected)
                                                        .map(|p| p.token.as_str())
                                                        .unwrap_or("");
                                                    egui::ComboBox::from_id_salt(
                                                        "cached_login_profile",
                                                    )
                                                    .width(combo_w)
                                                    .selected_text(selected_text)
                                                    .show_ui(ui, |ui| {
                                                        for (i, profile) in self
                                                            .data
                                                            .login_profiles
                                                            .iter()
                                                            .enumerate()
                                                        {
                                                            ui.selectable_value(
                                                                &mut selected,
                                                                i,
                                                                &profile.token,
                                                            );
                                                        }
                                                    });
                                                    if self.selected_cached_profile != Some(selected)
                                                    {
                                                        self.apply_cached_profile(selected);
                                                    }
                                                });
                                                ui.add_space(8.0);
                                            }

                                            ui.horizontal(|ui| {
                                                ui.add_sized(
                                                    [label_width, 36.0],
                                                    egui::Label::new(
                                                        egui::RichText::new("Token")
                                                            .strong()
                                                            .color(egui::Color32::from_rgb(194, 231, 247)),
                                                    ),
                                                );
                                                let token_w = ui.available_width().max(80.0);
                                                let token_response = ui.add_sized(
                                                    [token_w, 36.0],
                                                    egui::TextEdit::singleline(&mut self.login_token)
                                                        .hint_text(token_hint.clone()),
                                                );
                                                if token_response.changed() {
                                                    let token = self.login_token.trim().to_string();
                                                    if let Some(idx) = self
                                                        .data
                                                        .login_profiles
                                                        .iter()
                                                        .position(|p| p.token == token)
                                                    {
                                                        self.apply_cached_profile(idx);
                                                    } else {
                                                        self.selected_cached_profile = None;
                                                    }
                                                }
                                            });

                                            ui.add_space(8.0);

                                            ui.horizontal(|ui| {
                                                ui.add_sized(
                                                    [label_width, 36.0],
                                                    egui::Label::new(
                                                        egui::RichText::new(self.tr("密码", "Password"))
                                                            .strong()
                                                            .color(egui::Color32::from_rgb(194, 231, 247)),
                                                    ),
                                                );
                                                let pass_like_input_width = (ui.available_width()
                                                    - action_width
                                                    - action_gap
                                                    - action_safe_padding)
                                                    .max(56.0);
                                                ui.add_sized(
                                                    [pass_like_input_width, 36.0],
                                                    egui::TextEdit::singleline(&mut self.login_password)
                                                        .hint_text(pwd_hint.clone())
                                                        .password(!self.show_login_password),
                                                );
                                                ui.add_space(action_gap);
                                                if ui
                                                    .add_sized(
                                                        [action_width, 36.0],
                                                        egui::Button::new(
                                                            egui::RichText::new(if self.show_login_password {
                                                                self.tr("隐藏", "Hide")
                                                            } else {
                                                                self.tr("显示", "Show")
                                                            })
                                                            .color(egui::Color32::from_rgb(221, 243, 255)),
                                                        )
                                                        .fill(egui::Color32::from_rgba_unmultiplied(52, 59, 112, LOGIN_ALPHA))
                                                        .stroke(egui::Stroke::new(
                                                            1.0,
                                                            egui::Color32::from_rgba_unmultiplied(124, 141, 221, LOGIN_ALPHA),
                                                        )),
                                                    )
                                                    .clicked()
                                                {
                                                    self.show_login_password =
                                                        !self.show_login_password;
                                                }
                                            });

                                            ui.add_space(8.0);

                                            ui.horizontal(|ui| {
                                                ui.add_sized(
                                                    [label_width, 36.0],
                                                    egui::Label::new(
                                                        egui::RichText::new(self.tr("口令", "Permission"))
                                                            .strong()
                                                            .color(egui::Color32::from_rgb(194, 231, 247)),
                                                    ),
                                                );
                                                let pass_like_input_width = (ui.available_width()
                                                    - action_width
                                                    - action_gap
                                                    - action_safe_padding)
                                                    .max(56.0);
                                                ui.add_sized(
                                                    [pass_like_input_width, 36.0],
                                                    egui::TextEdit::singleline(
                                                        &mut self.login_permission,
                                                    )
                                                    .hint_text(perm_hint.clone())
                                                    .password(!self.show_login_permission),
                                                );
                                                ui.add_space(action_gap);
                                                if ui
                                                    .add_sized(
                                                        [action_width, 36.0],
                                                        egui::Button::new(
                                                            egui::RichText::new(if self.show_login_permission {
                                                                self.tr("隐藏", "Hide")
                                                            } else {
                                                                self.tr("显示", "Show")
                                                            })
                                                            .color(egui::Color32::from_rgb(221, 243, 255)),
                                                        )
                                                        .fill(egui::Color32::from_rgba_unmultiplied(52, 59, 112, LOGIN_ALPHA))
                                                        .stroke(egui::Stroke::new(
                                                            1.0,
                                                            egui::Color32::from_rgba_unmultiplied(124, 141, 221, LOGIN_ALPHA),
                                                        )),
                                                    )
                                                    .clicked()
                                                {
                                                    self.show_login_permission =
                                                        !self.show_login_permission;
                                                }
                                            });

                                            ui.add_space(10.0);
                                            ui.checkbox(
                                                &mut self.remember_token,
                                                remember_token_label,
                                            );

                                            ui.add_space(10.0);

                                            let form_ready = !self.login_token.trim().is_empty()
                                                && !self.login_password.trim().is_empty();
                                            let mut login_clicked = false;
                                            ui.horizontal(|ui| {
                                                let left =
                                                    ((ui.available_width() - 190.0) / 2.0).max(0.0);
                                                ui.add_space(left);
                                                login_clicked = ui
                                                    .add_enabled(
                                                        form_ready && !self.is_logging_in,
                                                        egui::Button::new(if self.is_logging_in {
                                                            self.tr("登录中...", "Logging in...")
                                                        } else {
                                                            self.tr("登录", "Login")
                                                        })
                                                        .fill(egui::Color32::from_rgba_unmultiplied(27, 126, 173, LOGIN_ALPHA))
                                                        .stroke(egui::Stroke::new(
                                                            1.0,
                                                            egui::Color32::from_rgba_unmultiplied(122, 216, 248, LOGIN_ALPHA),
                                                        ))
                                                        .wrap_mode(egui::TextWrapMode::Extend)
                                                        .min_size(egui::vec2(190.0, 42.0)),
                                                    )
                                                    .clicked();
                                            });

                                            if login_clicked
                                                || (enter_pressed && form_ready && !self.is_logging_in)
                                            {
                                                self.begin_login();
                                            }

                                            ui.add_space(6.0);
                                            ui.horizontal_wrapped(|ui| {
                                                ui.label(
                                                    egui::RichText::new(self.tr(
                                                        "还没有 Token？",
                                                        "Don't have a Token yet?",
                                                    ))
                                                        .small()
                                                        .color(egui::Color32::from_gray(185)),
                                                );
                                                ui.hyperlink_to(
                                                    self.tr("前往 webrpc 官网获取", "Get one from webrpc website"),
                                                    "https://webrpc.cn",
                                                );
                                            });

                                            if !self.login_message.is_empty() {
                                                ui.add_space(6.0);
                                                let color = if self.login_error {
                                                    egui::Color32::from_rgb(255, 107, 107)
                                                } else {
                                                    egui::Color32::from_rgb(105, 218, 135)
                                                };
                                                ui.label(
                                                    egui::RichText::new(&self.login_message)
                                                        .color(color),
                                                );
                                            }
                                        });
                                });
                        },
                    );
                    },
                );
            });
    }

    fn begin_open_session(&mut self) {
        let peer = self.modal_peer_token.trim().to_string();
        if peer.is_empty() {
            self.modal_error = self
                .tr("请输入对端 Token", "Please enter peer Token")
                .to_string();
            return;
        }
        let permission = self.modal_permission.trim().to_string();
        let target = self.find_session_index_by_peer(&peer);
        self.modal_error.clear();
        self.session_connect_error = None;
        self.begin_connect_peer(peer, permission, target);
    }

    fn begin_reconnect_session(&mut self, index: usize) {
        let (ui_connected, has_sdk, peer, permission) = {
            let Some(session) = self.chat_sessions.get(index) else {
                return;
            };
            (
                session.ui_connected,
                session.id.is_some(),
                session.peer_token.trim().to_string(),
                session.permission.clone(),
            )
        };
        if ui_connected {
            self.selected_session = Some(index);
            return;
        }
        if has_sdk {
            self.chat_sessions[index].ui_connected = true;
            self.selected_session = Some(index);
            self.session_connect_error = None;
            self.status = format!(
                "{} → {}",
                self.tr("已接受对端连接", "Accepted peer connection"),
                peer
            );
            self.persist_session_at_index(index);
            return;
        }
        if peer.is_empty() {
            return;
        }
        self.session_connect_error = None;
        self.begin_connect_peer(peer, permission, Some(index));
    }

    fn begin_connect_peer(
        &mut self,
        peer: String,
        permission: String,
        target_index: Option<usize>,
    ) {
        if self.open_session_busy || self.open_session_rx.is_some() {
            return;
        }
        let Some(handle) = self.client_handle else {
            let err = self
                .tr("未连接 webrpc", "Not connected to webrpc")
                .to_string();
            self.modal_error = err.clone();
            self.session_connect_error = Some(err);
            return;
        };
        if let Some(current) = self.current_user.as_ref()
            && peer.trim() == current.trim()
        {
            let err = self
                .tr(
                    "目标 Token 不能是当前登录 Token",
                    "Peer Token cannot be the current login Token",
                )
                .to_string();
            self.modal_error = err.clone();
            self.session_connect_error = Some(err);
            return;
        }
        self.open_session_target_index = target_index;
        self.open_session_busy = true;
        self.status = self
            .tr("正在连接会话…", "Connecting session...")
            .to_string();
        let (tx, rx) = mpsc::channel();
        self.open_session_rx = Some(rx);
        thread::spawn(move || {
            let r = open_session_worker_blocking(handle, peer, permission);
            let _ = tx.send(r);
        });
    }

    fn poll_open_session_worker(&mut self, ctx: &egui::Context) {
        let Some(rx) = self.open_session_rx.take() else {
            return;
        };
        match rx.try_recv() {
            Ok(Ok((sid, peer, perm))) => {
                self.open_session_busy = false;
                let (mut messages, cached_perm, cached_remark) =
                    self.load_session_history_bundle(&peer);
                let session_permission = if perm.is_empty() {
                    cached_perm
                } else {
                    perm
                };
                if messages.is_empty() {
                    messages.push(ChatMessage {
                        local_id: self.alloc_local_msg_id(),
                        is_me: false,
                        content: format!(
                            "{}: {sid}",
                            self.tr("会话已建立，会话 ID", "Session established, session ID")
                        ),
                        timestamp: now_str(),
                        kind: MessageKind::Text,
                        file_name: None,
                        file_path: None,
                        file_size_bytes: None,
                        transferred_bytes: None,
                        send_started_at: None,
                        send_speed_bps: None,
                        recv_speed_bps: None,
                        outbound: None,
                    });
                }
                let stored_perm = session_permission;
                let target = self.open_session_target_index.take();
                let index = if let Some(i) = target {
                    if i < self.chat_sessions.len() {
                        self.chat_sessions[i].id = Some(sid);
                        self.chat_sessions[i].ui_connected = true;
                        self.chat_sessions[i].peer_token = peer.clone();
                        self.chat_sessions[i].permission = stored_perm.clone();
                        if self.chat_sessions[i].remark.is_empty() && !cached_remark.is_empty() {
                            self.chat_sessions[i].remark = cached_remark.clone();
                        }
                        if self.chat_sessions[i].messages.is_empty() {
                            self.chat_sessions[i].messages = messages;
                        }
                        Some(i)
                    } else {
                        None
                    }
                } else if let Some(i) = self.find_session_index_by_peer(&peer) {
                    self.chat_sessions[i].id = Some(sid);
                    self.chat_sessions[i].ui_connected = true;
                    self.chat_sessions[i].peer_token = peer.clone();
                    self.chat_sessions[i].permission = stored_perm.clone();
                    if self.chat_sessions[i].remark.is_empty() && !cached_remark.is_empty() {
                        self.chat_sessions[i].remark = cached_remark.clone();
                    }
                    if self.chat_sessions[i].messages.is_empty() {
                        self.chat_sessions[i].messages = messages;
                    }
                    Some(i)
                } else if let Some(i) = self.find_session_index_by_id(sid) {
                    self.chat_sessions[i].ui_connected = true;
                    self.chat_sessions[i].peer_token = peer.clone();
                    self.chat_sessions[i].permission = stored_perm.clone();
                    if self.chat_sessions[i].remark.is_empty() && !cached_remark.is_empty() {
                        self.chat_sessions[i].remark = cached_remark.clone();
                    }
                    if self.chat_sessions[i].messages.is_empty() {
                        self.chat_sessions[i].messages = messages;
                    }
                    Some(i)
                } else {
                    let new_index = self.chat_sessions.len();
                    self.chat_sessions.push(WebrpcChatSession {
                        id: Some(sid),
                        peer_token: peer.clone(),
                        permission: stored_perm,
                        messages,
                        ui_connected: true,
                        remark: cached_remark,
                    });
                    Some(new_index)
                };
                if let Some(i) = index {
                    self.selected_session = Some(i);
                    self.persist_session_at_index(i);
                }
                self.dedupe_sessions_by_peer();
                self.show_new_session_modal = false;
                self.show_reconnect_confirm = false;
                self.reconnect_confirm_index = None;
                self.modal_peer_token.clear();
                self.modal_permission.clear();
                self.modal_error.clear();
                self.session_connect_error = None;
                self.status = format!(
                    "{} {} → {}",
                    self.tr("已连接会话", "Session connected"),
                    sid,
                    peer
                );
                ctx.request_repaint();
            }
            Ok(Err(e)) => {
                self.open_session_busy = false;
                self.open_session_target_index = None;
                self.modal_error = e.clone();
                self.session_connect_error = Some(e);
                self.status = format!(
                    "{}: {}",
                    self.tr("连接失败", "Connection failed"),
                    self.session_connect_error.as_deref().unwrap_or("")
                );
                ctx.request_repaint();
            }
            Err(mpsc::TryRecvError::Empty) => {
                self.open_session_rx = Some(rx);
                ctx.request_repaint_after(Duration::from_millis(200));
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.open_session_busy = false;
                self.modal_error = self
                    .tr("打开会话线程异常退出", "Open-session worker exited unexpectedly")
                    .to_string();
                ctx.request_repaint();
            }
        }
    }

    fn init_screenshot_hotkey(&mut self) {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            if let Ok(manager) = GlobalHotKeyManager::new() {
                let hotkey = HotKey::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyA);
                if manager.register(hotkey).is_ok() {
                    self.screenshot_hotkey_id = Some(hotkey.id());
                    self.screenshot_hotkey_manager = Some(manager);
                }
            }
        }
    }

    fn poll_screenshot_hotkey(&mut self, ctx: &egui::Context) {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            if self.screenshot_hotkey_id.is_none() || self.current_user.is_none() {
                return;
            }
            let hotkey_id = self.screenshot_hotkey_id;
            while let Ok(event) = global_hotkey::GlobalHotKeyEvent::receiver().try_recv() {
                if Some(event.id) == hotkey_id && !self.screenshot_in_progress {
                    self.begin_desktop_capture(ctx);
                }
            }
        }
    }

    fn begin_desktop_capture(&mut self, ctx: &egui::Context) {
        if self.screenshot_in_progress {
            return;
        }
        self.screenshot_in_progress = true;
        self.status = self.tr("正在截图…", "Capturing screenshot...").to_string();
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        let (tx, rx) = mpsc::channel();
        self.screenshot_rx = Some(rx);
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(250));
            let _ = tx.send(capture_desktop_screenshot_file());
        });
    }

    fn poll_screenshot_worker(&mut self, ctx: &egui::Context) {
        let Some(rx) = self.screenshot_rx.take() else {
            return;
        };
        match rx.try_recv() {
            Ok(Ok(path)) => {
                self.screenshot_in_progress = false;
                match load_screenshot_editor_state(&path) {
                    Ok(editor) => {
                        self.screenshot_editor = Some(editor);
                        self.status = self
                            .tr("截图完成，请编辑后确认", "Screenshot captured. Edit and confirm.")
                            .to_string();
                    }
                    Err(e) => {
                        self.status = format!("{}: {e}", self.tr("截图失败", "Screenshot failed"));
                    }
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                ctx.request_repaint();
            }
            Ok(Err(e)) => {
                self.screenshot_in_progress = false;
                self.status = format!("{}: {e}", self.tr("截图失败", "Screenshot failed"));
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                ctx.request_repaint();
            }
            Err(mpsc::TryRecvError::Empty) => {
                self.screenshot_rx = Some(rx);
                ctx.request_repaint_after(Duration::from_millis(120));
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.screenshot_in_progress = false;
                self.status = self
                    .tr("截图线程异常退出", "Screenshot worker exited unexpectedly")
                    .to_string();
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                ctx.request_repaint();
            }
        }
    }

    fn draw_screenshot_editor(&mut self, ctx: &egui::Context) {
        let title = self.tr("截图编辑", "Screenshot Editor").to_string();
        let crop_label = self.tr("裁剪", "Crop").to_string();
        let rect_label = self.tr("矩形", "Rect").to_string();
        let circle_label = self.tr("圆圈", "Circle").to_string();
        let arrow_label = self.tr("箭头", "Arrow").to_string();
        let text_label = self.tr("文字", "Text").to_string();
        let undo_label = self.tr("撤销", "Undo").to_string();
        let cancel_label = self.tr("取消", "Cancel").to_string();
        let confirm_label = self.tr("确认并附加", "Confirm Attach").to_string();
        let text_input_label = self.tr("文字:", "Text:").to_string();
        let select_hint = self
            .tr(
                "请在图片上按住鼠标左键拖拽选区，松开后进入标注",
                "Press and drag left mouse to select area, release to annotate",
            )
            .to_string();

        let Some(editor) = self.screenshot_editor.as_mut() else {
            return;
        };
        ensure_editor_texture(ctx, editor);
        let mut close_editor = false;
        let mut save_confirmed = false;
        egui::Window::new(title)
            .resizable(true)
            .default_size(egui::vec2(900.0, 700.0))
            .collapsible(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    tool_button(ui, editor, ScreenshotTool::Crop, &crop_label);
                    if editor.selection_done {
                        tool_button(ui, editor, ScreenshotTool::Rect, &rect_label);
                        tool_button(ui, editor, ScreenshotTool::Circle, &circle_label);
                        tool_button(ui, editor, ScreenshotTool::Arrow, &arrow_label);
                        tool_button(ui, editor, ScreenshotTool::Text, &text_label);
                    }
                    if ui.button(&undo_label).clicked() {
                        if editor.actions.pop().is_none() {
                            editor.crop_rect = None;
                            editor.selection_done = false;
                        }
                    }
                    if ui.button(&cancel_label).clicked() {
                        close_editor = true;
                    }
                    if ui
                        .add_enabled(editor.selection_done, egui::Button::new(&confirm_label))
                        .clicked()
                    {
                        save_confirmed = true;
                    }
                });
                if !editor.selection_done {
                    ui.label(egui::RichText::new(select_hint.clone()).small());
                }
                if editor.selection_done && editor.tool == ScreenshotTool::Text {
                    ui.horizontal(|ui| {
                        ui.label(&text_input_label);
                        ui.text_edit_singleline(&mut editor.text_input);
                    });
                }
                ui.separator();
                if let Some(tex) = &editor.texture {
                    let avail = ui.available_size();
                    let tex_size = tex.size_vec2();
                    let scale = (avail.x / tex_size.x).min(avail.y / tex_size.y).max(0.1);
                    let draw_size = tex_size * scale;
                    let (rect, response) =
                        ui.allocate_exact_size(draw_size, egui::Sense::click_and_drag());
                    ui.painter().image(
                        tex.id(),
                        rect,
                        egui::Rect::from_min_size(egui::Pos2::ZERO, tex_size),
                        egui::Color32::WHITE,
                    );
                    handle_editor_input(editor, &response, rect, tex_size, scale);
                    paint_editor_overlays(ui.painter(), editor, rect, scale);
                }
            });
        if close_editor {
            self.screenshot_editor = None;
            self.status = self.tr("已取消截图编辑", "Screenshot editing cancelled").to_string();
        } else if save_confirmed {
            match render_editor_to_file(editor) {
                Ok(path) => {
                    self.pending_file_path = Some(path.display().to_string());
                    self.status = format!(
                        "{}: {}",
                        self.tr("已添加截图", "Screenshot attached"),
                        path.display()
                    );
                    self.screenshot_editor = None;
                }
                Err(e) => {
                    self.status = format!("{}: {e}", self.tr("截图保存失败", "Save failed"));
                }
            }
        }
    }

    fn poll_inbound_events(&mut self, ctx: &egui::Context) {
        let Some(rx) = self.inbound_rx.take() else {
            return;
        };
        let mut got_event = false;
        let mut persist_sessions = false;
        while let Ok(ev) = rx.try_recv() {
            got_event = true;
            match ev {
                InboundUiEvent::PeerText { session_id, text } => {
                    if let Some(signal) = parse_file_transfer_signal(&text) {
                        match signal {
                            FileTransferSignal::Start { name, size_bytes } => {
                                let row_key = Self::inbound_row_key(session_id, &name);
                                // 每次 START 都视为一次全新传输：重置计时/累计并清理磁盘缓存文件。
                                self.inbound_file_start_marks
                                    .insert(row_key.clone(), Instant::now());
                                self.inbound_received_bytes.remove(&row_key);
                                let cached = Self::ensure_app_root()
                                    .join("received_files")
                                    .join(normalize_transfer_file_name(&name));
                                if cached.exists() {
                                    let _ = fs::remove_file(cached);
                                }
                                self.ensure_inbound_active_file_row(session_id, &name, size_bytes);
                            }
                            FileTransferSignal::Progress {
                                name,
                                size_bytes,
                                transferred_bytes,
                            } => {
                                self.apply_outbound_file_progress_signal(
                                    session_id,
                                    &name,
                                    size_bytes,
                                    transferred_bytes,
                                );
                            }
                            FileTransferSignal::End {
                                name,
                                size_bytes,
                                ok,
                            } => {
                                self.apply_inbound_file_end_signal(session_id, &name, size_bytes, ok);
                            }
                        }
                        continue;
                    }
                    let local_id = self.alloc_local_msg_id();
                    let i = self.ensure_session_for_inbound(session_id);
                    self.chat_sessions[i].messages.push(ChatMessage {
                        local_id,
                        is_me: false,
                        content: text,
                        timestamp: now_str(),
                        kind: MessageKind::Text,
                        file_name: None,
                        file_path: None,
                        file_size_bytes: None,
                        transferred_bytes: None,
                        send_started_at: None,
                        send_speed_bps: None,
                        recv_speed_bps: None,
                        outbound: None,
                    });
                    persist_sessions = true;
                }
                InboundUiEvent::PeerFile {
                    session_id,
                    name,
                    path,
                    size_bytes,
                } => {
                    let p = path.display().to_string();
                    let row_key = Self::inbound_row_key(session_id, &name);
                    let cached_speed = self.inbound_file_speed_cache.remove(&row_key);
                    let content = self.format_received_file_content(&name, size_bytes, cached_speed);
                    let now = now_str();
                    let local_id =
                        self.resolve_inbound_file_local_id(session_id, &name, size_bytes);
                    let i = self.ensure_session_for_inbound(session_id);
                    if let Some(msg) = self.chat_sessions[i]
                        .messages
                        .iter_mut()
                        .find(|m| m.local_id == local_id)
                    {
                        msg.content = content;
                        msg.timestamp = now;
                        msg.file_name = Some(name.clone());
                        msg.recv_speed_bps = cached_speed.or(msg.recv_speed_bps);
                        msg.file_size_bytes = Some(size_bytes);
                        let tracked = self.inbound_received_bytes.remove(&row_key);
                        msg.transferred_bytes =
                            Some(tracked.or(msg.transferred_bytes).unwrap_or(size_bytes));
                        msg.file_path = Some(p);
                    }
                    // 落盘完成，结束本次「传输会话」路由，下一轮 START 再建新索引
                    self.inbound_received_bytes.remove(&row_key);
                    self.inbound_active_file_row.remove(&row_key);
                    persist_sessions = true;
                }
                InboundUiEvent::PeerFileProgress {
                    session_id,
                    name,
                    size_bytes,
                    received_bytes,
                } => {
                    self.apply_inbound_file_progress(
                        session_id,
                        &name,
                        size_bytes,
                        received_bytes,
                    );
                }
                InboundUiEvent::OutboundSendProgressTick {
                    session_id,
                    local_id,
                    transferred_estimate,
                } => {
                    self.apply_outbound_send_progress_tick(
                        session_id,
                        local_id,
                        transferred_estimate,
                    );
                }
                InboundUiEvent::SendResult {
                    session_id,
                    local_id,
                    ok,
                    detail,
                } => {
                    if let Some(s) = self
                        .chat_sessions
                        .iter_mut()
                        .find(|s| s.id == Some(session_id))
                    {
                        if let Some(m) = s.messages.iter_mut().find(|m| m.local_id == local_id) {
                            if ok {
                                if let (Some(total_bytes), Some(started_at)) =
                                    (m.file_size_bytes, m.send_started_at)
                                {
                                    let elapsed_secs = started_at.elapsed().as_secs_f64().max(0.001);
                                    m.send_speed_bps = Some(total_bytes as f64 / elapsed_secs);
                                }
                                m.transferred_bytes = m.file_size_bytes;
                                m.outbound = Some(OutboundState::Sent);
                            } else {
                                m.outbound = Some(OutboundState::Failed(detail.clone()));
                            }
                            m.send_started_at = None;
                            if let (Some(size_bytes), Some(path)) = (m.file_size_bytes, &m.file_path)
                            {
                                if let Some(file_name) =
                                    Path::new(path).file_name().and_then(|f| f.to_str())
                                {
                                    let key = Self::file_timing_key(
                                        session_id,
                                        &normalize_transfer_file_name(file_name),
                                        size_bytes,
                                    );
                                    self.outbound_file_msg_index.remove(&key);
                                }
                            }
                        }
                    }
                    if ok {
                        self.status = self.tr("发送成功", "Sent successfully").to_string();
                    } else if !ok {
                        self.status =
                            format!("{}: {detail}", self.tr("发送失败", "Send failed"));
                    }
                    persist_sessions = true;
                }
            }
        }
        if persist_sessions {
            self.persist_all_sessions();
        }
        if got_event {
            ctx.request_repaint();
        }
        self.inbound_rx = Some(rx);
    }

    fn consume_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        for file in dropped {
            if let Some(path) = file.path {
                self.pending_file_path = Some(path.display().to_string());
                self.status = format!(
                    "{}: {}",
                    self.tr("已添加文件", "File added"),
                    path.display()
                );
                break;
            }
        }
    }

    fn send_composer(&mut self) {
        let Some(handle) = self.client_handle else {
            self.status = self
                .tr("未连接 webrpc", "Not connected to webrpc")
                .to_string();
            return;
        };
        let Some(index) = self.selected_session else {
            self.status = self
                .tr("请先选择或新建会话", "Please select or create a session first")
                .to_string();
            return;
        };
        let session = &self.chat_sessions[index];
        if !session.ui_connected {
            self.status = self
                .tr("会话未连接，请先连接后再发送", "Session is offline. Connect before sending.")
                .to_string();
            return;
        }
        let Some(sid) = session.id else {
            self.status = self
                .tr("会话未绑定 SDK，请重新连接", "Session not bound to SDK. Please reconnect.")
                .to_string();
            return;
        };
        if let Some(path_str) = self.pending_file_path.clone() {
            let path = PathBuf::from(&path_str);
            if !path.exists() {
                self.status = self
                    .tr("文件不存在，请重新选择", "File does not exist, please choose again")
                    .to_string();
                self.pending_file_path = None;
                return;
            }
            let local_id = self.alloc_local_msg_id();
            let sending_file_text = self.tr("发送文件中", "Sending file").to_string();
            let file_size_bytes = fs::metadata(&path).ok().map(|meta| meta.len());
            let display_name = path
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or(path_str.as_str());
            self.chat_sessions[index].messages.push(ChatMessage {
                local_id,
                is_me: true,
                content: format!("{sending_file_text}: {display_name}"),
                timestamp: now_str(),
                kind: MessageKind::File,
                file_name: Some(display_name.to_string()),
                file_path: Some(path_str.clone()),
                file_size_bytes,
                transferred_bytes: Some(0),
                send_started_at: Some(Instant::now()),
                send_speed_bps: None,
                recv_speed_bps: None,
                outbound: Some(OutboundState::Sending),
            });
            if let Some(size_bytes) = file_size_bytes
                && let Some(file_name) = path.file_name().and_then(|f| f.to_str())
            {
                let key = Self::file_timing_key(
                    sid,
                    &normalize_transfer_file_name(file_name),
                    size_bytes,
                );
                self.outbound_file_msg_index.insert(key, local_id);
            }
            self.status = self.tr("文件发送中…", "File is being sent...").to_string();
            self.pending_file_path = None;
            self.persist_session_at_index(index);

            let tx = self.inbound_tx.clone();
            let send_ok_text = self.tr("发送成功", "Sent successfully").to_string();
            thread::spawn(move || {
                let file_name = Path::new(&path_str)
                    .file_name()
                    .and_then(|f| f.to_str())
                    .unwrap_or("")
                    .to_string();
                let file_name = normalize_transfer_file_name(&file_name);
                let size_bytes = fs::metadata(&path_str).map(|meta| meta.len()).unwrap_or(0);
                let _ = webrpc_send_data(
                    handle,
                    sid,
                    &build_file_transfer_signal_start(&file_name, size_bytes),
                );

                // SendFile 会长时间占用 SDK；对端 PROGRESS 往往要等 SendFile 返回后才能送达。
                // 另起一线程跑 SendFile，本线程每秒推估算进度，气泡才能持续刷新。
                let finished = Arc::new(AtomicBool::new(false));
                let finished_worker = finished.clone();
                let path_for_worker = path_str.clone();
                let worker = thread::spawn(move || {
                    let r = webrpc_send_file(handle, sid, &path_for_worker);
                    finished_worker.store(true, Ordering::Release);
                    r
                });

                let tick_origin = Instant::now();
                loop {
                    thread::sleep(Duration::from_secs(1));
                    if finished.load(Ordering::Acquire) {
                        break;
                    }
                    let elapsed = tick_origin.elapsed().as_secs_f64();
                    let est = estimate_outbound_transferred_bytes(size_bytes, elapsed);
                    if let Some(ref t) = tx {
                        let _ = t.send(InboundUiEvent::OutboundSendProgressTick {
                            session_id: sid,
                            local_id,
                            transferred_estimate: est,
                        });
                    }
                }

                let result = match worker.join() {
                    Ok(r) => r,
                    Err(_) => Err("SendFile 线程异常".to_string()),
                };
                let _ = webrpc_send_data(
                    handle,
                    sid,
                    &build_file_transfer_signal_end(&file_name, size_bytes, result.is_ok()),
                );
                if let Some(t) = tx {
                    let (ok, detail) = match result {
                        Ok(()) => (true, send_ok_text),
                        Err(e) => (false, e),
                    };
                    let _ = t.send(InboundUiEvent::SendResult {
                        session_id: sid,
                        local_id,
                        ok,
                        detail,
                    });
                }
            });
            return;
        }

        let content = self.composer_input.trim().to_string();
        if content.is_empty() {
            self.status = self
                .tr("请输入消息或添加文件", "Please enter a message or attach a file")
                .to_string();
            return;
        }
        let local_id = self.alloc_local_msg_id();
        self.chat_sessions[index].messages.push(ChatMessage {
            local_id,
            is_me: true,
            content: content.clone(),
            timestamp: now_str(),
            kind: MessageKind::Text,
            file_name: None,
            file_path: None,
            file_size_bytes: None,
            transferred_bytes: None,
            send_started_at: None,
            send_speed_bps: None,
            recv_speed_bps: None,
            outbound: Some(OutboundState::Sending),
        });
        self.composer_input.clear();
        self.status = self
            .tr("消息发送中…", "Message is being sent...")
            .to_string();
        self.persist_session_at_index(index);

        let tx = self.inbound_tx.clone();
        let send_ok_text = self.tr("发送成功", "Sent successfully").to_string();
        thread::spawn(move || {
            let result = webrpc_send_data(handle, sid, &content);
            if let Some(t) = tx {
                let (ok, detail) = match result {
                    Ok(()) => (true, send_ok_text),
                    Err(e) => (false, e),
                };
                let _ = t.send(InboundUiEvent::SendResult {
                    session_id: sid,
                    local_id,
                    ok,
                    detail,
                });
            }
        });
    }

    fn close_session_at_index(&mut self, index: usize) {
        let Some(handle) = self.client_handle else {
            return;
        };
        let sdk_id = self.chat_sessions.get(index).and_then(|s| s.id);
        self.persist_session_at_index(index);
        if let Some(sid) = sdk_id {
            let close_result = webrpc_close_session(handle, sid);
            self.chat_sessions[index].id = None;
            self.chat_sessions[index].ui_connected = false;
            match close_result {
                Ok(()) => {
                    let kept = self.tr("历史记录已保留", "history kept");
                    self.status = format!(
                        "{} {sid} — {kept}",
                        self.tr("已断开连接", "Disconnected"),
                    );
                }
                Err(e) => {
                    self.status = if self.ui_lang == UiLanguage::Zh {
                        format!("会话 {sid} 已标记为未连接（CloseSession 异常: {e}）")
                    } else {
                        format!("Session {sid} marked offline (CloseSession error: {e})")
                    };
                }
            }
        } else {
            self.chat_sessions.remove(index);
            if self.chat_sessions.is_empty() {
                self.selected_session = None;
            } else {
                self.selected_session =
                    Some(index.min(self.chat_sessions.len().saturating_sub(1)));
            }
            self.status = self
                .tr("已从列表移除该会话", "Session removed from list")
                .to_string();
        }
    }

    fn draw_session_composer_panel(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, connected: bool) {
        egui::Frame::default()
            .fill(egui::Color32::from_rgba_unmultiplied(10, 30, 48, SESSION_ALPHA))
            .stroke(egui::Stroke::new(
                1.0,
                egui::Color32::from_rgba_unmultiplied(95, 200, 240, SESSION_ALPHA),
            ))
            .corner_radius(10.0)
            .inner_margin(egui::Margin::same(10))
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new(self.tr("发送区（文本/文件）", "Send Area (Text/File)"))
                        .strong(),
                );
                if !connected {
                    ui.label(
                        egui::RichText::new(self.tr(
                            "会话未连接，连接成功后才能发送",
                            "Session offline. Connect before sending.",
                        ))
                        .small()
                        .color(egui::Color32::from_rgb(255, 196, 140)),
                    );
                }
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    let sending_file = self.pending_file_path.is_some();
                    let send_label = if sending_file {
                        self.tr("发送文件", "Send File")
                    } else {
                        self.tr("发送消息", "Send Message")
                    };
                    let composer_hint = if sending_file {
                        self
                            .tr(
                                "已选择附件。Enter发送，Ctrl+Enter换行",
                                "Attachment selected. Enter to send, Ctrl+Enter for newline",
                            )
                            .to_string()
                    } else {
                        self
                            .tr(
                                "输入文本（支持多行）。Enter发送，Ctrl+Enter换行",
                                "Type message (multi-line supported). Enter to send, Ctrl+Enter for newline",
                            )
                            .to_string()
                    };
                    let w = (ui.available_width() - 210.0).max(120.0);
                    ui.add_enabled_ui(connected, |ui| {
                        ui.add_sized(
                            [w, 72.0],
                            egui::TextEdit::multiline(&mut self.composer_input)
                                .desired_rows(3)
                                .return_key(egui::KeyboardShortcut::new(
                                    egui::Modifiers::CTRL,
                                    egui::Key::Enter,
                                ))
                                .hint_text(composer_hint),
                        );
                    });
                    if ui
                        .add_enabled(
                            connected,
                            egui::Button::new(
                                egui::RichText::new(self.tr("选择文件", "Choose File"))
                                    .color(egui::Color32::from_rgb(221, 244, 255)),
                            )
                            .fill(egui::Color32::from_rgba_unmultiplied(29, 95, 133, SESSION_ALPHA))
                            .stroke(egui::Stroke::new(
                                1.0,
                                egui::Color32::from_rgba_unmultiplied(111, 198, 230, SESSION_ALPHA),
                            )),
                        )
                        .clicked()
                        && let Some(path) = rfd::FileDialog::new().pick_file()
                    {
                        self.pending_file_path = Some(path.display().to_string());
                    }
                    if ui
                        .add_enabled(
                            connected && !self.screenshot_in_progress,
                            egui::Button::new(
                                egui::RichText::new(if self.screenshot_in_progress {
                                    self.tr("截图中…", "Capturing...")
                                } else {
                                    self.tr("截图", "Screenshot")
                                })
                                .color(egui::Color32::from_rgb(221, 244, 255)),
                            )
                            .fill(egui::Color32::from_rgba_unmultiplied(29, 95, 133, SESSION_ALPHA))
                            .stroke(egui::Stroke::new(
                                1.0,
                                egui::Color32::from_rgba_unmultiplied(111, 198, 230, SESSION_ALPHA),
                            )),
                        )
                        .clicked()
                    {
                        self.begin_desktop_capture(ctx);
                    }
                    if ui
                        .add_enabled(
                            connected,
                            egui::Button::new(
                                egui::RichText::new(send_label)
                                    .color(egui::Color32::from_rgb(235, 249, 255)),
                            )
                            .fill(egui::Color32::from_rgba_unmultiplied(28, 126, 173, SESSION_ALPHA))
                            .stroke(egui::Stroke::new(
                                1.0,
                                egui::Color32::from_rgba_unmultiplied(123, 216, 248, SESSION_ALPHA),
                            )),
                        )
                        .clicked()
                    {
                        self.send_composer();
                    }
                });
                if let Some(path) = self.pending_file_path.clone() {
                    let file_name = Path::new(&path)
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or(&path)
                        .to_string();
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new(self.tr("移除附件", "Remove attachment"))
                                            .color(egui::Color32::from_rgb(255, 235, 242)),
                                    )
                                    .fill(egui::Color32::from_rgba_unmultiplied(
                                        119, 42, 76, SESSION_ALPHA,
                                    ))
                                    .stroke(egui::Stroke::new(
                                        1.0,
                                        egui::Color32::from_rgba_unmultiplied(
                                            215, 115, 156, SESSION_ALPHA,
                                        ),
                                    )),
                                )
                                .clicked()
                            {
                                self.pending_file_path = None;
                            }
                            ui.add_space(8.0);
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(format!(
                                        "{}: {}",
                                        self.tr("附件", "Attachment"),
                                        file_name
                                    ))
                                    .small()
                                    .color(egui::Color32::from_rgb(180, 220, 255)),
                                )
                                .wrap_mode(egui::TextWrapMode::Wrap),
                            );
                        });
                    });
                } else {
                    ui.label(
                        egui::RichText::new(self.tr(
                            "可拖拽文件到聊天窗口，发送按钮将自动走 SendFile",
                            "You can drag files into the chat window; send will use SendFile automatically",
                        ))
                        .small()
                        .weak(),
                    );
                }
            });
        if !self.show_new_session_modal
            && ctx.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.ctrl)
        {
            self.send_composer();
        }
    }

    /// 统一释放 webrpc：先 `CloseSession` 关闭本地仍记录的会话，再 `WebrpcClient_Free` 回收客户端，避免重复释放。
    fn release_webrpc_client(&mut self) {
        if let Some(handle) = self.client_handle.take() {
            for s in &self.chat_sessions {
                if let Some(sid) = s.id {
                    let _ = webrpc_close_session(handle, sid);
                }
            }
            webrpc_free(handle);
        }
    }

}

impl eframe::App for File2FileApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.ensure_style(ctx);
        self.ensure_logo_texture(ctx);
        self.poll_login_worker(ctx);
        if self.current_user.is_none() {
            self.apply_page_alpha_style(ctx, LOGIN_ALPHA);
            self.draw_login_page(ctx);
            return;
        }
        self.apply_page_alpha_style(ctx, SESSION_ALPHA);

        self.poll_screenshot_hotkey(ctx);
        self.poll_open_session_worker(ctx);
        self.poll_screenshot_worker(ctx);
        self.poll_inbound_events(ctx);
        self.maybe_sync_passive_sdk_sessions();
        self.consume_dropped_files(ctx);
        ctx.request_repaint_after(Duration::from_millis(33));
        self.draw_screenshot_editor(ctx);

        if self.show_new_session_modal {
            egui::Window::new(self.tr("新建会话", "New Session"))
                .id(egui::Id::new("new_webrpc_session_modal"))
                .collapsible(false)
                .resizable(true)
                .default_width(420.0)
                .frame(
                    egui::Frame::default()
                        .fill(egui::Color32::from_rgba_unmultiplied(6, 20, 36, SESSION_ALPHA))
                        .stroke(egui::Stroke::new(
                            1.0,
                            egui::Color32::from_rgba_unmultiplied(90, 210, 255, SESSION_ALPHA),
                        ))
                        .corner_radius(10.0)
                        .inner_margin(egui::Margin::same(14)),
                )
                .show(ctx, |ui| {
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        self.show_new_session_modal = false;
                        self.modal_error.clear();
                    }
                    ui.label(self.tr("对端 Token", "Peer Token"));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.modal_peer_token)
                            .desired_width(f32::INFINITY),
                    );
                    ui.add_space(8.0);
                    ui.label(self.tr(
                        "Permission（可留空，与 SDK 一致可传空串）",
                        "Permission (optional, empty string allowed as SDK behavior)",
                    ));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.modal_permission)
                            .desired_width(f32::INFINITY),
                    );
                    if !self.modal_error.is_empty() {
                        ui.add_space(6.0);
                        ui.label(
                            egui::RichText::new(&self.modal_error)
                                .color(egui::Color32::from_rgb(255, 120, 120)),
                        );
                    }
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        let can = !self.modal_peer_token.trim().is_empty() && !self.open_session_busy;
                        if ui
                            .add_enabled(
                                can,
                                egui::Button::new(
                                    egui::RichText::new(if self.open_session_busy {
                                        self.tr("连接中…", "Connecting...")
                                    } else {
                                        self.tr("连接", "Connect")
                                    })
                                    .color(egui::Color32::from_rgb(231, 247, 255)),
                                )
                                .fill(egui::Color32::from_rgb(25, 109, 150))
                                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(111, 209, 243))),
                            )
                            .clicked()
                        {
                            self.begin_open_session();
                        }
                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new(self.tr("取消", "Cancel"))
                                        .color(egui::Color32::from_rgb(222, 232, 255)),
                                )
                                .fill(egui::Color32::from_rgb(53, 60, 113))
                                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(125, 140, 220))),
                            )
                            .clicked()
                        {
                            self.show_new_session_modal = false;
                            self.modal_error.clear();
                        }
                    });
                });
        }

        if self.show_reconnect_confirm {
            let mut close_confirm = false;
            let mut do_connect = false;
            let peer_label = self
                .reconnect_confirm_index
                .and_then(|i| self.chat_sessions.get(i))
                .map(|s| {
                    Self::session_primary_label(
                        &s.remark,
                        &s.peer_token,
                        self.tr("对端", "Peer"),
                    )
                })
                .unwrap_or_default();
            egui::Window::new(self.tr("连接会话", "Connect Session"))
                .id(egui::Id::new("reconnect_session_confirm"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .frame(
                    egui::Frame::default()
                        .fill(egui::Color32::from_rgba_unmultiplied(6, 20, 36, SESSION_ALPHA))
                        .stroke(egui::Stroke::new(
                            1.0,
                            egui::Color32::from_rgba_unmultiplied(90, 210, 255, SESSION_ALPHA),
                        ))
                        .corner_radius(10.0)
                        .inner_margin(egui::Margin::same(14)),
                )
                .show(ctx, |ui| {
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        close_confirm = true;
                    }
                    ui.label(self.tr(
                        "是否连接该会话？连接成功后可继续收发消息。",
                        "Connect to this session? You can send and receive after connected.",
                    ));
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new(format!(
                            "{}: {peer_label}",
                            self.tr("对端 Token", "Peer Token")
                        ))
                        .strong(),
                    );
                    if let Some(err) = self.session_connect_error.as_ref() {
                        ui.add_space(6.0);
                        ui.label(
                            egui::RichText::new(format!(
                                "{}: {err}",
                                self.tr("连接失败", "Connection failed")
                            ))
                            .color(egui::Color32::from_rgb(255, 120, 120)),
                        );
                    }
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        let busy = self.open_session_busy;
                        if ui
                            .add_enabled(
                                !busy,
                                egui::Button::new(if busy {
                                    self.tr("连接中…", "Connecting...")
                                } else {
                                    self.tr("确认连接", "Connect")
                                }),
                            )
                            .clicked()
                        {
                            do_connect = true;
                        }
                        if ui.button(self.tr("取消", "Cancel")).clicked() {
                            close_confirm = true;
                        }
                    });
                });
            if close_confirm {
                self.show_reconnect_confirm = false;
                self.reconnect_confirm_index = None;
                self.session_connect_error = None;
            }
            if do_connect {
                if let Some(idx) = self.reconnect_confirm_index {
                    self.begin_reconnect_session(idx);
                }
            }
        }

        if self.show_session_remark_modal {
            let mut save_remark = false;
            let mut cancel_remark = false;
            let peer_hint = self
                .remark_edit_index
                .and_then(|i| self.chat_sessions.get(i))
                .map(|s| s.peer_token.clone())
                .unwrap_or_default();
            egui::Window::new(self.tr("会话备注", "Session Remark"))
                .id(egui::Id::new("session_remark_modal"))
                .collapsible(false)
                .resizable(true)
                .default_width(400.0)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .frame(
                    egui::Frame::default()
                        .fill(egui::Color32::from_rgba_unmultiplied(6, 20, 36, SESSION_ALPHA))
                        .stroke(egui::Stroke::new(
                            1.0,
                            egui::Color32::from_rgba_unmultiplied(90, 210, 255, SESSION_ALPHA),
                        ))
                        .corner_radius(10.0)
                        .inner_margin(egui::Margin::same(14)),
                )
                .show(ctx, |ui| {
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        cancel_remark = true;
                    }
                    ui.label(self.tr(
                        "为当前会话设置备注（如对方姓名、公司等），便于识别聊天对象。",
                        "Set a remark for this session (e.g. name, company) to identify the contact.",
                    ));
                    if !peer_hint.trim().is_empty() {
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(format!(
                                "{}: {peer_hint}",
                                self.tr("对端 Token", "Peer Token")
                            ))
                            .small()
                            .weak(),
                        );
                    }
                    ui.add_space(8.0);
                    ui.label(self.tr("备注", "Remark"));
                    let remark_hint = self
                        .tr("例如：张三 / 某某公司", "e.g. John / ACME Corp")
                        .to_string();
                    ui.add(
                        egui::TextEdit::singleline(&mut self.remark_edit_draft)
                            .hint_text(remark_hint)
                            .desired_width(f32::INFINITY),
                    );
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new(self.tr("保存", "Save"))
                                        .color(egui::Color32::from_rgb(231, 247, 255)),
                                )
                                .fill(egui::Color32::from_rgb(25, 109, 150)),
                            )
                            .clicked()
                        {
                            save_remark = true;
                        }
                        if ui.button(self.tr("取消", "Cancel")).clicked() {
                            cancel_remark = true;
                        }
                    });
                });
            if save_remark {
                self.save_session_remark();
            } else if cancel_remark {
                self.show_session_remark_modal = false;
                self.remark_edit_index = None;
            }
        }

        egui::TopBottomPanel::top("top")
            .frame(
                egui::Frame::default()
                    .fill(egui::Color32::from_rgba_unmultiplied(9, 20, 34, SESSION_ALPHA))
                    .stroke(egui::Stroke::new(
                        1.0,
                        egui::Color32::from_rgba_unmultiplied(60, 160, 210, SESSION_ALPHA),
                    ))
                    .inner_margin(egui::Margin::symmetric(14, 10)),
            )
            .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if let Some(logo) = self.logo_texture.as_ref() {
                    ui.add(
                        egui::Image::new((logo.id(), logo.size_vec2()))
                            .fit_to_exact_size(egui::vec2(TOPBAR_LOGO_SIZE[0], TOPBAR_LOGO_SIZE[1])),
                    );
                } else {
                    ui.heading(egui::RichText::new(self.tr("企业通信客户端", "Enterprise Chat Client")).strong());
                }
                ui.separator();
                ui.label(format!(
                    "{}: {}",
                    self.tr("当前", "Current"),
                    self.current_user.as_deref().unwrap_or(self.tr("未知", "Unknown"))
                ));
                if let Some(h) = self.client_handle {
                    let n = webrpc_session_size(h);
                    let login_perm = if self.active_login_permission.is_empty() {
                        self.tr("空", "empty").to_string()
                    } else {
                        self.active_login_permission.clone()
                    };
                    ui.label(format!(
                        "{}: {n} · {}: {}",
                        self.tr("SDK 会话数", "SDK sessions"),
                        self.tr("登录口令", "Login permission"),
                        login_perm
                    ));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let lang_btn = match self.ui_lang {
                        UiLanguage::Zh => "EN",
                        UiLanguage::En => "中文",
                    };
                    if ui
                        .add_sized(
                            [64.0, 30.0],
                            egui::Button::new(lang_btn).wrap_mode(egui::TextWrapMode::Extend),
                        )
                        .clicked()
                    {
                        self.ui_lang = match self.ui_lang {
                            UiLanguage::Zh => UiLanguage::En,
                            UiLanguage::En => UiLanguage::Zh,
                        };
                    }
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new(self.tr("退出登录", "Logout"))
                                    .color(egui::Color32::from_rgb(255, 232, 239)),
                            )
                            .fill(egui::Color32::from_rgb(133, 38, 73))
                            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(226, 114, 157))),
                        )
                        .clicked()
                    {
                        self.login_rx = None;
                        self.open_session_rx = None;
                        self.open_session_busy = false;
                        self.inbound_rx = None;
                        self.inbound_tx = None;
                        self.is_logging_in = false;
                        self.persist_all_sessions();
                        self.release_webrpc_client();
                        self.chat_sessions.clear();
                        self.selected_session = None;
                        self.show_new_session_modal = false;
                        self.show_reconnect_confirm = false;
                        self.reconnect_confirm_index = None;
                        self.open_session_target_index = None;
                        self.session_connect_error = None;
                        self.show_session_remark_modal = false;
                        self.remark_edit_index = None;
                        self.current_user = None;
                        self.active_login_permission.clear();
                        self.refill_login_from_cache();
                        self.login_message.clear();
                        self.login_error = false;
                        self.composer_input.clear();
                        self.pending_file_path = None;
                        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                            480.0, 520.0,
                        )));
                        ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(egui::vec2(
                            480.0, 520.0,
                        )));
                    }
                });
            });
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(&self.status).weak());
            });
        });

        egui::SidePanel::left("sessions")
            .resizable(false)
            .default_width(280.0)
            .min_width(280.0)
            .max_width(280.0)
            .frame(
                egui::Frame::default()
                    .fill(egui::Color32::from_rgba_unmultiplied(8, 28, 44, SESSION_ALPHA))
                    .stroke(egui::Stroke::new(
                        1.0,
                        egui::Color32::from_rgba_unmultiplied(70, 190, 240, SESSION_ALPHA),
                    ))
                    .inner_margin(egui::Margin::same(12)),
            )
            .show(ctx, |ui| {
                ui.heading(
                    egui::RichText::new(self.tr("会话", "Sessions"))
                        .color(egui::Color32::from_rgb(175, 232, 255)),
                );
                ui.add_space(6.0);
                if ui
                    .add_sized(
                        [ui.available_width(), 36.0],
                        egui::Button::new(self.tr("＋ 新建会话", "+ New Session"))
                            .fill(egui::Color32::from_rgba_unmultiplied(18, 90, 130, SESSION_ALPHA))
                            .stroke(egui::Stroke::new(
                                1.0,
                                egui::Color32::from_rgba_unmultiplied(120, 210, 255, SESSION_ALPHA),
                            )),
                    )
                    .clicked()
                {
                    self.show_new_session_modal = true;
                    self.modal_error.clear();
                }
                ui.add_space(10.0);
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for i in 0..self.chat_sessions.len() {
                        let (sdk_id, cached_peer, connected, remark) = {
                            let s = &self.chat_sessions[i];
                            (
                                s.id,
                                s.peer_token.clone(),
                                s.ui_connected,
                                s.remark.clone(),
                            )
                        };
                        let peer = if let Some(sid) = sdk_id {
                            let resolved = self.peer_token_for_session_info(sid, &cached_peer);
                            if resolved != cached_peer {
                                self.chat_sessions[i].peer_token = resolved.clone();
                            }
                            resolved
                        } else {
                            cached_peer
                        };
                        let peer_fallback = self.tr("对端", "Peer");
                        let primary = Self::session_primary_label(&remark, &peer, peer_fallback);
                        let is_selected = self.selected_session == Some(i);
                        let status_line = if connected {
                            format!("SID {}", sdk_id.unwrap_or(0))
                        } else if sdk_id.is_some() {
                            self.tr("待确认", "Pending").to_string()
                        } else {
                            self.tr("未连接", "Offline").to_string()
                        };
                        let mut label_lines = vec![primary];
                        if let Some(token_line) = Self::session_subtitle_token(&remark, &peer) {
                            label_lines.push(token_line);
                        }
                        label_lines.push(status_line);
                        let label = label_lines.join("\n");
                        let card_fill = if is_selected {
                            if connected {
                                egui::Color32::from_rgba_unmultiplied(28, 100, 145, SESSION_ALPHA)
                            } else {
                                egui::Color32::from_rgba_unmultiplied(52, 58, 92, SESSION_ALPHA)
                            }
                        } else if connected {
                            egui::Color32::from_rgba_unmultiplied(12, 52, 78, SESSION_ALPHA)
                        } else {
                            egui::Color32::from_rgba_unmultiplied(28, 32, 58, SESSION_ALPHA)
                        };
                        let text_color = if connected {
                            egui::Color32::from_rgb(214, 244, 255)
                        } else {
                            egui::Color32::from_rgb(178, 186, 214)
                        };
                        let row_h = 58.0;
                        const REMARK_BTN_W: f32 = 44.0;
                        ui.horizontal(|ui| {
                            let dot_col_w = 22.0;
                            let (dot_area, _) = ui.allocate_exact_size(
                                egui::vec2(dot_col_w, row_h),
                                egui::Sense::hover(),
                            );
                            let dot_center = egui::pos2(
                                dot_area.left() + dot_col_w * 0.5,
                                dot_area.center().y,
                            );
                            Self::paint_session_connection_dot(ui, dot_center, connected);
                            let session_btn_w =
                                (ui.available_width() - REMARK_BTN_W - 6.0).max(48.0);
                            if ui
                                .add_sized(
                                    [session_btn_w, row_h],
                                    egui::Button::new(
                                        egui::RichText::new(label).size(14.0).color(text_color),
                                    )
                                    .fill(card_fill)
                                    .stroke(egui::Stroke::new(
                                        1.0,
                                        if is_selected {
                                            egui::Color32::from_rgba_unmultiplied(
                                                130, 230, 255, SESSION_ALPHA,
                                            )
                                        } else if connected {
                                            egui::Color32::from_rgba_unmultiplied(
                                                55, 140, 185, SESSION_ALPHA,
                                            )
                                        } else {
                                            egui::Color32::from_rgba_unmultiplied(
                                                90, 98, 140, SESSION_ALPHA,
                                            )
                                        },
                                    )),
                                )
                                .clicked()
                            {
                                self.selected_session = Some(i);
                                self.session_connect_error = None;
                            }
                            if ui
                                .add_sized(
                                    [REMARK_BTN_W, row_h],
                                    egui::Button::new(
                                        egui::RichText::new(self.tr("备注", "Remark"))
                                            .size(12.0)
                                            .color(egui::Color32::from_rgb(210, 228, 248)),
                                    )
                                    .fill(egui::Color32::from_rgba_unmultiplied(40, 72, 98, SESSION_ALPHA))
                                    .stroke(egui::Stroke::new(
                                        1.0,
                                        egui::Color32::from_rgba_unmultiplied(90, 160, 200, SESSION_ALPHA),
                                    )),
                                )
                                .clicked()
                            {
                                self.selected_session = Some(i);
                                self.open_session_remark_editor(i);
                            }
                        });
                        ui.add_space(4.0);
                    }
                });
            });

        egui::CentralPanel::default()
            .frame(
                egui::Frame::default()
                    .fill(egui::Color32::from_rgba_unmultiplied(20, 19, 47, SESSION_ALPHA))
                    .stroke(egui::Stroke::new(
                        1.0,
                        egui::Color32::from_rgba_unmultiplied(130, 165, 255, SESSION_ALPHA),
                    ))
                    .inner_margin(egui::Margin::same(14)),
            )
            .show(ctx, |ui| {
            if let Some(index) = self.selected_session {
                let meta = self.chat_sessions.get(index).map(|s| {
                    (
                        s.id,
                        s.peer_token.clone(),
                        s.permission.clone(),
                        s.remark.clone(),
                        s.messages.clone(),
                        s.ui_connected,
                    )
                });
                if let Some((sdk_id, fallback_peer, perm, remark, messages, connected)) = meta {
                    let peer_token = if let Some(sid) = sdk_id {
                        self.peer_token_for_session_info(sid, &fallback_peer)
                    } else {
                        fallback_peer
                    };
                    let close_idx = index;
                    let display_title = Self::session_primary_label(
                        &remark,
                        &peer_token,
                        self.tr("对端", "Peer"),
                    );
                    ui.horizontal(|ui| {
                        let (dot_area, _) = ui.allocate_exact_size(
                            egui::vec2(18.0, 28.0),
                            egui::Sense::hover(),
                        );
                        Self::paint_session_connection_dot(ui, dot_area.center(), connected);
                        ui.heading(
                            egui::RichText::new(format!(
                                "{} {display_title}",
                                self.tr("与", "Chat with")
                            ))
                            .color(egui::Color32::from_rgb(215, 218, 255)),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new(self.tr("关闭此会话", "Close this session"))
                                            .color(egui::Color32::from_rgb(255, 232, 238)),
                                    )
                                    .fill(egui::Color32::from_rgb(124, 36, 71))
                                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(222, 111, 157))),
                                )
                                .clicked()
                            {
                                self.close_session_at_index(close_idx);
                            }
                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new(self.tr("备注", "Remark"))
                                            .color(egui::Color32::from_rgb(221, 244, 255)),
                                    )
                                    .fill(egui::Color32::from_rgba_unmultiplied(29, 95, 133, SESSION_ALPHA))
                                    .stroke(egui::Stroke::new(
                                        1.0,
                                        egui::Color32::from_rgba_unmultiplied(111, 198, 230, SESSION_ALPHA),
                                    )),
                                )
                                .clicked()
                            {
                                self.open_session_remark_editor(close_idx);
                            }
                        });
                    });
                    if let Some(token_line) = Self::session_subtitle_token(&remark, &peer_token) {
                        ui.label(
                            egui::RichText::new(format!("Token: {token_line}"))
                                .small()
                                .weak(),
                        );
                    }
                    let remark_meta = if remark.trim().is_empty() {
                        String::new()
                    } else {
                        format!(
                            "{}: {} · ",
                            self.tr("备注", "Remark"),
                            remark.trim()
                        )
                    };
                    let perm_text = if perm.is_empty() {
                        self.tr("（空）", "(empty)").to_string()
                    } else {
                        perm
                    };
                    let session_meta = if connected {
                        format!(
                            "{}{}: {} · {}: {} · Permission: {}",
                            remark_meta,
                            self.tr("状态", "Status"),
                            self.tr("已连接", "Connected"),
                            self.tr("会话 ID", "Session ID"),
                            sdk_id.unwrap_or(0),
                            perm_text
                        )
                    } else if sdk_id.is_some() {
                        format!(
                            "{}{}: {} · Permission: {}",
                            remark_meta,
                            self.tr("状态", "Status"),
                            self.tr("对端已连入，待确认", "Peer connected, pending accept"),
                            perm_text
                        )
                    } else {
                        format!(
                            "{}{}: {} · Permission: {}",
                            remark_meta,
                            self.tr("状态", "Status"),
                            self.tr("未连接（仅显示本地历史）", "Offline (local history only)"),
                            perm_text
                        )
                    };
                    ui.label(egui::RichText::new(session_meta).weak().small());
                    if !connected {
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(self.tr(
                                    "点击左侧会话或下方按钮可重新连接",
                                    "Click the session in the sidebar or use the button below to reconnect",
                                ))
                                .color(egui::Color32::from_rgb(255, 210, 130)),
                            );
                            if ui
                                .add_enabled(
                                    !self.open_session_busy,
                                    egui::Button::new(if self.open_session_busy {
                                        self.tr("连接中…", "Connecting...")
                                    } else {
                                        self.tr("连接此会话", "Connect")
                                    }),
                                )
                                .clicked()
                            {
                                self.reconnect_confirm_index = Some(close_idx);
                                self.show_reconnect_confirm = true;
                                self.session_connect_error = None;
                            }
                        });
                        if let Some(err) = self.session_connect_error.as_ref() {
                            ui.label(
                                egui::RichText::new(format!(
                                    "{}: {err}",
                                    self.tr("连接失败", "Connection failed")
                                ))
                                .color(egui::Color32::from_rgb(255, 120, 120)),
                            );
                        }
                    }
                    ui.separator();
                    let mut open_err: Option<String> = None;
                    // 预留底部发送区高度，避免消息列表被挤出可视范围。
                    let reserved_for_sender = if self.pending_file_path.is_some() {
                        190.0
                    } else {
                        158.0
                    };
                    let list_h = (ui.available_height() - reserved_for_sender).max(120.0);
                    egui::ScrollArea::vertical()
                        .auto_shrink([false; 2])
                        .stick_to_bottom(true)
                        .max_height(list_h)
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 10.0;
                            for msg in &messages {
                                let (
                                    who,
                                    bubble_fill,
                                    text_color,
                                    meta_color,
                                    avatar_fill,
                                    status_ok_color,
                                    status_warn_color,
                                    status_err_color,
                                ) = if msg.is_me {
                                    (
                                        self.tr("我", "Me"),
                                        egui::Color32::from_rgba_unmultiplied(30, 136, 229, SESSION_ALPHA),
                                        egui::Color32::from_rgb(252, 254, 255),
                                        egui::Color32::from_rgb(216, 232, 255),
                                        egui::Color32::from_rgba_unmultiplied(20, 92, 173, SESSION_ALPHA),
                                        egui::Color32::from_rgb(182, 255, 207),
                                        egui::Color32::from_rgb(255, 232, 158),
                                        egui::Color32::from_rgb(255, 196, 196),
                                    )
                                } else {
                                    (
                                        self.tr("对方", "Peer"),
                                        egui::Color32::from_rgba_unmultiplied(236, 240, 247, SESSION_ALPHA),
                                        egui::Color32::from_rgb(24, 30, 44),
                                        egui::Color32::from_rgb(98, 108, 125),
                                        egui::Color32::from_rgba_unmultiplied(94, 113, 142, SESSION_ALPHA),
                                        egui::Color32::from_rgb(72, 180, 120),
                                        egui::Color32::from_rgb(220, 160, 60),
                                        egui::Color32::from_rgb(220, 90, 90),
                                    )
                                };
                                let row_layout = if msg.is_me {
                                    egui::Layout::right_to_left(egui::Align::TOP)
                                } else {
                                    egui::Layout::left_to_right(egui::Align::TOP)
                                };
                                ui.with_layout(row_layout, |ui| {
                                    let bubble_max = (ui.available_width() * 0.58).min(420.0);
                                    egui::Frame::default()
                                        .fill(avatar_fill)
                                        .corner_radius(999.0)
                                        .inner_margin(egui::Margin::symmetric(10, 6))
                                        .show(ui, |ui| {
                                            ui.label(
                                                egui::RichText::new(if msg.is_me {
                                                    self.tr("我", "M")
                                                } else {
                                                    self.tr("对", "P")
                                                })
                                                    .strong()
                                                    .color(egui::Color32::from_rgb(246, 249, 255)),
                                            );
                                        });
                                    ui.add_space(8.0);
                                    egui::Frame::default()
                                        .fill(bubble_fill)
                                        .corner_radius(12.0)
                                        .inner_margin(egui::Margin::symmetric(14, 10))
                                        .show(ui, |ui| {
                                            ui.set_max_width(bubble_max);
                                            if matches!(msg.kind, MessageKind::Text) {
                                                ui.horizontal(|ui| {
                                                    ui.label(
                                                        egui::RichText::new(who)
                                                            .small()
                                                            .strong()
                                                            .color(text_color),
                                                    );
                                                    ui.label(
                                                        egui::RichText::new(&msg.timestamp)
                                                            .small()
                                                            .color(meta_color),
                                                    );
                                                    if msg.is_me {
                                                        if let Some(state) = &msg.outbound {
                                                            match state {
                                                                OutboundState::Sending => {
                                                                    ui.label(
                                                                        egui::RichText::new(self.tr(
                                                                            "发送中",
                                                                            "Sending",
                                                                        ))
                                                                            .small()
                                                                            .color(status_warn_color),
                                                                    );
                                                                }
                                                                OutboundState::Sent => {
                                                                    ui.label(
                                                                        egui::RichText::new(self.tr(
                                                                            "已发送",
                                                                            "Sent",
                                                                        ))
                                                                            .small()
                                                                            .color(status_ok_color),
                                                                    );
                                                                }
                                                                OutboundState::Failed(detail) => {
                                                                    ui.label(
                                                                        egui::RichText::new(format!(
                                                                            "{}: {detail}",
                                                                            self.tr(
                                                                                "发送失败",
                                                                                "Send failed",
                                                                            )
                                                                        ))
                                                                        .small()
                                                                        .color(status_err_color),
                                                                    );
                                                                }
                                                            }
                                                        }
                                                    }
                                                });
                                            }
                                            match msg.kind {
                                                MessageKind::Text => {
                                                    ui.add(
                                                        egui::Label::new(
                                                            egui::RichText::new(&msg.content)
                                                                .color(text_color)
                                                                .size(16.5),
                                                        )
                                                        .wrap_mode(egui::TextWrapMode::Wrap),
                                                    );
                                                }
                                                MessageKind::File => {
                                                    let role = if msg.is_me {
                                                        self.tr("发送", "Send")
                                                    } else {
                                                        self.tr("接收", "Receive")
                                                    };
                                                    let speed_text = if msg.is_me {
                                                        msg.send_speed_bps
                                                            .map(format_transfer_speed)
                                                            .unwrap_or_else(|| {
                                                                self.tr("计算中…", "calculating…")
                                                                    .to_string()
                                                            })
                                                    } else {
                                                        msg.recv_speed_bps
                                                            .map(format_transfer_speed)
                                                            .unwrap_or_else(|| {
                                                                self.tr("计算中…", "calculating…")
                                                                    .to_string()
                                                            })
                                                    };
                                                    let transferred_bytes =
                                                        msg.transferred_bytes.unwrap_or(0);
                                                    let transferred_human =
                                                        format_file_size(transferred_bytes);
                                                    let transferred_text = if msg.is_me {
                                                        format!(
                                                            "{}: {} ({} B)",
                                                            self.tr("已发送", "Sent"),
                                                            transferred_human,
                                                            transferred_bytes
                                                        )
                                                    } else {
                                                        format!(
                                                            "{}: {} ({} B)",
                                                            self.tr("已接收", "Received"),
                                                            transferred_human,
                                                            transferred_bytes
                                                        )
                                                    };
                                                    let state_text = if msg.is_me {
                                                        match msg.outbound.as_ref() {
                                                            Some(OutboundState::Sending) => (
                                                                self.tr("发送中", "Sending"),
                                                                status_warn_color,
                                                            ),
                                                            Some(OutboundState::Sent) => (
                                                                self.tr("完成", "Done"),
                                                                status_ok_color,
                                                            ),
                                                            Some(OutboundState::Failed(_)) => (
                                                                self.tr("失败", "Failed"),
                                                                status_err_color,
                                                            ),
                                                            None => (
                                                                self.tr("发送中", "Sending"),
                                                                status_warn_color,
                                                            ),
                                                        }
                                                    } else if msg.file_path.is_some() {
                                                        (self.tr("完成", "Done"), status_ok_color)
                                                    } else {
                                                        (
                                                            self.tr("接收中", "Receiving"),
                                                            status_warn_color,
                                                        )
                                                    }
                                                    .0;
                                                    // 第1行：单行文本（非列布局）
                                                    ui.scope(|ui| {
                                                        let line1 = format!(
                                                            "{} {} | {} | {} | {}",
                                                            role, &msg.timestamp, speed_text, transferred_text, state_text
                                                        );
                                                        ui.add(
                                                            egui::Label::new(
                                                                egui::RichText::new(line1)
                                                                    .small()
                                                                    .color(meta_color),
                                                            )
                                                            .wrap_mode(egui::TextWrapMode::Truncate),
                                                        );
                                                    });

                                                    // 第2行：文件名（自动换行）
                                                    let file_name = msg.file_name.as_deref().unwrap_or(
                                                        msg.content
                                                            .split(':')
                                                            .next_back()
                                                            .map(str::trim)
                                                            .unwrap_or(&msg.content),
                                                    );
                                                    ui.scope(|ui| {
                                                        ui.add(
                                                            egui::Label::new(
                                                                egui::RichText::new(file_name)
                                                                    .color(text_color)
                                                                    .size(16.5),
                                                            )
                                                            .wrap_mode(egui::TextWrapMode::Wrap),
                                                        );
                                                    });

                                                    // 第3行：绿色下划线“打开”
                                                    let open_label = egui::RichText::new(self.tr("打开", "Open"))
                                                        .small()
                                                        .underline()
                                                        .color(egui::Color32::from_rgb(139, 58, 98));
                                                    ui.scope(|ui| {
                                                        if let Some(path) = &msg.file_path {
                                                            let open_clicked = ui
                                                                .add(
                                                                    egui::Label::new(open_label)
                                                                        .sense(egui::Sense::click()),
                                                                )
                                                                .clicked();
                                                            if open_clicked {
                                                                let target = Path::new(path)
                                                                    .parent()
                                                                    .map(|p| p.to_path_buf())
                                                                    .unwrap_or_else(|| PathBuf::from(path));
                                                                if let Err(err) = opener::open(target) {
                                                                    open_err = Some(format!(
                                                                        "{}: {err}",
                                                                        self.tr("打开失败", "Open failed")
                                                                    ));
                                                                }
                                                            }
                                                        } else {
                                                            ui.label(open_label.weak());
                                                        }
                                                    });
                                                }
                                            }
                                        });
                                });
                            }
                        });

                    if let Some(err) = open_err {
                        self.status = err;
                    }
                    ui.add_space(8.0);
                    ui.separator();
                    self.draw_session_composer_panel(ctx, ui, connected);
                } else {
                    ui.label(self.tr(
                        "会话不存在，请在左侧新建会话。",
                        "Session not found, please create one from the left panel.",
                    ));
                }
            } else {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.heading(self.tr("请选择或新建会话", "Please select or create a session"));
                    ui.label(self.tr(
                        "点击左侧「新建会话」，输入对端 Token 与 Permission 后连接。",
                        "Click \"New Session\" on the left, then enter peer Token and Permission to connect.",
                    ));
                });
                ui.add_space(12.0);
                ui.separator();
                ui.group(|ui| {
                    ui.label(
                        egui::RichText::new(self.tr("发送区（未选择会话）", "Send Area (No Session Selected)"))
                            .strong(),
                    );
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        let w = (ui.available_width() - 210.0).max(120.0);
                        let no_session_hint = self
                            .tr("请先在左侧选择会话", "Please select a session from the left first")
                            .to_string();
                        ui.add_enabled_ui(false, |ui| {
                            ui.add_sized(
                                [w, 72.0],
                                egui::TextEdit::multiline(&mut self.composer_input)
                                    .desired_rows(3)
                                    .return_key(egui::KeyboardShortcut::new(
                                        egui::Modifiers::CTRL,
                                        egui::Key::Enter,
                                    ))
                                    .hint_text(no_session_hint),
                            );
                            let _ = ui.button(self.tr("选择文件", "Choose File"));
                            let _ = ui.button(if self.pending_file_path.is_some() {
                                self.tr("发送文件", "Send File")
                            } else {
                                self.tr("发送消息", "Send Message")
                            });
                        });
                    });
                    if let Some(path) = self.pending_file_path.as_ref() {
                        ui.label(
                            egui::RichText::new(format!(
                                "{}: {}",
                                self.tr("已拖拽附件（待选择会话后发送）", "Attachment dropped (send after selecting session)"),
                                path
                            ))
                                .small()
                                .weak(),
                        );
                    } else {
                        ui.label(
                            egui::RichText::new(self.tr(
                                "支持拖拽文件到窗口，选择会话后点击发送",
                                "Drag files into this window, then select session and click send",
                            ))
                                .small()
                                .weak(),
                        );
                    }
                });
            }
        });
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.release_webrpc_client();
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        // 透明清屏色：让未被面板覆盖的区域透出系统桌面/后景窗口。
        egui::Color32::TRANSPARENT.to_normalized_gamma_f32()
    }
}

impl Drop for File2FileApp {
    fn drop(&mut self) {
        self.release_webrpc_client();
    }
}

fn now_str() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

fn format_file_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

/// SendFile 阻塞、对端 PROGRESS 进不来时的「已传字节」保守曲线（单调、不超过 size），仅用于发送气泡。
fn estimate_outbound_transferred_bytes(size_bytes: u64, elapsed_secs: f64) -> u64 {
    if size_bytes == 0 {
        return 0;
    }
    let elapsed = elapsed_secs.max(0.001);
    // 按约 3 MiB/s 估算目标时长，让中小文件更快脱离“计算中”，同时保持单调且不过满。
    let ref_rate: f64 = 3.0 * 1024.0 * 1024.0;
    let t_ref = (size_bytes as f64 / ref_rate).max(1.5).min(300.0);
    // 使用平滑曲线：前段起步快、后段逐渐放缓，避免体感“卡在 0”或“太快冲满”。
    let x = (elapsed / t_ref).clamp(0.0, 1.0);
    let smooth = x * x * (3.0 - 2.0 * x); // smoothstep
    let p = (0.02 + 0.968 * smooth).min(0.988);
    let est = (size_bytes as f64 * p).round() as u64;
    est.min(size_bytes)
}

fn load_screenshot_editor_state(path: &Path) -> Result<ScreenshotEditorState, String> {
    let img = image::open(path)
        .map_err(|e| format!("读取截图失败: {e}"))?
        .to_rgba8();
    Ok(ScreenshotEditorState {
        source_image: img,
        texture: None,
        crop_rect: None,
        actions: Vec::new(),
        tool: ScreenshotTool::Crop,
        pending_drag_start: None,
        pending_drag_now: None,
        text_input: "note".to_string(),
        selection_done: false,
    })
}

fn ensure_editor_texture(ctx: &egui::Context, editor: &mut ScreenshotEditorState) {
    if editor.texture.is_some() {
        return;
    }
    let size = [editor.source_image.width() as usize, editor.source_image.height() as usize];
    let pixels = editor.source_image.as_raw();
    let image = egui::ColorImage::from_rgba_unmultiplied(size, pixels);
    editor.texture = Some(ctx.load_texture("screenshot-editor", image, Default::default()));
}

fn tool_button(ui: &mut egui::Ui, editor: &mut ScreenshotEditorState, t: ScreenshotTool, label: &str) {
    let selected = editor.tool == t;
    if ui.selectable_label(selected, label).clicked() {
        editor.tool = t;
    }
}

fn map_to_image_pos(rect: egui::Rect, tex_size: egui::Vec2, p: egui::Pos2) -> egui::Pos2 {
    let x = ((p.x - rect.min.x) / rect.width()).clamp(0.0, 1.0) * tex_size.x;
    let y = ((p.y - rect.min.y) / rect.height()).clamp(0.0, 1.0) * tex_size.y;
    egui::pos2(x, y)
}

fn map_from_image_pos(rect: egui::Rect, tex_size: egui::Vec2, p: egui::Pos2) -> egui::Pos2 {
    egui::pos2(
        rect.min.x + (p.x / tex_size.x) * rect.width(),
        rect.min.y + (p.y / tex_size.y) * rect.height(),
    )
}

fn handle_editor_input(
    editor: &mut ScreenshotEditorState,
    response: &egui::Response,
    rect: egui::Rect,
    tex_size: egui::Vec2,
    _scale: f32,
) {
    let effective_tool = if editor.selection_done {
        editor.tool
    } else {
        ScreenshotTool::Crop
    };
    if response.clicked()
        && effective_tool == ScreenshotTool::Text
        && let Some(pos) = response.interact_pointer_pos()
    {
        let image_pos = map_to_image_pos(rect, tex_size, pos);
        let text = editor.text_input.trim().to_string();
        if !text.is_empty() {
            editor.actions.push(ScreenshotAction::Text { pos: image_pos, text });
        }
    }
    if response.drag_started()
        && let Some(pos) = response.interact_pointer_pos()
    {
        let image_pos = map_to_image_pos(rect, tex_size, pos);
        editor.pending_drag_start = Some(image_pos);
        editor.pending_drag_now = Some(image_pos);
    }
    if response.dragged()
        && let Some(pos) = response.interact_pointer_pos()
    {
        editor.pending_drag_now = Some(map_to_image_pos(rect, tex_size, pos));
    }
    if response.drag_stopped()
        && let (Some(start), Some(end)) = (editor.pending_drag_start, editor.pending_drag_now)
    {
        match effective_tool {
            ScreenshotTool::Crop => {
                editor.crop_rect = Some((start, end));
                editor.selection_done = true;
            }
            ScreenshotTool::Rect => editor.actions.push(ScreenshotAction::Rect { start, end }),
            ScreenshotTool::Circle => editor.actions.push(ScreenshotAction::Circle { start, end }),
            ScreenshotTool::Arrow => editor.actions.push(ScreenshotAction::Arrow { start, end }),
            ScreenshotTool::Text => {}
        }
        editor.pending_drag_start = None;
        editor.pending_drag_now = None;
    }
}

fn paint_editor_overlays(
    painter: &egui::Painter,
    editor: &ScreenshotEditorState,
    draw_rect: egui::Rect,
    _scale: f32,
) {
    let tex_size = egui::vec2(
        editor.source_image.width() as f32,
        editor.source_image.height() as f32,
    );
    let stroke = egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 70, 70));
    if let Some((a, b)) = editor.crop_rect {
        let pa = map_from_image_pos(draw_rect, tex_size, a);
        let pb = map_from_image_pos(draw_rect, tex_size, b);
        painter.rect_stroke(
            egui::Rect::from_two_pos(pa, pb),
            0.0,
            egui::Stroke::new(2.0, egui::Color32::from_rgb(90, 220, 255)),
            egui::StrokeKind::Outside,
        );
    }
    for action in &editor.actions {
        match action {
            ScreenshotAction::Rect { start, end } => {
                let pa = map_from_image_pos(draw_rect, tex_size, *start);
                let pb = map_from_image_pos(draw_rect, tex_size, *end);
                painter.rect_stroke(
                    egui::Rect::from_two_pos(pa, pb),
                    0.0,
                    stroke,
                    egui::StrokeKind::Outside,
                );
            }
            ScreenshotAction::Circle { start, end } => {
                let pa = map_from_image_pos(draw_rect, tex_size, *start);
                let pb = map_from_image_pos(draw_rect, tex_size, *end);
                let c = egui::pos2((pa.x + pb.x) * 0.5, (pa.y + pb.y) * 0.5);
                let rx = (pa.x - pb.x).abs() * 0.5;
                let ry = (pa.y - pb.y).abs() * 0.5;
                let r = rx.max(ry);
                painter.circle_stroke(c, r, stroke);
            }
            ScreenshotAction::Arrow { start, end } => {
                let a = map_from_image_pos(draw_rect, tex_size, *start);
                let b = map_from_image_pos(draw_rect, tex_size, *end);
                painter.line_segment([a, b], stroke);
            }
            ScreenshotAction::Text { pos, text } => {
                let p = map_from_image_pos(draw_rect, tex_size, *pos);
                painter.text(
                    p,
                    egui::Align2::LEFT_TOP,
                    text,
                    egui::FontId::proportional(18.0),
                    egui::Color32::from_rgb(255, 70, 70),
                );
            }
        }
    }
    if let (Some(start), Some(now)) = (editor.pending_drag_start, editor.pending_drag_now) {
        let pa = map_from_image_pos(draw_rect, tex_size, start);
        let pb = map_from_image_pos(draw_rect, tex_size, now);
        painter.rect_stroke(
            egui::Rect::from_two_pos(pa, pb),
            0.0,
            egui::Stroke::new(1.0, egui::Color32::YELLOW),
            egui::StrokeKind::Outside,
        );
    }
}

fn render_editor_to_file(editor: &ScreenshotEditorState) -> Result<PathBuf, String> {
    let mut img = editor.source_image.clone();
    let red = Rgba([255u8, 60u8, 60u8, 255u8]);
    for action in &editor.actions {
        match action {
            ScreenshotAction::Rect { start, end } => {
                let x = start.x.min(end.x).max(0.0) as i32;
                let y = start.y.min(end.y).max(0.0) as i32;
                let w = (start.x - end.x).abs().max(1.0) as u32;
                let h = (start.y - end.y).abs().max(1.0) as u32;
                draw_hollow_rect_mut(&mut img, Rect::at(x, y).of_size(w, h), red);
            }
            ScreenshotAction::Circle { start, end } => {
                let cx = ((start.x + end.x) * 0.5).max(0.0) as i32;
                let cy = ((start.y + end.y) * 0.5).max(0.0) as i32;
                let r = ((start.x - end.x).abs().max((start.y - end.y).abs()) * 0.5).max(1.0);
                draw_hollow_circle_mut(&mut img, (cx, cy), r as i32, red);
            }
            ScreenshotAction::Arrow { start, end } => {
                draw_line_segment_mut(&mut img, (start.x, start.y), (end.x, end.y), red);
                let dx = end.x - start.x;
                let dy = end.y - start.y;
                let len = (dx * dx + dy * dy).sqrt().max(1.0);
                let ux = dx / len;
                let uy = dy / len;
                let size = 14.0;
                let left = (end.x - ux * size - uy * size * 0.6, end.y - uy * size + ux * size * 0.6);
                let right = (end.x - ux * size + uy * size * 0.6, end.y - uy * size - ux * size * 0.6);
                draw_line_segment_mut(&mut img, (end.x, end.y), left, red);
                draw_line_segment_mut(&mut img, (end.x, end.y), right, red);
            }
            ScreenshotAction::Text { pos, text } => {
                if let Some(font) = load_annotation_font() {
                    draw_text_mut(
                        &mut img,
                        red,
                        pos.x.max(0.0) as i32,
                        pos.y.max(0.0) as i32,
                        PxScale::from(26.0),
                        &font,
                        text,
                    );
                }
            }
        }
    }
    if let Some((a, b)) = editor.crop_rect {
        let x = a.x.min(b.x).max(0.0) as u32;
        let y = a.y.min(b.y).max(0.0) as u32;
        let w = (a.x - b.x).abs().max(1.0) as u32;
        let h = (a.y - b.y).abs().max(1.0) as u32;
        let vw = img.width().saturating_sub(x).max(1);
        let vh = img.height().saturating_sub(y).max(1);
        let cw = w.min(vw);
        let ch = h.min(vh);
        img = image::imageops::crop_imm(&img, x, y, cw, ch).to_image();
    }
    let dir = File2FileApp::ensure_app_root().join("screenshots");
    fs::create_dir_all(&dir).map_err(|e| format!("创建截图目录失败: {e}"))?;
    let stamp = Local::now().format("%Y%m%d_%H%M%S_%3f");
    let out = dir.join(format!("screenshot_v3_{stamp}.png"));
    img.save(&out).map_err(|e| format!("保存截图失败: {e}"))?;
    Ok(out)
}

fn load_annotation_font() -> Option<FontArc> {
    let candidates = [
        "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
        "/System/Library/Fonts/PingFang.ttc",
        "C:\\Windows\\Fonts\\msyh.ttc",
        "C:\\Windows\\Fonts\\arial.ttf",
    ];
    for p in candidates {
        if let Ok(bytes) = fs::read(p)
            && let Ok(font) = FontArc::try_from_vec(bytes)
        {
            return Some(font);
        }
    }
    None
}

fn capture_desktop_screenshot_file() -> Result<PathBuf, String> {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        let screens = Screen::all().map_err(|e| format!("列出屏幕失败: {e}"))?;
        let screen = screens
            .into_iter()
            .max_by_key(|s| i64::from(s.display_info.width) * i64::from(s.display_info.height))
            .ok_or_else(|| "未检测到可用屏幕".to_string())?;
        let image = screen.capture().map_err(|e| format!("截图失败: {e}"))?;
        let dir = File2FileApp::ensure_app_root().join("screenshots");
        fs::create_dir_all(&dir).map_err(|e| format!("创建截图目录失败: {e}"))?;
        let stamp = Local::now().format("%Y%m%d_%H%M%S_%3f");
        let file_path = dir.join(format!("screenshot_{stamp}.png"));
        image
            .save(&file_path)
            .map_err(|e| format!("保存截图失败: {e}"))?;
        return Ok(file_path);
    }

    #[cfg(target_os = "linux")]
    {
        use std::process::Command;

        let dir = File2FileApp::ensure_app_root().join("screenshots");
        fs::create_dir_all(&dir).map_err(|e| format!("创建截图目录失败: {e}"))?;
        let stamp = Local::now().format("%Y%m%d_%H%M%S_%3f");
        let file_path = dir.join(format!("screenshot_{stamp}.png"));
        let out = file_path.to_string_lossy().to_string();
        // Linux 下尝试常见截图工具（避免额外链接系统图形库）。
        let ok_grim = Command::new("grim")
            .arg(&out)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok_grim && file_path.exists() {
            return Ok(file_path);
        }
        let ok_gnome = Command::new("gnome-screenshot")
            .args(["-f", &out])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok_gnome && file_path.exists() {
            return Ok(file_path);
        }
        let ok_scrot = Command::new("scrot")
            .arg(&out)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok_scrot && file_path.exists() {
            return Ok(file_path);
        }

        Err(
            "Linux 截图失败：请安装 grim 或 gnome-screenshot 或 scrot，并确保图形会话可用"
                .to_string(),
        )
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        Err("当前平台暂不支持截图".to_string())
    }
}

fn format_transfer_speed(bytes_per_sec: f64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    if bytes_per_sec >= GB {
        format!("{:.2} GB/s", bytes_per_sec / GB)
    } else if bytes_per_sec >= MB {
        format!("{:.2} MB/s", bytes_per_sec / MB)
    } else if bytes_per_sec >= KB {
        format!("{:.2} KB/s", bytes_per_sec / KB)
    } else {
        format!("{:.0} B/s", bytes_per_sec.max(0.0))
    }
}

const FILE_TRANSFER_SIGNAL_PREFIX: &str = "__F2F_FILE_SIGNAL__";

fn encode_signal_field(input: &str) -> String {
    input.replace('%', "%25").replace('|', "%7C")
}

fn decode_signal_field(input: &str) -> String {
    input.replace("%7C", "|").replace("%25", "%")
}

fn normalize_transfer_file_name(input: &str) -> String {
    input.chars()
        .filter(|c| *c != '/' && *c != '\\' && *c != ':' && *c != '\0')
        .collect()
}

fn build_file_transfer_signal_start(file_name: &str, size_bytes: u64) -> String {
    format!(
        "{FILE_TRANSFER_SIGNAL_PREFIX}|START|{}|{size_bytes}",
        encode_signal_field(file_name)
    )
}

fn build_file_transfer_signal_progress(
    file_name: &str,
    size_bytes: u64,
    transferred_bytes: u64,
) -> String {
    format!(
        "{FILE_TRANSFER_SIGNAL_PREFIX}|PROGRESS|{}|{size_bytes}|{transferred_bytes}",
        encode_signal_field(file_name)
    )
}

fn build_file_transfer_signal_end(file_name: &str, size_bytes: u64, ok: bool) -> String {
    let status = if ok { "OK" } else { "FAIL" };
    format!(
        "{FILE_TRANSFER_SIGNAL_PREFIX}|END|{}|{size_bytes}|{status}",
        encode_signal_field(file_name)
    )
}

fn parse_file_transfer_signal(text: &str) -> Option<FileTransferSignal> {
    let mut parts = text.split('|');
    if parts.next()? != FILE_TRANSFER_SIGNAL_PREFIX {
        return None;
    }
    let kind = parts.next()?;
    let name = decode_signal_field(parts.next()?);
    let size_bytes = parts.next()?.parse::<u64>().ok()?;
    match kind {
        "START" => Some(FileTransferSignal::Start { name, size_bytes }),
        "PROGRESS" => Some(FileTransferSignal::Progress {
            name,
            size_bytes,
            transferred_bytes: parts.next()?.parse::<u64>().ok()?,
        }),
        "END" => {
            let ok = matches!(parts.next(), Some("OK"));
            Some(FileTransferSignal::End {
                name,
                size_bytes,
                ok,
            })
        }
        _ => None,
    }
}

/// 与 Go 示例一致：`New` 后周期查询 `LoginStatus`，直到返回值 **非 0**，再调用 `GetReceivePort`。
#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
fn login_worker_blocking(
    token: String,
    password: String,
    permission: String,
) -> Result<(usize, i32), String> {
    let token_c = CString::new(token).map_err(|_| "Token 含 NUL 非法".to_string())?;
    let pass_c = CString::new(password).map_err(|_| "密码含 NUL 非法".to_string())?;
    let perm_c = CString::new(permission).map_err(|_| "口令含 NUL 非法".to_string())?;

    let handle = unsafe {
        WebrpcClient_New(
            token_c.as_ptr() as *mut c_char,
            pass_c.as_ptr() as *mut c_char,
            perm_c.as_ptr() as *mut c_char,
        )
    };
    if handle == 0 {
        return Err("SDK 返回空句柄".to_string());
    }

    // 轮询期间保持 CString 存活（与 Go 在循环前创建 C 字符串、结束前不释放一致）。
    let _hold = (token_c, pass_c, perm_c);

    for _ in 0..20 {
        let status = unsafe { WebrpcClient_LoginStatus(handle) };
        if status != 0 {
            let port = unsafe { WebrpcClient_GetReceivePort(handle) };
            return Ok((handle, port));
        }
        thread::sleep(Duration::from_millis(500));
    }
    unsafe { WebrpcClient_Free(handle) };
    Err("登录等待超时（10 秒内未完成登录，请检查 Token/密码后重试）".to_string())
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn login_worker_blocking(token: String, password: String, permission: String) -> Result<(usize, i32), String> {
    let _ = (token, password, permission);
    Err("当前平台未接入 webrpc（仅支持 macOS/Linux/Windows）".to_string())
}

fn spawn_webrpc_callback_thread(handle: usize, port: i32, inbound_tx: mpsc::Sender<InboundUiEvent>) {
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        let _ = (handle, port, inbound_tx);
        return;
    }
    #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
    {
        if port <= 0 || port > u16::MAX as i32 {
            eprintln!("webrpc: 无效回调 TCP 端口 {port}");
            return;
        }
        let port = port as u16;
        let _ = thread::Builder::new()
            .name("webrpc-callback-tcp".into())
            .spawn(move || webrpc_callback_tcp_loop(handle, port, inbound_tx));
    }
}

/// 与 Go `startReceivingCallbackData` 相同：连接 `127.0.0.1:port`，按大端解析帧。
#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
fn webrpc_callback_tcp_loop(handle: usize, port: u16, inbound_tx: mpsc::Sender<InboundUiEvent>) {
    let addr = format!("127.0.0.1:{port}");
    let mut conn = match TcpStream::connect(&addr) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("webrpc: 连接回调 TCP 失败 {addr}: {e}");
            return;
        }
    };
    eprintln!("webrpc: 已连接回调 TCP {addr}");
    loop {
        let mut sid = [0u8; 4];
        if read_full(&mut conn, &mut sid).is_err() {
            break;
        }
        let session_id = u32::from_be_bytes(sid);
        let mut dt = [0u8; 1];
        if read_full(&mut conn, &mut dt).is_err() {
            break;
        }
        let data_type = dt[0];
        // 与 Go 示例代码分支一致：2 => 数据流，1 => 文件流（注释与实现以代码为准）
        match data_type {
            2 => handle_callback_data_stream(&mut conn, session_id, &inbound_tx),
            1 => handle_callback_file_stream(handle, &mut conn, session_id, &inbound_tx),
            _ => eprintln!("webrpc: 未知 data_type {data_type}"),
        }
    }
    eprintln!("webrpc: 回调 TCP 读取结束");
}

fn read_full<R: Read>(reader: &mut R, buf: &mut [u8]) -> io::Result<()> {
    let mut off = 0;
    while off < buf.len() {
        let n = reader.read(&mut buf[off..])?;
        if n == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "connection closed",
            ));
        }
        off += n;
    }
    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
fn handle_callback_data_stream(
    conn: &mut TcpStream,
    session_id: u32,
    inbound_tx: &mpsc::Sender<InboundUiEvent>,
) {
    let mut len_buf = [0u8; 4];
    if read_full(conn, &mut len_buf).is_err() {
        return;
    }
    let n = u32::from_be_bytes(len_buf) as usize;
    let mut data = vec![0u8; n];
    if read_full(conn, &mut data).is_err() {
        return;
    }
    let text = String::from_utf8_lossy(&data).to_string();
    if n > 0 && n < 1024 {
        eprintln!("webrpc: 数据流 session={session_id} 文本预览={text}");
    }
    let _ = inbound_tx.send(InboundUiEvent::PeerText { session_id, text });
}

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
fn handle_callback_file_stream(
    handle: usize,
    conn: &mut TcpStream,
    session_id: u32,
    inbound_tx: &mpsc::Sender<InboundUiEvent>,
) {
    let mut len_buf = [0u8; 4];
    if read_full(conn, &mut len_buf).is_err() {
        return;
    }
    let name_len = u32::from_be_bytes(len_buf) as usize;
    let mut name_bytes = vec![0u8; name_len];
    if read_full(conn, &mut name_bytes).is_err() {
        return;
    }
    let mut len2 = [0u8; 4];
    if read_full(conn, &mut len2).is_err() {
        return;
    }
    let file_len = u32::from_be_bytes(len2) as usize;
    let name_raw = String::from_utf8_lossy(&name_bytes);
    eprintln!("webrpc: 文件流 session={session_id} 文件名={name_raw} 大小={file_len}");
    let base = File2FileApp::ensure_app_root().join("received_files");
    let _ = fs::create_dir_all(&base);
    let safe = normalize_transfer_file_name(&name_raw);
    if safe.is_empty() {
        return;
    }
    let safe_for_signal = safe.clone();
    let path = base.join(&safe);
    // 回调文件流可能被拆成多段到达；这里把“历史已落盘”作为累计基线。
    let already_saved_bytes = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    let segment_bytes = file_len as u64;
    let mut read_bytes: u64 = 0;
    let started_at = Instant::now();
    let mut last_report = started_at;
    let mut file_data = Vec::with_capacity(file_len);
    let mut chunk = vec![0u8; 64 * 1024];
    while read_bytes < segment_bytes {
        let need = ((segment_bytes - read_bytes) as usize).min(chunk.len());
        let at_segment_start = read_bytes == 0;
        if read_full(conn, &mut chunk[..need]).is_err() {
            return;
        }
        file_data.extend_from_slice(&chunk[..need]);
        read_bytes += need as u64;
        let cumulative_received = already_saved_bytes + read_bytes;
        let cumulative_total = already_saved_bytes + segment_bytes;
        let now = Instant::now();
        // 本段首次读到数据立刻回传 PROGRESS，之后每 1 秒一次，便于发送端气泡持续刷新平均速率。
        let first_byte_report = at_segment_start;
        let periodic_report = now.duration_since(last_report) >= Duration::from_secs(1);
        if first_byte_report || periodic_report {
            let _ = inbound_tx.send(InboundUiEvent::PeerFileProgress {
                session_id,
                name: safe_for_signal.clone(),
                size_bytes: cumulative_total,
                received_bytes: cumulative_received,
            });
            let _ = webrpc_send_data(
                handle,
                session_id,
                &build_file_transfer_signal_progress(
                    &safe_for_signal,
                    cumulative_total,
                    cumulative_received,
                ),
            );
            last_report = now;
        }
    }
    let final_cumulative = already_saved_bytes + segment_bytes;
    let _ = inbound_tx.send(InboundUiEvent::PeerFileProgress {
        session_id,
        name: safe_for_signal.clone(),
        size_bytes: final_cumulative,
        received_bytes: final_cumulative,
    });
    let _ = webrpc_send_data(
        handle,
        session_id,
        &build_file_transfer_signal_progress(&safe_for_signal, final_cumulative, final_cumulative),
    );
    // 文件流可能按多段回调到达；仅在 START 清理一次，这里始终 append，避免后段覆盖前段。
    let write_result = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .and_then(|mut file| file.write_all(&file_data));
    if let Err(e) = write_result {
        eprintln!("webrpc: 保存文件失败 {path:?}: {e}");
    } else {
        let size_bytes = fs::metadata(&path)
            .map(|meta| meta.len())
            .unwrap_or(file_len as u64);
        eprintln!("webrpc: 已追加保存 {path:?}");
        let _ = inbound_tx.send(InboundUiEvent::PeerFile {
            session_id,
            name: safe,
            path,
            size_bytes,
        });
    }

}

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
fn webrpc_free(handle: usize) {
    unsafe { WebrpcClient_Free(handle) };
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn webrpc_free(_handle: usize) {}

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
fn open_session_worker_blocking(
    handle: usize,
    peer_token: String,
    permission: String,
) -> Result<(u32, String, String), String> {
    let peer_c = CString::new(peer_token.clone()).map_err(|_| "对端 Token 含 NUL".to_string())?;
    let perm_c = CString::new(permission.clone()).map_err(|_| "Permission 含 NUL".to_string())?;
    let ret = unsafe {
        WebrpcClient_OpenSession(
            handle,
            peer_c.as_ptr() as *mut c_char,
            perm_c.as_ptr() as *mut c_char,
        )
    };
    let _hold = (peer_c, perm_c);
    if ret <= 0 {
        return Err("OpenSession 返回 0（会话创建失败）".to_string());
    }
    let sid = ret;
    Ok((sid, peer_token, permission))
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn open_session_worker_blocking(
    _handle: usize,
    peer_token: String,
    permission: String,
) -> Result<(u32, String, String), String> {
    let _ = (peer_token, permission);
    Err("当前平台未接入 webrpc".to_string())
}

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
fn webrpc_send_data(handle: usize, session_id: u32, text: &str) -> Result<(), String> {
    let c = CString::new(text).map_err(|_| "消息含 NUL 无法发送".to_string())?;
    let ret = unsafe {
        WebrpcClient_SendData(
            handle,
            session_id,
            c.as_ptr() as *mut c_char,
            text.len() as i32,
            10000,
        )
    };
    if ret == 1 {
        Ok(())
    } else if ret == 0 {
        Err("发送失败（接口返回0）".to_string())
    } else {
        Err(format!("SendData 返回 {ret}（0=失败, 1=成功）"))
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn webrpc_send_data(_handle: usize, _session_id: u32, _text: &str) -> Result<(), String> {
    Err("未接入 webrpc".into())
}

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
fn webrpc_send_file(handle: usize, session_id: u32, path: &str) -> Result<(), String> {
    let c = CString::new(path).map_err(|_| "路径含 NUL".to_string())?;
    let ret = unsafe { WebrpcClient_SendFile(handle, session_id, c.as_ptr() as *mut c_char) };
    if ret == 1 {
        Ok(())
    } else if ret == 0 {
        Err("发送失败（接口返回0）".to_string())
    } else {
        Err(format!("SendFile 返回 {ret}（0=失败, 1=成功）"))
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn webrpc_send_file(_handle: usize, _session_id: u32, _path: &str) -> Result<(), String> {
    Err("未接入 webrpc".into())
}

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
fn webrpc_close_session(handle: usize, session_id: u32) -> Result<(), String> {
    let ret = unsafe { WebrpcClient_CloseSession(handle, session_id) };
    if ret == 1 {
        Ok(())
    } else {
        Err(format!("CloseSession 返回 {ret}（0=失败, 1=成功）"))
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn webrpc_close_session(_handle: usize, _session_id: u32) -> Result<(), String> {
    Err("未接入 webrpc".into())
}

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
fn webrpc_session_size(handle: usize) -> u16 {
    unsafe { WebrpcClient_SessionSize(handle) }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn webrpc_session_size(_handle: usize) -> u16 {
    0
}

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
fn webrpc_tar_token_by_session(handle: usize, session_id: u32) -> Result<String, String> {
    let ptr = unsafe { WebrpcClient_TarTokenBySession(handle, session_id) };
    if ptr.is_null() {
        return Err("TarTokenBySession 返回空指针".to_string());
    }
    let token = unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .trim()
        .to_string();
    Ok(token)
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn webrpc_tar_token_by_session(_handle: usize, _session_id: u32) -> Result<String, String> {
    Err("未接入 webrpc".into())
}

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
unsafe extern "C" {
    fn WebrpcClient_New(token: *mut c_char, passwd: *mut c_char, permission: *mut c_char)
    -> usize;
    fn WebrpcClient_LoginStatus(handle: usize) -> i32;
    fn WebrpcClient_GetReceivePort(handle: usize) -> i32;
    fn WebrpcClient_OpenSession(
        handle: usize,
        to_token: *mut c_char,
        permission: *mut c_char,
    ) -> u32;
    fn WebrpcClient_SessionSize(handle: usize) -> u16;
    fn WebrpcClient_TarTokenBySession(handle: usize, session_id: u32) -> *mut c_char;
    fn WebrpcClient_CloseSession(handle: usize, session_id: u32) -> i32;
    fn WebrpcClient_SendData(
        handle: usize,
        session_id: u32,
        data: *mut c_char,
        data_len: i32,
        time_out: i64,
    ) -> i32;
    fn WebrpcClient_SendFile(handle: usize, session_id: u32, file_path: *mut c_char) -> i32;
    fn WebrpcClient_Free(handle: usize);
}
