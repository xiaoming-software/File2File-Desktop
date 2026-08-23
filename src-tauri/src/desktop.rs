use std::collections::VecDeque;
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[cfg(not(target_os = "macos"))]
use enigo::{Axis, Button, Coordinate, Direction, Enigo, Mouse, Settings};
#[cfg(target_os = "macos")]
use enigo::{Button, Direction};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use image::codecs::jpeg::JpegEncoder;
use image::{ColorType, DynamicImage, RgbaImage};
use openh264::decoder::Decoder;
use openh264::encoder::{
    BitRate, Complexity, Encoder, EncoderConfig, FrameRate, IntraFramePeriod, UsageType,
};
use openh264::formats::{RgbSliceU8, YUVBuffer, YUVSource};
use serde::{Deserialize, Serialize};
use tauri::Emitter;

use crate::webrpc;

#[cfg(target_os = "macos")]
#[path = "desktop_mac.rs"]
mod desktop_mac;

#[cfg(target_os = "macos")]
const HARD_MAX_EDGE: u32 = 1280;
#[cfg(not(target_os = "macos"))]
const HARD_MAX_EDGE: u32 = 1920;
#[cfg(target_os = "macos")]
const TARGET_FPS: u32 = 15;
#[cfg(not(target_os = "macos"))]
const TARGET_FPS: u32 = 15;
const FRAME_INTERVAL: Duration = Duration::from_millis(1000 / TARGET_FPS as u64);
const TARGET_BITRATE_BPS: u32 = 4_800_000;
const JPEG_QUALITY: u8 = 80;
const KEY_EVERY: u32 = 75;
const SEND_TIMEOUT_MS: i64 = 0;
const KEY_SEND_TIMEOUT_MS: i64 = 40;
const CLICK_SEND_TIMEOUT_MS: i64 = 40;
const VIDEO_BIN_VER: u8 = 1;
const DESKTOP_SIGNAL_TYPE: u8 = 8;
const DESKTOP_INPUT_TYPE: u8 = 9;
const RING_TIMEOUT_MS: u64 = 45_000;
const MAC_CONTROLLED_UNSUPPORTED: &str = "暂时不支持控制 Mac 桌面。";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DesktopPhase {
    Idle,
    Outgoing,
    Incoming,
    Controlling,
    Controlled,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopUiState {
    pub phase: DesktopPhase,
    pub session_id: u32,
    pub share_id: String,
    pub peer_token: String,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub kbps: u32,
    pub error: String,
}

impl DesktopUiState {
    fn idle() -> Self {
        Self::idle_with(String::new())
    }

    fn idle_with(error: String) -> Self {
        Self {
            phase: DesktopPhase::Idle,
            session_id: 0,
            share_id: String::new(),
            peer_token: String::new(),
            width: 0,
            height: 0,
            fps: 0,
            kbps: 0,
            error,
        }
    }
}

struct Session {
    phase: DesktopPhase,
    session_id: u32,
    share_id: String,
    peer_token: String,
    width: u32,
    height: u32,
}

struct LiveShare {
    stop: Arc<AtomicBool>,
    _worker: thread::JoinHandle<()>,
}

struct DesktopInner {
    session: Option<Session>,
    live: Option<LiveShare>,
    fps: u32,
    kbps: u32,
    error: String,
}

fn inner() -> &'static Mutex<DesktopInner> {
    static CELL: OnceLock<Mutex<DesktopInner>> = OnceLock::new();
    CELL.get_or_init(|| {
        Mutex::new(DesktopInner {
            session: None,
            live: None,
            fps: 0,
            kbps: 0,
            error: String::new(),
        })
    })
}

fn lock_inner() -> std::sync::MutexGuard<'static, DesktopInner> {
    inner().lock().unwrap_or_else(|err| err.into_inner())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|item| item.as_millis() as u64)
        .unwrap_or(0)
}

fn emit_state(state: DesktopUiState) {
    if let Some(app) = webrpc::app_handle() {
        let _ = app.emit("webrpc-desktop-state", state);
    }
}

fn snapshot(guard: &DesktopInner) -> DesktopUiState {
    let Some(session) = guard.session.as_ref() else {
        return DesktopUiState::idle();
    };
    DesktopUiState {
        phase: session.phase,
        session_id: session.session_id,
        share_id: session.share_id.clone(),
        peer_token: session.peer_token.clone(),
        width: session.width,
        height: session.height,
        fps: guard.fps,
        kbps: guard.kbps,
        error: guard.error.clone(),
    }
}

fn stop_live(live: &Option<LiveShare>) {
    if let Some(item) = live {
        item.stop.store(true, Ordering::SeqCst);
    }
}

fn clear_to_idle(guard: &mut DesktopInner, emit: bool) {
    stop_live(&guard.live);
    guard.live = None;
    guard.session = None;
    guard.fps = 0;
    guard.kbps = 0;
    clear_screen_map();
    if let Ok(mut box_) = mouse_outbox().lock() {
        box_.clicks.clear();
        box_.latest_move = None;
    }
    try_reset_injected_mouse();
    if emit {
        emit_state(DesktopUiState::idle_with(std::mem::take(&mut guard.error)));
    }
}

#[derive(Serialize)]
struct SignalMessage<'a> {
    #[serde(rename = "type")]
    kind: u8,
    data: SignalData<'a>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SignalData<'a> {
    op: &'a str,
    share_id: &'a str,
    width: u32,
    height: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InboundSignal {
    op: String,
    #[serde(default)]
    share_id: String,
    #[serde(default)]
    width: u32,
    #[serde(default)]
    height: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InboundFrame {
    #[serde(default)]
    share_id: String,
    #[serde(default)]
    width: u32,
    #[serde(default)]
    height: u32,
    #[serde(default)]
    video: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopFrameEvent {
    session_id: u32,
    width: u32,
    height: u32,
    jpeg: String,
}

fn send_signal(session_id: u32, op: &str, share_id: &str, width: u32, height: u32) {
    let Ok(payload) = serde_json::to_string(&SignalMessage {
        kind: DESKTOP_SIGNAL_TYPE,
        data: SignalData {
            op,
            share_id,
            width,
            height,
        },
    }) else {
        return;
    };
    let _ = webrpc::send_json_timeout(session_id, &payload, 10_000);
}

fn new_share_id(session_id: u32) -> String {
    format!("{session_id}-{}", now_ms())
}

fn pick_monitor() -> Result<xcap::Monitor, String> {
    let monitors = xcap::Monitor::all().map_err(|err| format!("读取显示器失败: {err}"))?;
    if monitors.is_empty() {
        return Err("没有可用的显示器".into());
    }
    let primary = monitors.iter().find(|item| item.is_primary().unwrap_or(false));
    Ok(primary.cloned().unwrap_or_else(|| monitors[0].clone()))
}

fn even_scale(img: RgbaImage) -> RgbaImage {
    let mut w = img.width();
    let mut h = img.height();
    if w == 0 || h == 0 {
        return img;
    }
    let edge = w.max(h);
    if edge > HARD_MAX_EDGE {
        let scale = HARD_MAX_EDGE as f32 / edge as f32;
        w = ((w as f32 * scale).round() as u32).max(2);
        h = ((h as f32 * scale).round() as u32).max(2);
    }
    w &= !1;
    h &= !1;
    if w < 2 {
        w = 2;
    }
    if h < 2 {
        h = 2;
    }
    if w == img.width() && h == img.height() {
        img
    } else {
        image::imageops::resize(&img, w, h, image::imageops::FilterType::Triangle)
    }
}

fn target_bitrate(_width: u32, _height: u32) -> u32 {
    TARGET_BITRATE_BPS
}

fn encode_h264(encoder: &mut Encoder, img: &RgbaImage) -> Result<(Vec<u8>, bool), String> {
    let rgb = DynamicImage::ImageRgba8(img.clone()).to_rgb8();
    let src = RgbSliceU8::new(
        rgb.as_raw(),
        (rgb.width() as usize, rgb.height() as usize),
    );
    let yuv = YUVBuffer::from_rgb_source(src);
    let bitstream = encoder
        .encode(&yuv)
        .map_err(|err| format!("H.264 编码失败: {err}"))?;
    let bytes = bitstream.to_vec();
    let key = bytes.windows(5).any(|w| w[0] == 0 && w[1] == 0 && is_idr_start(w));
    Ok((bytes, key))
}

fn is_idr_start(bytes: &[u8]) -> bool {
    if bytes.len() < 5 {
        return false;
    }
    if bytes[0] == 0 && bytes[1] == 0 && bytes[2] == 1 {
        return bytes[3] & 0x1F == 5;
    }
    if bytes[0] == 0 && bytes[1] == 0 && bytes[2] == 0 && bytes[3] == 1 {
        return bytes[4] & 0x1F == 5;
    }
    false
}

fn rgba_from_recorder(frame: xcap::Frame) -> Option<RgbaImage> {
    let expected = (frame.width as usize).saturating_mul(frame.height as usize).saturating_mul(4);
    if frame.width == 0 || frame.height == 0 || frame.raw.len() != expected {
        eprintln!(
            "desktop: skip recorder frame {}x{} bytes={}",
            frame.width,
            frame.height,
            frame.raw.len()
        );
        return None;
    }
    RgbaImage::from_raw(frame.width, frame.height, frame.raw)
}

fn build_encoder(width: u32, height: u32) -> Result<Encoder, String> {
    let config = EncoderConfig::new()
        .max_frame_rate(FrameRate::from_hz(TARGET_FPS as f32))
        .bitrate(BitRate::from_bps(target_bitrate(width, height)))
        .usage_type(UsageType::ScreenContentRealTime)
        .complexity(Complexity::Low)
        .skip_frames(false)
        .intra_frame_period(IntraFramePeriod::from_num_frames(KEY_EVERY));
    Encoder::with_api_config(openh264::OpenH264API::from_source(), config)
        .map_err(|err| format!("初始化 H.264 编码器失败: {err}"))
}

fn encoder_for<'a>(
    slot: &'a mut Option<Encoder>,
    size: &mut (u32, u32),
    width: u32,
    height: u32,
) -> Result<&'a mut Encoder, String> {
    if slot.is_none() || *size != (width, height) {
        *slot = Some(build_encoder(width, height)?);
        *size = (width, height);
    }
    slot.as_mut().ok_or_else(|| "编码器未就绪".to_string())
}

#[cfg(not(target_os = "macos"))]
fn capture_frame(monitor: &xcap::Monitor) -> Result<RgbaImage, String> {
    monitor
        .capture_image()
        .map_err(|err| format!("采集屏幕失败: {err}"))
}

#[cfg_attr(target_os = "macos", allow(dead_code))]
fn start_capture_loop(session_id: u32, share_id: String, stop: Arc<AtomicBool>) -> Result<(), String> {
    thread::Builder::new()
        .name("desktop-share".into())
        .spawn(move || {
            let result = (|| {
                let monitor = pick_monitor()?;
                run_best_capture(session_id, &share_id, stop, monitor)
            })();
            if let Err(err) = result {
                eprintln!("desktop: capture stopped: {err}");
                {
                    let mut guard = lock_inner();
                    if guard
                        .session
                        .as_ref()
                        .map(|item| item.share_id == share_id && item.phase == DesktopPhase::Controlled)
                        .unwrap_or(false)
                    {
                        guard.error = err;
                        emit_state(snapshot(&guard));
                    }
                }
                send_signal(session_id, "stop", &share_id, 0, 0);
            }
        })
        .map_err(|err| format!("画面采集线程失败: {err}"))?;
    Ok(())
}

fn run_best_capture(
    session_id: u32,
    share_id: &str,
    stop: Arc<AtomicBool>,
    monitor: xcap::Monitor,
) -> Result<(), String> {
    // macOS 正式采集走 ScreenCaptureKit，才能拿到其它 App 窗口。
    #[cfg(target_os = "macos")]
    {
        eprintln!("desktop: macos capture via ScreenCaptureKit");
        return run_sck_loop(session_id, share_id, stop, monitor);
    }

    #[cfg(not(target_os = "macos"))]
    {
        if let Ok((rec, rx)) = monitor.video_recorder() {
            if rec.start().is_ok() {
                if let Some(first) = wait_recorder_frame(&rx, Duration::from_millis(1200)) {
                    eprintln!("desktop: capture via video recorder");
                    return run_recorder_loop(
                        session_id,
                        share_id,
                        stop,
                        monitor,
                        rec,
                        rx,
                        Some(first),
                    );
                }
                eprintln!("desktop: recorder produced no frame, fallback to screenshot");
                let _ = rec.stop();
            }
        } else {
            eprintln!("desktop: video recorder unavailable, fallback to screenshot");
        }
        run_screenshot_loop(session_id, share_id, stop, monitor)
    }
}

#[cfg_attr(target_os = "macos", allow(dead_code))]
fn wait_recorder_frame(
    rx: &std::sync::mpsc::Receiver<xcap::Frame>,
    timeout: Duration,
) -> Option<xcap::Frame> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left.min(Duration::from_millis(200))) {
            Ok(frame) => {
                if (frame.width as usize)
                    .saturating_mul(frame.height as usize)
                    .saturating_mul(4)
                    == frame.raw.len()
                    && frame.width > 0
                    && frame.height > 0
                {
                    return Some(frame);
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return None,
        }
    }
    None
}

#[cfg_attr(target_os = "macos", allow(dead_code))]
fn run_recorder_loop(
    session_id: u32,
    share_id: &str,
    stop: Arc<AtomicBool>,
    monitor: xcap::Monitor,
    recorder: xcap::VideoRecorder,
    rx: std::sync::mpsc::Receiver<xcap::Frame>,
    first: Option<xcap::Frame>,
) -> Result<(), String> {
    remember_screen(&monitor);
    let mut encoder = None;
    let mut enc_size = (0u32, 0u32);
    let mut last = Instant::now() - FRAME_INTERVAL;
    let mut seq = 0u32;
    let mut sent_frames = 0u32;
    let mut sent_bytes = 0u64;
    let mut stats_at = Instant::now();
    let mut pending = first;
    send_signal(session_id, "start", share_id, 0, 0);
    while !stop.load(Ordering::SeqCst) {
        let frame = if let Some(frame) = pending.take() {
            frame
        } else {
            match rx.recv_timeout(Duration::from_millis(200)) {
                Ok(frame) => frame,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
        };
        if last.elapsed() < FRAME_INTERVAL {
            continue;
        }
        last = Instant::now();
        let Some(img) = rgba_from_recorder(frame) else {
            continue;
        };
        let img = even_scale(img);
        let encoder = match encoder_for(&mut encoder, &mut enc_size, img.width(), img.height()) {
            Ok(item) => item,
            Err(err) => {
                eprintln!("desktop: encoder {err}");
                continue;
            }
        };
        if seq % KEY_EVERY == 0 || KEY_REQUEST.swap(false, Ordering::SeqCst) {
            encoder.force_intra_frame();
        }
        let (bytes, key) = match encode_h264(encoder, &img) {
            Ok(item) => item,
            Err(err) => {
                eprintln!("desktop: encode {err}");
                continue;
            }
        };
        if bytes.is_empty() {
            continue;
        }
        {
            let mut guard = lock_inner();
            if let Some(session) = guard.session.as_mut() {
                if session.share_id == share_id {
                    session.width = img.width();
                    session.height = img.height();
                }
            }
        }
        if !send_video_frame(session_id, share_id, seq, img.width(), img.height(), key, &bytes) {
            if stop.load(Ordering::SeqCst) {
                break;
            }
        }
        seq = seq.wrapping_add(1);
        sent_frames += 1;
        sent_bytes += bytes.len() as u64;
        maybe_emit_stats(&stop, share_id, &mut stats_at, &mut sent_frames, &mut sent_bytes);
    }
    let _ = recorder.stop();
    Ok(())
}

#[cfg(target_os = "macos")]
fn run_sck_loop(
    session_id: u32,
    share_id: &str,
    stop: Arc<AtomicBool>,
    monitor: xcap::Monitor,
) -> Result<(), String> {
    remember_screen(&monitor);
    let mut capturer = desktop_mac::MacCapturer::start(HARD_MAX_EDGE, TARGET_FPS)?;
    let first = capturer.wait_first(Duration::from_secs(12))?;
    let mut encoder = None;
    let mut enc_size = (0u32, 0u32);
    let mut last = Instant::now() - FRAME_INTERVAL;
    let mut seq = 0u32;
    let mut sent_frames = 0u32;
    let mut sent_bytes = 0u64;
    let mut stats_at = Instant::now();
    let mut pending = Some(first);
    send_signal(session_id, "start", share_id, 0, 0);
    while !stop.load(Ordering::SeqCst) {
        let raw = if let Some(frame) = pending.take() {
            frame
        } else {
            match capturer.take() {
                Some(frame) => frame,
                None => {
                    thread::sleep(Duration::from_millis(10));
                    continue;
                }
            }
        };
        if last.elapsed() < FRAME_INTERVAL {
            continue;
        }
        last = Instant::now();
        let img = even_scale(raw);
        let encoder = match encoder_for(&mut encoder, &mut enc_size, img.width(), img.height()) {
            Ok(item) => item,
            Err(err) => {
                eprintln!("desktop: encoder {err}");
                continue;
            }
        };
        if seq % KEY_EVERY == 0 || KEY_REQUEST.swap(false, Ordering::SeqCst) {
            encoder.force_intra_frame();
        }
        let (bytes, key) = match encode_h264(encoder, &img) {
            Ok(item) => item,
            Err(err) => {
                eprintln!("desktop: encode {err}");
                continue;
            }
        };
        if bytes.is_empty() {
            continue;
        }
        {
            let mut guard = lock_inner();
            if let Some(session) = guard.session.as_mut() {
                if session.share_id == share_id {
                    session.width = img.width();
                    session.height = img.height();
                }
            }
        }
        if !send_video_frame(session_id, share_id, seq, img.width(), img.height(), key, &bytes) {
            if stop.load(Ordering::SeqCst) {
                break;
            }
        }
        seq = seq.wrapping_add(1);
        sent_frames += 1;
        sent_bytes += bytes.len() as u64;
        maybe_emit_stats(&stop, share_id, &mut stats_at, &mut sent_frames, &mut sent_bytes);
    }
    capturer.stop();
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn run_screenshot_loop(
    session_id: u32,
    share_id: &str,
    stop: Arc<AtomicBool>,
    monitor: xcap::Monitor,
) -> Result<(), String> {
    remember_screen(&monitor);
    let mut encoder = None;
    let mut enc_size = (0u32, 0u32);
    let mut seq = 0u32;
    let mut sent_frames = 0u32;
    let mut sent_bytes = 0u64;
    let mut stats_at = Instant::now();
    send_signal(session_id, "start", share_id, 0, 0);
    while !stop.load(Ordering::SeqCst) {
        let started = Instant::now();
        let raw = capture_frame(&monitor)?;
        let img = even_scale(raw);
        let encoder = match encoder_for(&mut encoder, &mut enc_size, img.width(), img.height()) {
            Ok(item) => item,
            Err(err) => {
                eprintln!("desktop: encoder {err}");
                if let Some(rest) = FRAME_INTERVAL.checked_sub(started.elapsed()) {
                    thread::sleep(rest);
                }
                continue;
            }
        };
        if seq % KEY_EVERY == 0 || KEY_REQUEST.swap(false, Ordering::SeqCst) {
            encoder.force_intra_frame();
        }
        let (bytes, key) = match encode_h264(encoder, &img) {
            Ok(item) => item,
            Err(err) => {
                eprintln!("desktop: encode {err}");
                if let Some(rest) = FRAME_INTERVAL.checked_sub(started.elapsed()) {
                    thread::sleep(rest);
                }
                continue;
            }
        };
        if !bytes.is_empty() {
            {
                let mut guard = lock_inner();
                if let Some(session) = guard.session.as_mut() {
                    if session.share_id == share_id {
                        session.width = img.width();
                        session.height = img.height();
                    }
                }
            }
            send_video_frame(session_id, share_id, seq, img.width(), img.height(), key, &bytes);
            seq = seq.wrapping_add(1);
            sent_frames += 1;
            sent_bytes += bytes.len() as u64;
            maybe_emit_stats(&stop, share_id, &mut stats_at, &mut sent_frames, &mut sent_bytes);
        }
        if let Some(rest) = FRAME_INTERVAL.checked_sub(started.elapsed()) {
            thread::sleep(rest);
        }
    }
    Ok(())
}

fn maybe_emit_stats(
    stop: &AtomicBool,
    share_id: &str,
    stats_at: &mut Instant,
    frames: &mut u32,
    bytes: &mut u64,
) {
    if stats_at.elapsed() < Duration::from_millis(800) {
        return;
    }
    let secs = stats_at.elapsed().as_secs_f32().max(0.2);
    let fps = (*frames as f32 / secs).round() as u32;
    let kbps = ((*bytes as f32 * 8.0) / secs / 1000.0).round() as u32;
    *frames = 0;
    *bytes = 0;
    *stats_at = Instant::now();
    if stop.load(Ordering::SeqCst) {
        return;
    }
    let mut guard = lock_inner();
    if guard
        .session
        .as_ref()
        .map(|item| item.share_id == share_id)
        .unwrap_or(false)
    {
        guard.fps = fps;
        guard.kbps = kbps;
        emit_state(snapshot(&guard));
    }
}

fn send_video_frame(
    session_id: u32,
    share_id: &str,
    seq: u32,
    width: u32,
    height: u32,
    key: bool,
    bytes: &[u8],
) -> bool {
    let share = share_id.as_bytes();
    if share.len() > 255 {
        return false;
    }
    let mut payload = Vec::with_capacity(12 + share.len() + bytes.len());
    payload.extend_from_slice(&[0, 0, 0, VIDEO_BIN_VER, if key { 1 } else { 0 }, share.len() as u8]);
    payload.extend_from_slice(share);
    payload.extend_from_slice(&(width.min(u16::MAX as u32) as u16).to_le_bytes());
    payload.extend_from_slice(&(height.min(u16::MAX as u32) as u16).to_le_bytes());
    payload.extend_from_slice(&seq.to_le_bytes());
    payload.extend_from_slice(bytes);
    flush_mouse_sends();
    let timeout = if key { KEY_SEND_TIMEOUT_MS } else { SEND_TIMEOUT_MS };
    let ok = webrpc::send_bytes_timeout(session_id, &payload, timeout);
    if !ok && key {
        KEY_REQUEST.store(true, Ordering::SeqCst);
    }
    ok
}

fn jpeg_from_rgb(width: u32, height: u32, rgb: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut encoder = JpegEncoder::new_with_quality(Cursor::new(&mut out), JPEG_QUALITY);
    encoder
        .encode(rgb, width, height, ColorType::Rgb8.into())
        .map_err(|err| format!("画面转码失败: {err}"))?;
    Ok(out)
}

#[tauri::command]
pub fn desktop_invite(session_id: u32) -> Result<(), String> {
    if session_id == 0 {
        return Err("请先连接会话再发起远程控制。".into());
    }
    if webrpc::rpc_handle() == 0 {
        return Err("尚未登录。".into());
    }
    let peer = webrpc::peer_token_of(session_id);
    let share_id = new_share_id(session_id);
    {
        let mut guard = lock_inner();
        if guard.session.is_some() {
            return Err("当前已有远程控制，请先结束后再试。".into());
        }
        guard.error.clear();
        guard.fps = 0;
        guard.kbps = 0;
        guard.session = Some(Session {
            phase: DesktopPhase::Outgoing,
            session_id,
            share_id: share_id.clone(),
            peer_token: peer,
            width: 0,
            height: 0,
        });
        emit_state(snapshot(&guard));
    }
    send_signal(session_id, "invite", &share_id, 0, 0);
    spawn_ring_timeout(session_id, share_id, DesktopPhase::Outgoing);
    Ok(())
}

#[tauri::command]
pub fn desktop_share_start(session_id: u32) -> Result<(), String> {
    desktop_invite(session_id)
}

#[tauri::command]
pub fn desktop_accept() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        Err(MAC_CONTROLLED_UNSUPPORTED.into())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let (session_id, share_id) = {
            let mut guard = lock_inner();
            let session = guard
                .session
                .as_mut()
                .ok_or_else(|| "没有待处理的远程控制请求。".to_string())?;
            if session.phase != DesktopPhase::Incoming {
                return Err("当前没有待处理的远程控制请求。".into());
            }
            session.phase = DesktopPhase::Controlled;
            (session.session_id, session.share_id.clone())
        };
        send_signal(session_id, "accept", &share_id, 0, 0);
        if let Err(err) = begin_controlled(session_id, share_id.clone()) {
            let mut guard = lock_inner();
            clear_to_idle(&mut guard, true);
            send_signal(session_id, "hangup", &share_id, 0, 0);
            return Err(err);
        }
        emit_state(snapshot(&lock_inner()));
        Ok(())
    }
}

#[tauri::command]
pub fn desktop_reject() -> Result<(), String> {
    hangup_with_op(None)
}

#[tauri::command]
pub fn desktop_hangup() -> Result<(), String> {
    hangup_with_op(None)
}

#[tauri::command]
pub fn desktop_share_stop() -> Result<(), String> {
    desktop_hangup()
}

fn hangup_with_op(forced: Option<&str>) -> Result<(), String> {
    let (session_id, share_id, op) = {
        let mut guard = lock_inner();
        let Some(session) = guard.session.take() else {
            emit_state(DesktopUiState::idle());
            return Ok(());
        };
        stop_live(&guard.live);
        guard.live = None;
        guard.fps = 0;
        guard.kbps = 0;
        emit_state(DesktopUiState::idle());
        let op = forced.unwrap_or(match session.phase {
            DesktopPhase::Incoming => "reject",
            DesktopPhase::Outgoing => "cancel",
            _ => "hangup",
        });
        (session.session_id, session.share_id, op.to_string())
    };
    send_signal(session_id, &op, &share_id, 0, 0);
    Ok(())
}

fn begin_controlled(session_id: u32, share_id: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let _ = (session_id, share_id);
        Err(MAC_CONTROLLED_UNSUPPORTED.into())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let stop = Arc::new(AtomicBool::new(false));
        start_capture_loop(session_id, share_id, stop.clone())?;
        let mut guard = lock_inner();
        guard.live = Some(LiveShare {
            stop,
            _worker: thread::spawn(|| {}),
        });
        Ok(())
    }
}

fn spawn_ring_timeout(session_id: u32, share_id: String, wait_phase: DesktopPhase) {
    thread::Builder::new()
        .name("desktop-ring".into())
        .spawn(move || {
            thread::sleep(Duration::from_millis(RING_TIMEOUT_MS));
            let send = {
                let mut guard = lock_inner();
                let Some(session) = guard.session.as_ref() else {
                    return;
                };
                if session.session_id != session_id
                    || session.share_id != share_id
                    || session.phase != wait_phase
                {
                    return;
                }
                let op = if wait_phase == DesktopPhase::Outgoing {
                    "timeout"
                } else {
                    "reject"
                };
                let sid = session.session_id;
                let id = session.share_id.clone();
                clear_to_idle(&mut guard, true);
                (sid, id, op.to_string())
            };
            send_signal(send.0, &send.2, &send.1, 0, 0);
        })
        .ok();
}

#[tauri::command]
pub fn desktop_state() -> DesktopUiState {
    snapshot(&lock_inner())
}

pub fn shutdown() {
    let mut guard = lock_inner();
    clear_to_idle(&mut guard, true);
}

pub fn on_session_dead(session_id: u32) {
    let should_clear = {
        let guard = lock_inner();
        guard
            .session
            .as_ref()
            .map(|item| item.session_id == session_id)
            .unwrap_or(false)
    };
    if should_clear {
        shutdown();
    }
}

pub fn on_signal(session_id: u32, data: serde_json::Value) {
    let parsed: InboundSignal = match serde_json::from_value(data) {
        Ok(v) => v,
        Err(_) => return,
    };
    match parsed.op.trim() {
        "invite" => on_invite(session_id, &parsed),
        "accept" => on_accept(session_id, &parsed.share_id),
        "start" => on_peer_start(session_id, &parsed),
        "unsupported" => on_peer_unsupported(session_id, &parsed.share_id),
        "stop" | "reject" | "cancel" | "hangup" | "busy" | "timeout" => {
            on_peer_stop(session_id, &parsed.share_id)
        }
        "key" => on_peer_key(session_id, &parsed.share_id),
        _ => {}
    }
}

fn on_invite(session_id: u32, parsed: &InboundSignal) {
    if parsed.share_id.is_empty() {
        return;
    }
    #[cfg(target_os = "macos")]
    {
        send_signal(session_id, "unsupported", &parsed.share_id, 0, 0);
    }
    #[cfg(not(target_os = "macos"))]
    {
        let peer = webrpc::peer_token_of(session_id);
        let mut guard = lock_inner();
        if let Some(session) = guard.session.as_ref() {
            if session.session_id == session_id && session.share_id == parsed.share_id {
                return;
            }
            drop(guard);
            send_signal(session_id, "busy", &parsed.share_id, 0, 0);
            return;
        }
        guard.session = Some(Session {
            phase: DesktopPhase::Incoming,
            session_id,
            share_id: parsed.share_id.clone(),
            peer_token: peer,
            width: 0,
            height: 0,
        });
        emit_state(snapshot(&guard));
        drop(guard);
        spawn_ring_timeout(session_id, parsed.share_id.clone(), DesktopPhase::Incoming);
    }
}

fn on_accept(session_id: u32, share_id: &str) {
    {
        let mut guard = lock_inner();
        let Some(session) = guard.session.as_mut() else {
            return;
        };
        if session.phase != DesktopPhase::Outgoing
            || session.session_id != session_id
            || (!share_id.is_empty() && session.share_id != share_id)
        {
            return;
        }
        session.phase = DesktopPhase::Controlling;
        reset_decoder();
        recv_stats().reset();
        emit_state(snapshot(&guard));
    }
    send_signal(session_id, "key", share_id, 0, 0);
}

fn on_peer_start(session_id: u32, parsed: &InboundSignal) {
    let mut guard = lock_inner();
    let Some(session) = guard.session.as_mut() else {
        return;
    };
    if session.session_id != session_id {
        return;
    }
    if !parsed.share_id.is_empty() && session.share_id != parsed.share_id {
        return;
    }
    if session.phase != DesktopPhase::Controlling && session.phase != DesktopPhase::Outgoing {
        return;
    }
    session.phase = DesktopPhase::Controlling;
    if parsed.width > 0 {
        session.width = parsed.width;
    }
    if parsed.height > 0 {
        session.height = parsed.height;
    }
    emit_state(snapshot(&guard));
}

fn on_peer_unsupported(session_id: u32, share_id: &str) {
    let mut guard = lock_inner();
    let Some(session) = guard.session.as_ref() else {
        return;
    };
    if session.session_id != session_id {
        return;
    }
    if !share_id.is_empty() && session.share_id != share_id {
        return;
    }
    guard.error = MAC_CONTROLLED_UNSUPPORTED.to_string();
    clear_to_idle(&mut guard, true);
}

fn on_peer_stop(session_id: u32, share_id: &str) {
    let mut guard = lock_inner();
    let Some(session) = guard.session.as_ref() else {
        return;
    };
    if session.session_id != session_id {
        return;
    }
    if !share_id.is_empty() && session.share_id != share_id {
        return;
    }
    clear_to_idle(&mut guard, true);
}

fn on_peer_key(session_id: u32, share_id: &str) {
    let guard = lock_inner();
    let Some(session) = guard.session.as_ref() else {
        return;
    };
    if session.phase != DesktopPhase::Controlled
        || session.session_id != session_id
        || (!share_id.is_empty() && session.share_id != share_id)
    {
        return;
    }
    KEY_REQUEST.store(true, Ordering::SeqCst);
}

static KEY_REQUEST: AtomicBool = AtomicBool::new(false);

pub fn on_video_binary(session_id: u32, payload: &[u8]) {
    let Some((share_id, width, height, _key, h264)) = parse_video_bin(payload) else {
        return;
    };
    present_h264(session_id, &share_id, width, height, h264);
}

pub fn on_video_frame(session_id: u32, data: serde_json::Value) {
    let parsed: InboundFrame = match serde_json::from_value(data) {
        Ok(v) => v,
        Err(_) => return,
    };
    if parsed.video.is_empty() {
        return;
    }
    let Ok(bytes) = B64.decode(parsed.video.trim()) else {
        return;
    };
    present_h264(session_id, &parsed.share_id, parsed.width, parsed.height, &bytes);
}

fn parse_video_bin(payload: &[u8]) -> Option<(String, u32, u32, bool, &[u8])> {
    if payload.len() < 12 || payload[0] != 0 || payload[1] != 0 || payload[2] != 0 {
        return None;
    }
    if payload[3] != VIDEO_BIN_VER {
        return None;
    }
    let key = payload[4] & 1 != 0;
    let share_len = payload[5] as usize;
    let header = 6 + share_len + 8;
    if payload.len() < header {
        return None;
    }
    let share_id = std::str::from_utf8(&payload[6..6 + share_len])
        .ok()?
        .to_string();
    let dim = 6 + share_len;
    let width = u16::from_le_bytes([payload[dim], payload[dim + 1]]) as u32;
    let height = u16::from_le_bytes([payload[dim + 2], payload[dim + 3]]) as u32;
    let h264 = &payload[header..];
    if h264.is_empty() {
        return None;
    }
    Some((share_id, width, height, key, h264))
}

fn present_h264(session_id: u32, share_id: &str, hint_w: u32, hint_h: u32, bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }
    {
        let mut guard = lock_inner();
        match guard.session.as_mut() {
            Some(session)
                if session.session_id == session_id
                    && session.phase == DesktopPhase::Controlling
                    && (share_id.is_empty() || session.share_id == share_id) =>
            {
                if hint_w > 0 {
                    session.width = hint_w;
                }
                if hint_h > 0 {
                    session.height = hint_h;
                }
            }
            _ => return,
        }
    }
    let decoded = {
        let mut decoder = lock_decoder();
        match decoder.decode(bytes) {
            Ok(Some(yuv)) => {
                let (w, h) = yuv.dimensions();
                let mut rgb = vec![0u8; yuv.rgb8_len()];
                yuv.write_rgb8(&mut rgb);
                Some((w as u32, h as u32, rgb))
            }
            Ok(None) => None,
            Err(err) => {
                eprintln!("desktop: decode {err}");
                reset_decoder();
                send_signal(session_id, "key", share_id, 0, 0);
                None
            }
        }
    };
    let Some((width, height, rgb)) = decoded else {
        return;
    };
    let Ok(jpeg) = jpeg_from_rgb(width, height, &rgb) else {
        return;
    };
    recv_stats().note(bytes.len() as u64);
    if let Some((fps, kbps)) = recv_stats().take_if_due() {
        let mut guard = lock_inner();
        if guard
            .session
            .as_ref()
            .map(|item| item.session_id == session_id && item.phase == DesktopPhase::Controlling)
            .unwrap_or(false)
        {
            guard.fps = fps;
            guard.kbps = kbps;
            if let Some(session) = guard.session.as_mut() {
                session.width = width;
                session.height = height;
            }
            emit_state(snapshot(&guard));
        }
    }
    if let Some(app) = webrpc::app_handle() {
        let _ = app.emit(
            "webrpc-desktop-frame",
            DesktopFrameEvent {
                session_id,
                width,
                height,
                jpeg: B64.encode(&jpeg),
            },
        );
    }
}

fn decoder_cell() -> &'static Mutex<Decoder> {
    static CELL: OnceLock<Mutex<Decoder>> = OnceLock::new();
    CELL.get_or_init(|| {
        Mutex::new(Decoder::new().unwrap_or_else(|err| panic!("init h264 decoder: {err}")))
    })
}

fn lock_decoder() -> std::sync::MutexGuard<'static, Decoder> {
    decoder_cell()
        .lock()
        .unwrap_or_else(|err| err.into_inner())
}

fn reset_decoder() {
    if let Ok(next) = Decoder::new() {
        *lock_decoder() = next;
    }
}

struct RecvStats {
    frames: AtomicU32,
    bytes: AtomicU64,
    started: Mutex<Instant>,
}

impl RecvStats {
    fn reset(&self) {
        self.frames.store(0, Ordering::Relaxed);
        self.bytes.store(0, Ordering::Relaxed);
        *self.started.lock().unwrap_or_else(|err| err.into_inner()) = Instant::now();
    }

    fn note(&self, bytes: u64) {
        self.frames.fetch_add(1, Ordering::Relaxed);
        self.bytes.fetch_add(bytes, Ordering::Relaxed);
    }

    fn take_if_due(&self) -> Option<(u32, u32)> {
        let mut started = self.started.lock().unwrap_or_else(|err| err.into_inner());
        if started.elapsed() < Duration::from_millis(800) {
            return None;
        }
        let secs = started.elapsed().as_secs_f32().max(0.2);
        let fps = (self.frames.swap(0, Ordering::Relaxed) as f32 / secs).round() as u32;
        let kbps = (self.bytes.swap(0, Ordering::Relaxed) as f32 * 8.0 / secs / 1000.0).round() as u32;
        *started = Instant::now();
        Some((fps, kbps))
    }
}

fn recv_stats() -> &'static RecvStats {
    static CELL: OnceLock<RecvStats> = OnceLock::new();
    CELL.get_or_init(|| RecvStats {
        frames: AtomicU32::new(0),
        bytes: AtomicU64::new(0),
        started: Mutex::new(Instant::now()),
    })
}

#[derive(Clone, Copy)]
struct ScreenMap {
    x: i32,
    y: i32,
    w: u32,
    h: u32,
}

fn screen_cell() -> &'static Mutex<Option<ScreenMap>> {
    static CELL: OnceLock<Mutex<Option<ScreenMap>>> = OnceLock::new();
    CELL.get_or_init(|| Mutex::new(None))
}

fn remember_screen(monitor: &xcap::Monitor) {
    #[cfg(target_os = "macos")]
    {
        let _ = monitor;
        let (x, y, w, h) = desktop_mac::logical_screen();
        *screen_cell().lock().unwrap_or_else(|err| err.into_inner()) = Some(ScreenMap { x, y, w, h });
        return;
    }
    #[cfg(not(target_os = "macos"))]
    {
        let map = ScreenMap {
            x: monitor.x().unwrap_or(0),
            y: monitor.y().unwrap_or(0),
            w: monitor.width().unwrap_or(1).max(1),
            h: monitor.height().unwrap_or(1).max(1),
        };
        *screen_cell().lock().unwrap_or_else(|err| err.into_inner()) = Some(map);
    }
}

fn clear_screen_map() {
    *screen_cell().lock().unwrap_or_else(|err| err.into_inner()) = None;
}

fn current_screen() -> Option<ScreenMap> {
    *screen_cell().lock().unwrap_or_else(|err| err.into_inner())
}

fn norm_to_screen(nx: f32, ny: f32) -> Option<(i32, i32)> {
    let map = current_screen()?;
    let x = map.x + (nx.clamp(0.0, 1.0) * map.w as f32).round() as i32;
    let y = map.y + (ny.clamp(0.0, 1.0) * map.h as f32).round() as i32;
    Some((x, y))
}

#[derive(Serialize)]
struct InputMessage<'a> {
    #[serde(rename = "type")]
    kind: u8,
    data: InputData<'a>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InputData<'a> {
    share_id: &'a str,
    op: &'a str,
    btn: &'a str,
    x: f32,
    y: f32,
    dx: i32,
    dy: i32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InboundInput {
    #[serde(default)]
    share_id: String,
    #[serde(default)]
    op: String,
    #[serde(default)]
    btn: String,
    #[serde(default)]
    x: f32,
    #[serde(default)]
    y: f32,
    #[serde(default)]
    dx: i32,
    #[serde(default)]
    dy: i32,
}

#[tauri::command]
pub fn desktop_input(
    op: String,
    btn: Option<String>,
    x: Option<f32>,
    y: Option<f32>,
    dx: Option<i32>,
    dy: Option<i32>,
) -> Result<(), String> {
    let (session_id, share_id) = {
        let guard = lock_inner();
        let session = guard
            .session
            .as_ref()
            .ok_or_else(|| "当前没有远程控制。".to_string())?;
        if session.phase != DesktopPhase::Controlling {
            return Err("只有控制端可以发送鼠标操作。".into());
        }
        (session.session_id, session.share_id.clone())
    };
    queue_mouse_send(QueuedInput {
        session_id,
        share_id,
        op: op.trim().to_string(),
        btn: btn.unwrap_or_default(),
        x: x.unwrap_or(0.0),
        y: y.unwrap_or(0.0),
        dx: dx.unwrap_or(0),
        dy: dy.unwrap_or(0),
    });
    Ok(())
}

#[derive(Clone)]
struct QueuedInput {
    session_id: u32,
    share_id: String,
    op: String,
    btn: String,
    x: f32,
    y: f32,
    dx: i32,
    dy: i32,
}

struct MouseOutbox {
    clicks: VecDeque<QueuedInput>,
    latest_move: Option<QueuedInput>,
}

fn mouse_outbox() -> &'static Mutex<MouseOutbox> {
    static CELL: OnceLock<Mutex<MouseOutbox>> = OnceLock::new();
    CELL.get_or_init(|| {
        ensure_mouse_send_loop();
        Mutex::new(MouseOutbox {
            clicks: VecDeque::new(),
            latest_move: None,
        })
    })
}

fn queue_mouse_send(item: QueuedInput) {
    let mut box_ = mouse_outbox().lock().unwrap_or_else(|err| err.into_inner());
    if item.op == "move" {
        box_.latest_move = Some(item);
        return;
    }
    if item.op == "up" {
        while box_.clicks.len() >= 64 {
            if let Some(idx) = box_.clicks.iter().position(|c| c.op != "up") {
                box_.clicks.remove(idx);
            } else {
                box_.clicks.pop_front();
                break;
            }
        }
        box_.clicks.push_back(item);
        return;
    }
    if box_.clicks.len() >= 64 {
        box_.clicks.pop_front();
    }
    box_.clicks.push_back(item);
}

fn take_mouse_sends() -> Vec<QueuedInput> {
    let mut box_ = mouse_outbox().lock().unwrap_or_else(|err| err.into_inner());
    let mut out = Vec::with_capacity(box_.clicks.len() + 1);
    out.extend(box_.clicks.drain(..));
    if let Some(item) = box_.latest_move.take() {
        out.push(item);
    }
    out
}

fn send_queued_input(item: &QueuedInput) -> bool {
    let Ok(payload) = serde_json::to_string(&InputMessage {
        kind: DESKTOP_INPUT_TYPE,
        data: InputData {
            share_id: &item.share_id,
            op: &item.op,
            btn: &item.btn,
            x: item.x,
            y: item.y,
            dx: item.dx,
            dy: item.dy,
        },
    }) else {
        return false;
    };
    let timeout = if item.op == "move" {
        SEND_TIMEOUT_MS
    } else {
        CLICK_SEND_TIMEOUT_MS
    };
    webrpc::send_json_timeout(item.session_id, &payload, timeout)
}

fn flush_mouse_sends() {
    let items = take_mouse_sends();
    let mut retry = Vec::new();
    for item in items {
        if !send_queued_input(&item) && item.op == "up" {
            retry.push(item);
        }
    }
    if retry.is_empty() {
        return;
    }
    let mut box_ = mouse_outbox().lock().unwrap_or_else(|err| err.into_inner());
    for item in retry.into_iter().rev() {
        box_.clicks.push_front(item);
    }
}

fn ensure_mouse_send_loop() {
    static START: OnceLock<()> = OnceLock::new();
    START.get_or_init(|| {
        let _ = thread::Builder::new()
            .name("desktop-mouse-send".into())
            .spawn(|| loop {
                thread::sleep(Duration::from_millis(8));
                flush_mouse_sends();
            });
    });
}

pub fn on_input(session_id: u32, data: serde_json::Value) {
    let parsed: InboundInput = match serde_json::from_value(data) {
        Ok(v) => v,
        Err(_) => return,
    };
    {
        let guard = lock_inner();
        let Some(session) = guard.session.as_ref() else {
            return;
        };
        if session.phase != DesktopPhase::Controlled
            || session.session_id != session_id
            || (!parsed.share_id.is_empty() && session.share_id != parsed.share_id)
        {
            return;
        }
    }
    if let Err(err) = inject_input(&parsed) {
        eprintln!("desktop: inject {err}");
    }
}

enum MouseJob {
    Move(i32, i32),
    Down(Button, i32, i32),
    Up(Button, i32, i32),
    Dbl(Button, i32, i32),
    Wheel(i32, i32, i32, i32),
    Reset,
}

#[derive(Default)]
struct HeldButtons {
    left: Option<Instant>,
    right: Option<Instant>,
    middle: Option<Instant>,
}

fn inject_input(parsed: &InboundInput) -> Result<(), String> {
    let Some((x, y)) = norm_to_screen(parsed.x, parsed.y) else {
        return Err("被控端屏幕信息未就绪".into());
    };
    let button = match parsed.btn.trim() {
        "right" => Button::Right,
        "middle" => Button::Middle,
        _ => Button::Left,
    };
    let job = match parsed.op.trim() {
        "move" => MouseJob::Move(x, y),
        "down" => MouseJob::Down(button, x, y),
        "up" => MouseJob::Up(button, x, y),
        "dblclick" => MouseJob::Dbl(button, x, y),
        "wheel" => MouseJob::Wheel(x, y, parsed.dx, parsed.dy),
        _ => return Ok(()),
    };
    enqueue_mouse(job);
    Ok(())
}

fn mouse_cell() -> &'static OnceLock<std::sync::mpsc::Sender<MouseJob>> {
    static CELL: OnceLock<std::sync::mpsc::Sender<MouseJob>> = OnceLock::new();
    &CELL
}

fn mouse_tx() -> &'static std::sync::mpsc::Sender<MouseJob> {
    mouse_cell().get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::channel::<MouseJob>();
        thread::Builder::new()
            .name("desktop-mouse".into())
            .spawn(move || run_mouse_loop(rx))
            .expect("desktop mouse thread");
        tx
    })
}

fn enqueue_mouse(job: MouseJob) {
    let _ = mouse_tx().send(job);
}

fn try_reset_injected_mouse() {
    if let Some(tx) = mouse_cell().get() {
        let _ = tx.send(MouseJob::Reset);
    }
}

fn run_mouse_loop(rx: std::sync::mpsc::Receiver<MouseJob>) {
    #[cfg(target_os = "macos")]
    {
        let mut held = HeldButtons::default();
        loop {
            let jobs = take_mouse_jobs(&rx);
            match jobs {
                None => break,
                Some(jobs) => {
                    for job in jobs {
                        if let Err(err) = apply_mouse_mac(&mut held, job) {
                            eprintln!("desktop: mouse {err}");
                            release_all_buttons_mac(&mut held);
                        }
                    }
                    release_stuck_buttons_mac(&mut held);
                }
            }
        }
        return;
    }

    #[cfg(not(target_os = "macos"))]
    {
        let mut mouse = match Enigo::new(&Settings::default()) {
            Ok(item) => item,
            Err(err) => {
                eprintln!("desktop: init mouse failed: {err}");
                return;
            }
        };
        let mut held = HeldButtons::default();
        loop {
            let jobs = take_mouse_jobs(&rx);
            match jobs {
                None => break,
                Some(jobs) => {
                    for job in jobs {
                        if let Err(err) = apply_mouse(&mut mouse, &mut held, job) {
                            eprintln!("desktop: mouse {err}");
                            release_all_buttons(&mut mouse, &mut held);
                        }
                    }
                    release_stuck_buttons(&mut mouse, &mut held);
                }
            }
        }
    }
}

fn take_mouse_jobs(rx: &std::sync::mpsc::Receiver<MouseJob>) -> Option<Vec<MouseJob>> {
    let first = match rx.recv_timeout(Duration::from_millis(200)) {
        Ok(job) => Some(job),
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => None,
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return None,
    };
    let mut jobs = Vec::new();
    if let Some(job) = first {
        jobs.push(job);
        while let Ok(next) = rx.try_recv() {
            if matches!(next, MouseJob::Move(_, _)) && matches!(jobs.last(), Some(MouseJob::Move(_, _)))
            {
                *jobs.last_mut().unwrap() = next;
            } else {
                jobs.push(next);
            }
        }
    }
    Some(jobs)
}

fn held_slot(held: &mut HeldButtons, button: Button) -> Option<&mut Option<Instant>> {
    match button {
        Button::Left => Some(&mut held.left),
        Button::Right => Some(&mut held.right),
        Button::Middle => Some(&mut held.middle),
        _ => None,
    }
}

fn mark_down(held: &mut HeldButtons, button: Button) {
    if let Some(slot) = held_slot(held, button) {
        *slot = Some(Instant::now());
    }
}

fn mark_up(held: &mut HeldButtons, button: Button) {
    if let Some(slot) = held_slot(held, button) {
        *slot = None;
    }
}

fn button_hold_limit(button: Button) -> Duration {
    match button {
        Button::Right | Button::Middle => Duration::from_millis(800),
        _ => Duration::from_secs(10),
    }
}

#[cfg(target_os = "macos")]
fn release_stuck_buttons_mac(held: &mut HeldButtons) {
    let now = Instant::now();
    let stuck: Vec<Button> = [
        (held.left, Button::Left),
        (held.right, Button::Right),
        (held.middle, Button::Middle),
    ]
    .into_iter()
    .filter_map(|(at, button)| {
        at.filter(|start| now.saturating_duration_since(*start) >= button_hold_limit(button))
            .map(|_| button)
    })
    .collect();
    for button in stuck {
        let (x, y) = desktop_mac::last_xy();
        let _ = desktop_mac::button_at(button, Direction::Release, x, y, 1);
        mark_up(held, button);
    }
}

#[cfg(target_os = "macos")]
fn release_all_buttons_mac(held: &mut HeldButtons) {
    let (x, y) = desktop_mac::last_xy();
    for button in [Button::Left, Button::Right, Button::Middle] {
        let _ = desktop_mac::button_at(button, Direction::Release, x, y, 1);
    }
    held.left = None;
    held.right = None;
    held.middle = None;
}

#[cfg(target_os = "macos")]
fn apply_mouse_mac(held: &mut HeldButtons, job: MouseJob) -> Result<(), String> {
    let drag = if held.left.is_some() {
        Some(Button::Left)
    } else if held.right.is_some() {
        Some(Button::Right)
    } else if held.middle.is_some() {
        Some(Button::Middle)
    } else {
        None
    };
    match job {
        MouseJob::Move(x, y) => desktop_mac::move_abs(x, y, drag),
        MouseJob::Down(button, x, y) => {
            desktop_mac::move_abs(x, y, None)?;
            desktop_mac::button_at(button, Direction::Press, x, y, 1)?;
            mark_down(held, button);
            Ok(())
        }
        MouseJob::Up(button, x, y) => {
            desktop_mac::move_abs(x, y, None)?;
            desktop_mac::button_at(button, Direction::Release, x, y, 1)?;
            let _ = desktop_mac::button_at(button, Direction::Release, x, y, 1);
            mark_up(held, button);
            Ok(())
        }
        MouseJob::Dbl(button, x, y) => {
            desktop_mac::move_abs(x, y, None)?;
            desktop_mac::button_at(button, Direction::Click, x, y, 1)?;
            thread::sleep(Duration::from_millis(40));
            desktop_mac::button_at(button, Direction::Click, x, y, 2)?;
            mark_up(held, button);
            Ok(())
        }
        MouseJob::Wheel(x, y, dx, dy) => {
            desktop_mac::move_abs(x, y, None)?;
            desktop_mac::scroll(dx, dy)
        }
        MouseJob::Reset => {
            release_all_buttons_mac(held);
            Ok(())
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn release_stuck_buttons(mouse: &mut Enigo, held: &mut HeldButtons) {
    let now = Instant::now();
    let stuck: Vec<Button> = [
        (held.left, Button::Left),
        (held.right, Button::Right),
        (held.middle, Button::Middle),
    ]
    .into_iter()
    .filter_map(|(at, button)| {
        at.filter(|start| now.saturating_duration_since(*start) >= button_hold_limit(button))
            .map(|_| button)
    })
    .collect();
    if stuck.is_empty() {
        return;
    }
    for button in stuck {
        let _ = mouse.button(button, Direction::Release);
        mark_up(held, button);
    }
    restore_os_cursor();
}

#[cfg(not(target_os = "macos"))]
fn release_all_buttons(mouse: &mut Enigo, held: &mut HeldButtons) {
    for button in [Button::Left, Button::Right, Button::Middle] {
        let _ = mouse.button(button, Direction::Release);
    }
    held.left = None;
    held.right = None;
    held.middle = None;
    restore_os_cursor();
}

fn restore_os_cursor() {
    #[cfg(target_os = "windows")]
    crate::win::show_mouse_cursor();
}

#[cfg(not(target_os = "macos"))]
fn apply_mouse(mouse: &mut Enigo, held: &mut HeldButtons, job: MouseJob) -> Result<(), String> {
    match job {
        MouseJob::Move(x, y) => mouse
            .move_mouse(x, y, Coordinate::Abs)
            .map_err(|err| format!("移动鼠标失败: {err}")),
        MouseJob::Down(button, x, y) => {
            mouse
                .move_mouse(x, y, Coordinate::Abs)
                .map_err(|err| format!("移动鼠标失败: {err}"))?;
            mouse
                .button(button, Direction::Press)
                .map_err(|err| format!("按下鼠标失败: {err}"))?;
            mark_down(held, button);
            Ok(())
        }
        MouseJob::Up(button, x, y) => {
            mouse
                .move_mouse(x, y, Coordinate::Abs)
                .map_err(|err| format!("移动鼠标失败: {err}"))?;
            mouse
                .button(button, Direction::Release)
                .map_err(|err| format!("松开鼠标失败: {err}"))?;
            let _ = mouse.button(button, Direction::Release);
            mark_up(held, button);
            restore_os_cursor();
            Ok(())
        }
        MouseJob::Dbl(button, x, y) => {
            mouse
                .move_mouse(x, y, Coordinate::Abs)
                .map_err(|err| format!("移动鼠标失败: {err}"))?;
            mouse
                .button(button, Direction::Click)
                .map_err(|err| format!("双击失败: {err}"))?;
            thread::sleep(Duration::from_millis(40));
            mouse
                .button(button, Direction::Click)
                .map_err(|err| format!("双击失败: {err}"))?;
            mark_up(held, button);
            restore_os_cursor();
            Ok(())
        }
        MouseJob::Wheel(x, y, dx, dy) => {
            mouse
                .move_mouse(x, y, Coordinate::Abs)
                .map_err(|err| format!("移动鼠标失败: {err}"))?;
            if dy != 0 {
                mouse
                    .scroll(dy, Axis::Vertical)
                    .map_err(|err| format!("滚轮失败: {err}"))?;
            }
            if dx != 0 {
                mouse
                    .scroll(dx, Axis::Horizontal)
                    .map_err(|err| format!("滚轮失败: {err}"))?;
            }
            Ok(())
        }
        MouseJob::Reset => {
            release_all_buttons(mouse, held);
            Ok(())
        }
    }
}
