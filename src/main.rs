#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use std::ffi::{CStr, CString, c_char};
use std::fs;
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::Local;
use eframe::egui;
use image::ImageReader;
use serde::{Deserialize, Serialize};

/// 登录页固定内容区宽度（像素）
const EMBEDDED_LOGO_BYTES: &[u8] = include_bytes!("../assets/file2file_logo.png");
const EMBEDDED_ICON_BYTES: &[u8] = include_bytes!("../assets/file2file_icon.ico");
const TOPBAR_LOGO_SIZE: [f32; 2] = [170.0, 30.0];
const APP_DATA_DIR_NAME: &str = "file2file_data";

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
        .with_resizable(false);
    if let Some(icon_data) = load_app_icon_data() {
        viewport = viewport.with_icon(icon_data);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "File2File文件传输",
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
    file_path: Option<String>,
    outbound: Option<OutboundState>,
}

/// 与 webrpc `OpenSession` 对应的本地会话（内存态，退出登录后清空）。
#[derive(Debug, Clone)]
struct WebrpcChatSession {
    /// `WebrpcClient_OpenSession` 返回的会话 ID
    id: u32,
    peer_token: String,
    permission: String,
    messages: Vec<ChatMessage>,
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
    SendResult {
        session_id: u32,
        local_id: u64,
        ok: bool,
        detail: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct PersistedData {
    saved_token: Option<String>,
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
    next_local_msg_id: u64,
    ui_lang: UiLanguage,
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

        Self {
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
            next_local_msg_id: 1,
            ui_lang: UiLanguage::Zh,
        }
    }

    fn tr<'a>(&self, zh: &'a str, en: &'a str) -> &'a str {
        match self.ui_lang {
            UiLanguage::Zh => zh,
            UiLanguage::En => en,
        }
    }

    fn alloc_local_msg_id(&mut self) -> u64 {
        let id = self.next_local_msg_id;
        self.next_local_msg_id = self.next_local_msg_id.saturating_add(1);
        id
    }

    fn find_session_index_by_id(&self, sid: u32) -> Option<usize> {
        self.chat_sessions.iter().position(|s| s.id == sid)
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

    fn ensure_session_for_inbound(&mut self, sid: u32) -> usize {
        if let Some(i) = self.find_session_index_by_id(sid) {
            return i;
        }
        let peer_token = self.peer_token_for_session_info(sid, "");
        self.chat_sessions.push(WebrpcChatSession {
            id: sid,
            peer_token,
            permission: String::new(),
            messages: Vec::new(),
        });
        self.chat_sessions.len().saturating_sub(1)
    }

    fn ensure_app_root() -> PathBuf {
        let app_root = user_workspace_dir().join(APP_DATA_DIR_NAME);
        let _ = fs::create_dir_all(&app_root);
        app_root
    }

    fn load_or_create_data(path: &Path) -> Result<PersistedData> {
        if !path.exists() {
            let initial = PersistedData::default();
            let text = serde_json::to_string_pretty(&initial)?;
            fs::write(path, text)?;
            return Ok(initial);
        }

        let raw = fs::read_to_string(path).with_context(|| format!("读取状态文件失败: {path:?}"))?;
        let parsed = serde_json::from_str::<PersistedData>(&raw)
            .with_context(|| format!("解析状态文件失败: {path:?}"))?;
        Ok(parsed)
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
        if self.login_token.is_empty() && let Some(token) = &self.data.saved_token {
            self.login_token = token.clone();
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

        if self.remember_token {
            self.data.saved_token = Some(token.clone());
        } else {
            self.data.saved_token = None;
        }
        self.save_data();

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
                self.current_user = Some(self.login_token.trim().to_string());
                self.active_login_permission = self.login_permission.trim().to_string();
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
                self.composer_input.clear();
                self.pending_file_path = None;
                self.show_new_session_modal = false;
                self.save_data();
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
                    .fill(egui::Color32::from_rgb(13, 24, 39))
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
                                .fill(egui::Color32::from_rgb(11, 31, 46))
                                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(48, 136, 178)))
                                .inner_margin(egui::Margin::same(14))
                                .corner_radius(12.0)
                                .show(ui, |ui| {
                                    egui::ScrollArea::vertical()
                                        .auto_shrink([false; 2])
                                        .max_height((ui.available_height() - 4.0).max(220.0))
                                        .show(ui, |ui| {
                                            egui::Frame::default()
                                                .fill(egui::Color32::from_rgb(20, 19, 47))
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
                                            let remember_token_label =
                                                self.tr("记住 Token", "Remember Token").to_string();

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
                                                ui.add_sized(
                                                    [token_w, 36.0],
                                                    egui::TextEdit::singleline(&mut self.login_token)
                                                        .hint_text(token_hint.clone()),
                                                );
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
                                                        .fill(egui::Color32::from_rgb(52, 59, 112))
                                                        .stroke(egui::Stroke::new(
                                                            1.0,
                                                            egui::Color32::from_rgb(124, 141, 221),
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
                                                        .fill(egui::Color32::from_rgb(52, 59, 112))
                                                        .stroke(egui::Stroke::new(
                                                            1.0,
                                                            egui::Color32::from_rgb(124, 141, 221),
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
                                                        .fill(egui::Color32::from_rgb(27, 126, 173))
                                                        .stroke(egui::Stroke::new(
                                                            1.0,
                                                            egui::Color32::from_rgb(122, 216, 248),
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
        if self.open_session_busy || self.open_session_rx.is_some() {
            return;
        }
        let Some(handle) = self.client_handle else {
            self.modal_error = self
                .tr("未连接 webrpc", "Not connected to webrpc")
                .to_string();
            return;
        };
        let peer = self.modal_peer_token.trim().to_string();
        if peer.is_empty() {
            self.modal_error = self
                .tr("请输入对端 Token", "Please enter peer Token")
                .to_string();
            return;
        }
        if let Some(current) = self.current_user.as_ref()
            && peer == current.trim()
        {
            self.modal_error = self
                .tr(
                    "目标 Token 不能是当前登录 Token",
                    "Peer Token cannot be the current login Token",
                )
                .to_string();
            return;
        }
        let permission = self.modal_permission.trim().to_string();
        self.modal_error.clear();
        self.open_session_busy = true;
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
                let welcome = ChatMessage {
                    local_id: self.alloc_local_msg_id(),
                    is_me: false,
                    content: format!(
                        "{}: {sid}",
                        self.tr("会话已建立，会话 ID", "Session established, session ID")
                    ),
                    timestamp: now_str(),
                    kind: MessageKind::Text,
                    file_path: None,
                    outbound: None,
                };
                if let Some(i) = self.find_session_index_by_id(sid) {
                    self.chat_sessions[i].peer_token = peer.clone();
                    self.chat_sessions[i].permission = perm;
                    self.chat_sessions[i].messages.push(welcome);
                    self.selected_session = Some(i);
                } else {
                    self.chat_sessions.push(WebrpcChatSession {
                        id: sid,
                        peer_token: peer.clone(),
                        permission: perm,
                        messages: vec![welcome],
                    });
                    self.selected_session = Some(self.chat_sessions.len().saturating_sub(1));
                }
                self.show_new_session_modal = false;
                self.modal_peer_token.clear();
                self.modal_permission.clear();
                self.modal_error.clear();
                self.status = format!(
                    "{} {} → {}",
                    self.tr("已打开会话", "Opened session"),
                    sid,
                    peer
                );
                ctx.request_repaint();
            }
            Ok(Err(e)) => {
                self.open_session_busy = false;
                self.modal_error = e;
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

    fn poll_inbound_events(&mut self, ctx: &egui::Context) {
        let Some(rx) = self.inbound_rx.take() else {
            return;
        };
        let mut got_event = false;
        while let Ok(ev) = rx.try_recv() {
            got_event = true;
            match ev {
                InboundUiEvent::PeerText { session_id, text } => {
                    let local_id = self.alloc_local_msg_id();
                    let i = self.ensure_session_for_inbound(session_id);
                    self.chat_sessions[i].messages.push(ChatMessage {
                        local_id,
                        is_me: false,
                        content: text,
                        timestamp: now_str(),
                        kind: MessageKind::Text,
                        file_path: None,
                        outbound: None,
                    });
                }
                InboundUiEvent::PeerFile {
                    session_id,
                    name,
                    path,
                    size_bytes,
                } => {
                    let p = path.display().to_string();
                    let i = self.ensure_session_for_inbound(session_id);
                    let recv_file_text = self
                        .tr("对端发来文件", "Received file from peer")
                        .to_string();
                    let size_text = format_file_size(size_bytes);
                    let content = format!("{recv_file_text}: {name} ({size_text})");
                    let now = now_str();
                    let existing = self.chat_sessions[i].messages.iter_mut().find(|m| {
                        !m.is_me
                            && matches!(m.kind, MessageKind::File)
                            && m.file_path
                                .as_deref()
                                .and_then(|fp| Path::new(fp).file_name())
                                .and_then(|f| f.to_str())
                                .map(|f| f == name)
                                .unwrap_or(false)
                    });
                    if let Some(msg) = existing {
                        msg.content = content;
                        msg.timestamp = now;
                    } else {
                        let local_id = self.alloc_local_msg_id();
                        self.chat_sessions[i].messages.push(ChatMessage {
                            local_id,
                            is_me: false,
                            content,
                            timestamp: now,
                            kind: MessageKind::File,
                            file_path: Some(p),
                            outbound: None,
                        });
                    }
                }
                InboundUiEvent::SendResult {
                    session_id,
                    local_id,
                    ok,
                    detail,
                } => {
                    if let Some(s) = self.chat_sessions.iter_mut().find(|s| s.id == session_id) {
                        if let Some(m) = s.messages.iter_mut().find(|m| m.local_id == local_id) {
                            m.outbound = Some(if ok {
                                OutboundState::Sent
                            } else {
                                OutboundState::Failed(detail.clone())
                            });
                        }
                    }
                    if ok {
                        self.status = self.tr("发送成功", "Sent successfully").to_string();
                    } else if !ok {
                        self.status =
                            format!("{}: {detail}", self.tr("发送失败", "Send failed"));
                    }
                }
            }
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
        let sid = self.chat_sessions[index].id;
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
            self.chat_sessions[index].messages.push(ChatMessage {
                local_id,
                is_me: true,
                content: format!("{sending_file_text}: {}", path.display()),
                timestamp: now_str(),
                kind: MessageKind::File,
                file_path: Some(path_str.clone()),
                outbound: Some(OutboundState::Sending),
            });
            self.status = self.tr("文件发送中…", "File is being sent...").to_string();
            self.pending_file_path = None;

            let tx = self.inbound_tx.clone();
            let send_ok_text = self.tr("发送成功", "Sent successfully").to_string();
            thread::spawn(move || {
                let result = webrpc_send_file(handle, sid, &path_str);
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
            file_path: None,
            outbound: Some(OutboundState::Sending),
        });
        self.composer_input.clear();
        self.status = self
            .tr("消息发送中…", "Message is being sent...")
            .to_string();

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
        let Some(session) = self.chat_sessions.get(index) else {
            return;
        };
        let sid = session.id;
        let close_result = webrpc_close_session(handle, sid);
        self.chat_sessions.remove(index);
        if self.chat_sessions.is_empty() {
            self.selected_session = None;
        } else {
            self.selected_session = Some(index.min(self.chat_sessions.len().saturating_sub(1)));
        }
        match close_result {
            Ok(()) => {
                self.status = format!("{} {sid}", self.tr("已关闭会话", "Closed session"));
            }
            Err(e) => {
                self.status = if self.ui_lang == UiLanguage::Zh {
                    format!("会话 {sid} 已在本地移除（CloseSession 返回异常: {e}）")
                } else {
                    format!("Session {sid} removed locally (CloseSession returned error: {e})")
                };
            }
        }
    }

    /// 统一释放 webrpc 客户端句柄，避免重复释放。
    fn release_webrpc_client(&mut self) {
        if let Some(handle) = self.client_handle.take() {
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
            self.draw_login_page(ctx);
            return;
        }

        self.poll_open_session_worker(ctx);
        self.poll_inbound_events(ctx);
        self.consume_dropped_files(ctx);
        ctx.request_repaint_after(Duration::from_millis(33));

        if self.show_new_session_modal {
            egui::Window::new(self.tr("新建会话", "New Session"))
                .id(egui::Id::new("new_webrpc_session_modal"))
                .collapsible(false)
                .resizable(true)
                .default_width(420.0)
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

        egui::TopBottomPanel::top("top")
            .frame(
                egui::Frame::default()
                    .fill(egui::Color32::from_rgb(9, 20, 34))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(44, 98, 132)))
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
                        self.release_webrpc_client();
                        self.chat_sessions.clear();
                        self.selected_session = None;
                        self.show_new_session_modal = false;
                        self.current_user = None;
                        self.login_password.clear();
                        self.active_login_permission.clear();
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
            .resizable(true)
            .default_width(280.0)
            .min_width(220.0)
            .frame(
                egui::Frame::default()
                    .fill(egui::Color32::from_rgb(10, 31, 45))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(45, 133, 176)))
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
                            .fill(egui::Color32::from_rgb(20, 84, 120))
                            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(96, 186, 225))),
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
                        let (sid, cached_peer) = {
                            let s = &self.chat_sessions[i];
                            (s.id, s.peer_token.clone())
                        };
                        let peer = self.peer_token_for_session_info(sid, &cached_peer);
                        if peer != cached_peer {
                            self.chat_sessions[i].peer_token = peer.clone();
                        }
                        let display_peer = if peer.trim().is_empty() {
                            self.tr("对端", "Peer").to_string()
                        } else {
                            peer
                        };
                        let is_selected = self.selected_session == Some(i);
                        let label = format!("{}\nSID {}", display_peer, sid);
                        let card_fill = if is_selected {
                            egui::Color32::from_rgb(30, 92, 132)
                        } else {
                            egui::Color32::from_rgb(14, 47, 68)
                        };
                        if ui
                            .add_sized(
                                [ui.available_width(), 56.0],
                                egui::Button::new(
                                    egui::RichText::new(label)
                                        .size(14.0)
                                        .color(egui::Color32::from_rgb(214, 244, 255)),
                                )
                                .fill(card_fill)
                                .stroke(egui::Stroke::new(
                                    1.0,
                                    if is_selected {
                                        egui::Color32::from_rgb(120, 221, 255)
                                    } else {
                                        egui::Color32::from_rgb(52, 124, 162)
                                    },
                                )),
                            )
                            .clicked()
                        {
                            self.selected_session = Some(i);
                        }
                        ui.add_space(4.0);
                    }
                });
            });

        egui::CentralPanel::default()
            .frame(
                egui::Frame::default()
                    .fill(egui::Color32::from_rgb(20, 19, 47))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(95, 92, 191)))
                    .inner_margin(egui::Margin::same(14)),
            )
            .show(ctx, |ui| {
            if let Some(index) = self.selected_session {
                let meta = self.chat_sessions.get(index).map(|s| {
                    (
                        s.id,
                        s.peer_token.clone(),
                        s.permission.clone(),
                        s.messages.clone(),
                    )
                });
                if let Some((sid, fallback_peer, perm, messages)) = meta {
                    let peer_token = self.peer_token_for_session_info(sid, &fallback_peer);
                    let close_idx = index;
                    ui.horizontal(|ui| {
                        ui.heading(
                            egui::RichText::new(format!(
                                "{} {peer_token}",
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
                        });
                    });
                    ui.label(
                        egui::RichText::new(format!(
                            "{}: {} · Permission: {}",
                            self.tr("会话 ID", "Session ID"),
                            sid,
                            if perm.is_empty() {
                                self.tr("（空）", "(empty)").to_string()
                            } else {
                                perm
                            }
                        ))
                        .weak()
                        .small(),
                    );
                    ui.add_space(8.0);
                    let mut open_err: Option<String> = None;
                    // 预留底部发送区高度，避免选择附件后发送区被挤出可视范围。
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
                                        egui::Color32::from_rgb(30, 136, 229),
                                        egui::Color32::from_rgb(252, 254, 255),
                                        egui::Color32::from_rgb(216, 232, 255),
                                        egui::Color32::from_rgb(20, 92, 173),
                                        egui::Color32::from_rgb(182, 255, 207),
                                        egui::Color32::from_rgb(255, 232, 158),
                                        egui::Color32::from_rgb(255, 196, 196),
                                    )
                                } else {
                                    (
                                        self.tr("对方", "Peer"),
                                        egui::Color32::from_rgb(236, 240, 247),
                                        egui::Color32::from_rgb(24, 30, 44),
                                        egui::Color32::from_rgb(98, 108, 125),
                                        egui::Color32::from_rgb(94, 113, 142),
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
                                    let bubble_max = (ui.available_width() * 0.72).clamp(220.0, 560.0);
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
                                                                        self.tr("发送失败", "Send failed")
                                                                    ))
                                                                    .small()
                                                                    .color(status_err_color),
                                                                );
                                                            }
                                                        }
                                                    }
                                                }
                                            });
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
                                                    ui.add(
                                                        egui::Label::new(
                                                            egui::RichText::new(&msg.content)
                                                                .color(text_color)
                                                                .size(16.5),
                                                        )
                                                        .wrap_mode(egui::TextWrapMode::Wrap),
                                                    );
                                                    if let Some(path) = &msg.file_path
                                                        && ui
                                                            .add(
                                                                egui::Button::new(
                                                                    egui::RichText::new(self.tr(
                                                                        "打开目录",
                                                                        "Open Folder",
                                                                    ))
                                                                        .color(egui::Color32::from_rgb(
                                                                            224, 246, 255,
                                                                        )),
                                                                )
                                                                .fill(egui::Color32::from_rgb(
                                                                    26, 98, 136,
                                                                ))
                                                                .stroke(egui::Stroke::new(
                                                                    1.0,
                                                                    egui::Color32::from_rgb(
                                                                        104, 199, 236,
                                                                    ),
                                                                )),
                                                            )
                                                            .clicked()
                                                    {
                                                        let target = Path::new(path)
                                                            .parent()
                                                            .map(|p| p.to_path_buf())
                                                            .unwrap_or_else(|| PathBuf::from(path));
                                                        if let Err(err) = opener::open(target) {
                                                            open_err =
                                                                Some(format!(
                                                                    "{}: {err}",
                                                                    self.tr("打开失败", "Open failed")
                                                                ));
                                                        }
                                                    }
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
                    ui.group(|ui| {
                        ui.label(
                            egui::RichText::new(self.tr("发送区（文本/文件）", "Send Area (Text/File)"))
                                .strong(),
                        );
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            let sending_file = self.pending_file_path.is_some();
                            let send_label = if sending_file {
                                self.tr("发送文件", "Send File")
                            } else {
                                self.tr("发送消息", "Send Message")
                            };
                            let composer_hint = if sending_file {
                                self.tr(
                                    "已选择附件。Enter发送，Ctrl+Enter换行",
                                    "Attachment selected. Enter to send, Ctrl+Enter for newline",
                                )
                                .to_string()
                            } else {
                                self.tr(
                                    "输入文本（支持多行）。Enter发送，Ctrl+Enter换行",
                                    "Type message (multi-line supported). Enter to send, Ctrl+Enter for newline",
                                )
                                .to_string()
                            };
                            let w = (ui.available_width() - 210.0).max(120.0);
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
                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new(self.tr("选择文件", "Choose File"))
                                            .color(egui::Color32::from_rgb(221, 244, 255)),
                                    )
                                    .fill(egui::Color32::from_rgb(29, 95, 133))
                                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(111, 198, 230))),
                                )
                                .clicked()
                                && let Some(path) = rfd::FileDialog::new().pick_file()
                            {
                                self.pending_file_path = Some(path.display().to_string());
                            }
                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new(send_label)
                                            .color(egui::Color32::from_rgb(235, 249, 255)),
                                    )
                                    .fill(egui::Color32::from_rgb(28, 126, 173))
                                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(123, 216, 248))),
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
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui
                                            .add(
                                                egui::Button::new(
                                                    egui::RichText::new(
                                                        self.tr(
                                                            "移除附件",
                                                            "Remove attachment",
                                                        ),
                                                    )
                                                    .color(egui::Color32::from_rgb(
                                                        255, 235, 242,
                                                    )),
                                                )
                                                .fill(egui::Color32::from_rgb(119, 42, 76))
                                                .stroke(egui::Stroke::new(
                                                    1.0,
                                                    egui::Color32::from_rgb(215, 115, 156),
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
                                    },
                                );
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
fn webrpc_callback_tcp_loop(_handle: usize, port: u16, inbound_tx: mpsc::Sender<InboundUiEvent>) {
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
            1 => handle_callback_file_stream(&mut conn, session_id, &inbound_tx),
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
    let mut file_data = vec![0u8; file_len];
    if read_full(conn, &mut file_data).is_err() {
        return;
    }
    let name_raw = String::from_utf8_lossy(&name_bytes);
    eprintln!("webrpc: 文件流 session={session_id} 文件名={name_raw} 大小={file_len}");
    let base = File2FileApp::ensure_app_root().join("received_files");
    let _ = fs::create_dir_all(&base);
    let safe: String = name_raw
        .chars()
        .filter(|c| *c != '/' && *c != '\\' && *c != ':' && *c != '\0')
        .collect();
    if safe.is_empty() {
        return;
    }
    let path = base.join(&safe);
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
