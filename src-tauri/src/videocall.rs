use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{self, SyncSender};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, Stream};
use image::{DynamicImage, RgbaImage};
use openh264::decoder::Decoder as H264Decoder;
use openh264::encoder::{
    BitRate, Complexity, Encoder, EncoderConfig, FrameRate, IntraFramePeriod, UsageType,
};
use openh264::formats::{RgbSliceU8, YUVBuffer, YUVSource};
use ropus::{Application, Bitrate, Channels, DecodeMode, Decoder, Encoder as OpusEncoder};
use serde::{Deserialize, Serialize};
use tauri::Emitter;

use crate::webrpc;

#[cfg(any(target_os = "windows", target_os = "macos"))]
use crate::camera::CameraCapture;

const SAMPLE_RATE: u32 = 16_000;
const FRAME_MS: u32 = 20;
const FRAME_SAMPLES: usize = (SAMPLE_RATE * FRAME_MS / 1000) as usize;
const RING_TIMEOUT_MS: u64 = 45_000;
const SEND_TIMEOUT_MS: i64 = 0;
const AUDIO_BIN_VER: u8 = 2;
/// webrpc 二进制路由：0/0/0/3 → videocall（1 为远程桌面）
const VIDEO_BIN_VER: u8 = 3;
const OPUS_PACKET_MAX: usize = 4000;
const OPUS_DECODE_MAX_SAMPLES: usize = (SAMPLE_RATE as usize * 120) / 1000;
const OPUS_BITRATE: u32 = 32_000;
const MAX_BUFFER_SEC: u32 = 2;
const MAX_SEND_FRAMES: usize = (MAX_BUFFER_SEC * 1000 / FRAME_MS) as usize;
const MAX_PLAY_SAMPLES: usize = (SAMPLE_RATE * MAX_BUFFER_SEC) as usize;
const RECV_OPUS_QUEUE: usize = MAX_SEND_FRAMES;
const MAX_EDGE: u32 = 480;
const TARGET_FPS: u32 = 8;
const FRAME_INTERVAL: Duration = Duration::from_millis(1000 / TARGET_FPS as u64);
/// 与 SendBudget 对齐，避免编码出超大帧后被整帧丢弃导致卡顿
const VIDEO_BITRATE_BPS: u32 = 180_000;
const KEY_EVERY: u32 = 24;
/// 双向合计约 256KB/s（每端出站 128KB/s）；音频约 4KB/s，其余给 H.264
const OUT_BYTES_PER_SEC: usize = 128 * 1024;
const SESSION_SIGNAL_TYPE: u8 = 12;
const SIGNAL_TIMEOUT_MS: i64 = 10_000;
const HANGUP_SIGNAL_TIMEOUT_MS: i64 = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum VCallPhase {
    Idle,
    Outgoing,
    Incoming,
    Active,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VCallUiState {
    pub phase: VCallPhase,
    pub session_id: u32,
    pub call_id: String,
    pub peer_token: String,
    pub muted: bool,
    pub has_camera: bool,
    pub started_at: u64,
}

impl VCallUiState {
    fn idle() -> Self {
        Self {
            phase: VCallPhase::Idle,
            session_id: 0,
            call_id: String::new(),
            peer_token: String::new(),
            muted: false,
            has_camera: false,
            started_at: 0,
        }
    }
}

struct Call {
    phase: VCallPhase,
    chat_session_id: u32,
    media_session_id: u32,
    call_id: String,
    peer_token: String,
    peer_pass: String,
    is_inviter: bool,
    muted: bool,
    has_camera: bool,
    started_at: Instant,
    started_unix_ms: u64,
}

#[allow(dead_code)]
struct SendStream(Stream);
unsafe impl Send for SendStream {}

struct LiveMedia {
    stop: Arc<AtomicBool>,
    _input: SendStream,
    _output: SendStream,
    _audio_recv: thread::JoinHandle<()>,
    _audio_send: thread::JoinHandle<()>,
    _video_send: Option<thread::JoinHandle<()>>,
    _video_recv: thread::JoinHandle<()>,
}

struct VCallInner {
    call: Option<Call>,
    media: Option<LiveMedia>,
}

fn inner() -> &'static Mutex<VCallInner> {
    static CELL: OnceLock<Mutex<VCallInner>> = OnceLock::new();
    CELL.get_or_init(|| Mutex::new(VCallInner { call: None, media: None }))
}

fn send_pcm_buf() -> Arc<Mutex<VecDeque<Vec<i16>>>> {
    static CELL: OnceLock<Arc<Mutex<VecDeque<Vec<i16>>>>> = OnceLock::new();
    CELL.get_or_init(|| Arc::new(Mutex::new(VecDeque::new()))).clone()
}

fn play_pcm_buf() -> Arc<Mutex<VecDeque<i16>>> {
    static CELL: OnceLock<Arc<Mutex<VecDeque<i16>>>> = OnceLock::new();
    CELL.get_or_init(|| Arc::new(Mutex::new(VecDeque::new()))).clone()
}

fn opus_queue() -> Arc<Mutex<OpusQueue>> {
    static CELL: OnceLock<Arc<Mutex<OpusQueue>>> = OnceLock::new();
    CELL.get_or_init(|| Arc::new(Mutex::new(OpusQueue::new()))).clone()
}

fn capture_accum() -> Arc<Mutex<Vec<i16>>> {
    static CELL: OnceLock<Arc<Mutex<Vec<i16>>>> = OnceLock::new();
    CELL.get_or_init(|| Arc::new(Mutex::new(Vec::new()))).clone()
}

fn audio_recv_tx() -> Arc<Mutex<Option<SyncSender<(u32, Vec<u8>)>>>> {
    static CELL: OnceLock<Arc<Mutex<Option<SyncSender<(u32, Vec<u8>)>>>>> = OnceLock::new();
    CELL.get_or_init(|| Arc::new(Mutex::new(None))).clone()
}

fn video_recv_tx() -> Arc<Mutex<Option<SyncSender<(u32, Vec<u8>)>>>> {
    static CELL: OnceLock<Arc<Mutex<Option<SyncSender<(u32, Vec<u8>)>>>>> = OnceLock::new();
    CELL.get_or_init(|| Arc::new(Mutex::new(None))).clone()
}

fn send_budget() -> Arc<Mutex<SendBudget>> {
    static CELL: OnceLock<Arc<Mutex<SendBudget>>> = OnceLock::new();
    CELL.get_or_init(|| Arc::new(Mutex::new(SendBudget::new()))).clone()
}

struct OpusQueue {
    pending: BTreeMap<u32, Vec<u8>>,
    next_seq: Option<u32>,
}

impl OpusQueue {
    fn new() -> Self {
        Self {
            pending: BTreeMap::new(),
            next_seq: None,
        }
    }
    fn clear(&mut self) {
        self.pending.clear();
        self.next_seq = None;
    }
    fn insert(&mut self, seq: u32, opus: Vec<u8>) {
        if opus.is_empty() {
            return;
        }
        if self.next_seq.is_none() {
            self.next_seq = Some(seq);
        }
        self.pending.insert(seq, opus);
        while self.pending.len() > RECV_OPUS_QUEUE {
            if let Some(oldest) = self.pending.keys().next().copied() {
                if self.next_seq.is_some_and(|n| oldest >= n) {
                    break;
                }
                self.pending.remove(&oldest);
            } else {
                break;
            }
        }
    }
    fn pop_in_order(&mut self) -> Option<Vec<u8>> {
        let next = self.next_seq?;
        let opus = self.pending.remove(&next)?;
        self.next_seq = Some(next + 1);
        Some(opus)
    }
}

struct SendBudget {
    window_start: Instant,
    bytes_sent: usize,
}

impl SendBudget {
    fn new() -> Self {
        Self {
            window_start: Instant::now(),
            bytes_sent: 0,
        }
    }
    fn charge(&mut self, nbytes: usize) -> bool {
        let now = Instant::now();
        if now.duration_since(self.window_start) >= Duration::from_secs(1) {
            self.window_start = now;
            self.bytes_sent = 0;
        }
        if self.bytes_sent.saturating_add(nbytes) > OUT_BYTES_PER_SEC {
            return false;
        }
        self.bytes_sent = self.bytes_sent.saturating_add(nbytes);
        true
    }
    fn reset(&mut self) {
        self.window_start = Instant::now();
        self.bytes_sent = 0;
    }
}

fn lock_inner() -> std::sync::MutexGuard<'static, VCallInner> {
    inner().lock().unwrap_or_else(|e| e.into_inner())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn is_busy() -> bool {
    lock_inner().call.is_some()
}

fn emit_state(state: VCallUiState) {
    if let Some(app) = webrpc::app_handle() {
        let _ = app.emit("webrpc-vcall-state", state);
    }
}

fn emit_error(msg: &str) {
    if let Some(app) = webrpc::app_handle() {
        let _ = app.emit("webrpc-vcall-error", msg);
    }
}

fn snapshot(call: Option<&Call>) -> VCallUiState {
    let Some(call) = call else {
        return VCallUiState::idle();
    };
    VCallUiState {
        phase: call.phase,
        session_id: call.chat_session_id,
        call_id: call.call_id.clone(),
        peer_token: call.peer_token.clone(),
        muted: call.muted,
        has_camera: call.has_camera,
        started_at: call.started_unix_ms,
    }
}

fn reset_buffers() {
    send_pcm_buf().lock().unwrap_or_else(|e| e.into_inner()).clear();
    play_pcm_buf().lock().unwrap_or_else(|e| e.into_inner()).clear();
    opus_queue().lock().unwrap_or_else(|e| e.into_inner()).clear();
    capture_accum().lock().unwrap_or_else(|e| e.into_inner()).clear();
    send_budget().lock().unwrap_or_else(|e| e.into_inner()).reset();
}

fn stop_media(media: &Option<LiveMedia>) {
    if let Some(live) = media {
        live.stop.store(true, Ordering::SeqCst);
    }
}

fn clear_to_idle(guard: &mut VCallInner, emit: bool, close_webrpc: bool) {
    let (media_id, is_inviter) = guard
        .call
        .as_ref()
        .map(|c| (c.media_session_id, c.is_inviter))
        .unwrap_or((0, false));
    stop_media(&guard.media);
    guard.media = None;
    guard.call = None;
    *audio_recv_tx().lock().unwrap_or_else(|e| e.into_inner()) = None;
    *video_recv_tx().lock().unwrap_or_else(|e| e.into_inner()) = None;
    reset_buffers();
    if close_webrpc && is_inviter && media_id > 0 {
        let id = media_id;
        thread::Builder::new()
            .name("vcall-close".into())
            .spawn(move || webrpc::close_videocall_webrpc_session(id))
            .ok();
    }
    if emit {
        emit_state(VCallUiState::idle());
    }
}

fn has_local_camera() -> bool {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        crate::camera::probe_camera()
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        false
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
    call_id: &'a str,
}

#[derive(Serialize)]
struct SessionSignal<'a> {
    #[serde(rename = "type")]
    kind: u8,
    data: SessionSignalData<'a>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionSignalData<'a> {
    op: &'a str,
    call_id: &'a str,
    session_id: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InboundSignal {
    op: String,
    #[serde(default)]
    call_id: String,
}

fn send_signal(chat_id: u32, op: &str, call_id: &str) {
    send_signal_timeout(chat_id, op, call_id, SIGNAL_TIMEOUT_MS);
}

fn send_signal_timeout(chat_id: u32, op: &str, call_id: &str, timeout_ms: i64) {
    let Ok(payload) = serde_json::to_string(&SignalMessage {
        kind: 6,
        data: SignalData { op, call_id },
    }) else {
        return;
    };
    let _ = webrpc::send_json_timeout(chat_id, &payload, timeout_ms);
}

fn send_session_signal(chat_id: u32, op: &str, call_id: &str, media_id: u32) {
    send_session_signal_timeout(chat_id, op, call_id, media_id, SIGNAL_TIMEOUT_MS);
}

fn send_session_signal_timeout(
    chat_id: u32,
    op: &str,
    call_id: &str,
    media_id: u32,
    timeout_ms: i64,
) {
    if chat_id == 0 || media_id == 0 {
        return;
    }
    let Ok(payload) = serde_json::to_string(&SessionSignal {
        kind: SESSION_SIGNAL_TYPE,
        data: SessionSignalData {
            op,
            call_id,
            session_id: media_id,
        },
    }) else {
        return;
    };
    let _ = webrpc::send_json_timeout(chat_id, &payload, timeout_ms);
}

/// 本地已清理后，后台尽力通知对端并关闭媒体会话（不阻塞 UI）。
fn spawn_hangup_notify(chat_id: u32, op: &str, call_id: &str, media_id: u32, is_inviter: bool) {
    let op = op.to_string();
    let call_id = call_id.to_string();
    thread::Builder::new()
        .name("vcall-hangup".into())
        .spawn(move || {
            if media_id > 0 {
                send_session_signal_timeout(
                    chat_id,
                    "stop",
                    &call_id,
                    media_id,
                    HANGUP_SIGNAL_TIMEOUT_MS,
                );
            }
            send_signal_timeout(chat_id, &op, &call_id, HANGUP_SIGNAL_TIMEOUT_MS);
            if is_inviter && media_id > 0 {
                webrpc::close_videocall_webrpc_session(media_id);
            }
        })
        .ok();
}

fn new_call_id(chat_session_id: u32) -> String {
    format!("vc-{chat_session_id}-{}", now_ms())
}

fn end_call(op: &str) -> Result<(), String> {
    let (chat_id, call_id, media_id, is_inviter) = {
        let mut guard = lock_inner();
        let Some(call) = guard.call.take() else {
            return Ok(());
        };
        stop_media(&guard.media);
        guard.media = None;
        *audio_recv_tx().lock().unwrap_or_else(|e| e.into_inner()) = None;
        *video_recv_tx().lock().unwrap_or_else(|e| e.into_inner()) = None;
        reset_buffers();
        (
            call.chat_session_id,
            call.call_id,
            call.media_session_id,
            call.is_inviter,
        )
    };
    emit_state(VCallUiState::idle());
    spawn_hangup_notify(chat_id, op, &call_id, media_id, is_inviter);
    Ok(())
}

#[tauri::command]
pub fn vcall_invite(session_id: u32, peer_pass: Option<String>) -> Result<(), String> {
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = (session_id, peer_pass);
        return Err("当前平台暂不支持视频通话。".into());
    }
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        if session_id == 0 {
            return Err("请先连接会话再发起视频通话。".into());
        }
        if webrpc::rpc_handle() == 0 {
            return Err("尚未登录。".into());
        }
        if crate::voice::is_busy() {
            return Err("当前已有语音通话，请先结束。".into());
        }
        let peer = webrpc::peer_token_of(session_id);
        let mut pass = peer_pass.unwrap_or_default().trim().to_string();
        if pass.is_empty() {
            pass = webrpc::peer_pass_of(session_id);
        }
        if pass.is_empty() {
            return Err("缺少对方口令，请重新连接聊天会话后再试。".into());
        }
        webrpc::remember_session_pass(session_id, &pass);
        let call_id = new_call_id(session_id);
        let has_camera = has_local_camera();
        {
            let mut guard = lock_inner();
            if guard.call.is_some() {
                return Err("当前已有视频通话，请先结束。".into());
            }
            guard.call = Some(Call {
                phase: VCallPhase::Outgoing,
                chat_session_id: session_id,
                media_session_id: 0,
                call_id: call_id.clone(),
                peer_token: peer,
                peer_pass: pass,
                is_inviter: true,
                muted: false,
                has_camera,
                started_at: Instant::now(),
                started_unix_ms: now_ms(),
            });
            emit_state(snapshot(guard.call.as_ref()));
        }
        send_signal(session_id, "vcall_invite", &call_id);
        spawn_ring_timeout(session_id, call_id, VCallPhase::Outgoing);
        Ok(())
    }
}

#[tauri::command]
pub fn vcall_accept() -> Result<(), String> {
    let (chat_id, call_id) = {
        let mut guard = lock_inner();
        let call = guard
            .call
            .as_mut()
            .ok_or_else(|| "没有待接听的视频通话。".to_string())?;
        if call.phase != VCallPhase::Incoming {
            return Err("当前没有待接听的视频通话。".into());
        }
        call.phase = VCallPhase::Active;
        call.started_at = Instant::now();
        call.started_unix_ms = now_ms();
        (call.chat_session_id, call.call_id.clone())
    };
    send_signal(chat_id, "vcall_accept", &call_id);
    emit_state(snapshot(lock_inner().call.as_ref()));
    Ok(())
}

#[tauri::command]
pub fn vcall_reject() -> Result<(), String> {
    let op = match lock_inner().call.as_ref().map(|c| c.phase) {
        Some(VCallPhase::Incoming) => "vcall_reject",
        Some(VCallPhase::Outgoing) => "vcall_cancel",
        _ => "vcall_hangup",
    };
    end_call(op)
}

#[tauri::command]
pub fn vcall_hangup() -> Result<(), String> {
    end_call("vcall_hangup")
}

#[tauri::command]
pub fn vcall_set_mute(muted: bool) -> Result<(), String> {
    mute_flag().store(muted, Ordering::Relaxed);
    let mut guard = lock_inner();
    let Some(call) = guard.call.as_mut() else {
        return Ok(());
    };
    if call.phase != VCallPhase::Active {
        return Ok(());
    }
    call.muted = muted;
    emit_state(snapshot(Some(call)));
    Ok(())
}

#[tauri::command]
pub fn vcall_state() -> VCallUiState {
    snapshot(lock_inner().call.as_ref())
}

pub fn shutdown() {
    let mut guard = lock_inner();
    clear_to_idle(&mut guard, true, true);
}

pub fn on_chat_session_dead(chat_session_id: u32) {
    let should = lock_inner()
        .call
        .as_ref()
        .map(|c| c.chat_session_id == chat_session_id)
        .unwrap_or(false);
    if should {
        let mut guard = lock_inner();
        let is_inviter = guard.call.as_ref().map(|c| c.is_inviter).unwrap_or(false);
        let media_id = guard.call.as_ref().map(|c| c.media_session_id).unwrap_or(0);
        clear_to_idle(&mut guard, true, false);
        if is_inviter && media_id > 0 {
            webrpc::close_videocall_webrpc_session(media_id);
        }
    }
}

pub fn on_media_session_dead(media_session_id: u32) {
    let should = lock_inner()
        .call
        .as_ref()
        .map(|c| c.media_session_id == media_session_id)
        .unwrap_or(false);
    if should {
        let mut guard = lock_inner();
        clear_to_idle(&mut guard, true, false);
    }
}

pub fn on_session_signal(chat_session_id: u32, data: serde_json::Value) {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Payload {
        op: String,
        #[serde(default)]
        call_id: String,
        #[serde(default)]
        session_id: u32,
    }
    let parsed: Payload = match serde_json::from_value(data) {
        Ok(v) => v,
        Err(_) => return,
    };
    let op = parsed.op.trim();
    let call_id = parsed.call_id.trim();
    if call_id.is_empty() {
        return;
    }
    match op {
        "start" => on_session_start(chat_session_id, call_id, parsed.session_id),
        "stop" => on_session_stop(chat_session_id, call_id),
        _ => {}
    }
}

pub fn on_signal(session_id: u32, data: serde_json::Value) {
    let parsed: InboundSignal = match serde_json::from_value(data) {
        Ok(v) => v,
        Err(_) => return,
    };
    let op = parsed.op.trim();
    if !op.starts_with("vcall_") {
        return;
    }
    let call_id = parsed.call_id.trim();
    match op {
        "vcall_invite" => on_invite(session_id, call_id),
        "vcall_accept" => on_accept(session_id, call_id),
        "vcall_reject" | "vcall_cancel" | "vcall_hangup" | "vcall_busy" | "vcall_timeout" => {
            on_end(session_id, call_id)
        }
        _ => {}
    }
}

pub fn on_audio_binary(session_id: u32, payload: &[u8]) {
    let Some((call_id, seq, _ts, opus)) = parse_audio_binary(payload) else {
        return;
    };
    let ok = {
        let guard = lock_inner();
        guard.call.as_ref().map(|c| {
            c.phase == VCallPhase::Active
                && c.media_session_id == session_id
                && c.media_session_id > 0
                && (call_id.is_empty() || call_id == c.call_id)
        }).unwrap_or(false)
    };
    if !ok {
        return;
    }
    let recv_cell = audio_recv_tx();
    let tx_slot = recv_cell.lock().unwrap_or_else(|e| e.into_inner());
    let Some(tx) = tx_slot.as_ref() else {
        return;
    };
    let _ = tx.try_send((seq, opus));
}

pub fn on_video_binary(session_id: u32, payload: &[u8]) {
    let Some((call_id, seq, h264)) = parse_video_binary(payload) else {
        return;
    };
    let ok = {
        let guard = lock_inner();
        guard.call.as_ref().map(|c| {
            c.phase == VCallPhase::Active
                && c.media_session_id == session_id
                && (call_id.is_empty() || call_id == c.call_id)
        }).unwrap_or(false)
    };
    if !ok || h264.is_empty() {
        return;
    }
    let recv_cell = video_recv_tx();
    let tx_slot = recv_cell.lock().unwrap_or_else(|e| e.into_inner());
    let Some(tx) = tx_slot.as_ref() else {
        return;
    };
    let _ = tx.try_send((seq, h264.to_vec()));
}

fn on_invite(session_id: u32, call_id: &str) {
    if call_id.is_empty() {
        return;
    }
    if crate::voice::is_busy() {
        send_signal(session_id, "vcall_busy", call_id);
        return;
    }
    let peer = webrpc::peer_token_of(session_id);
    let mut guard = lock_inner();
    if let Some(call) = guard.call.as_ref() {
        if call.chat_session_id == session_id && call.call_id == call_id {
            return;
        }
        drop(guard);
        send_signal(session_id, "vcall_busy", call_id);
        return;
    }
    guard.call = Some(Call {
        phase: VCallPhase::Incoming,
        chat_session_id: session_id,
        media_session_id: 0,
        call_id: call_id.to_string(),
        peer_token: peer,
        peer_pass: String::new(),
        is_inviter: false,
        muted: false,
        has_camera: has_local_camera(),
        started_at: Instant::now(),
        started_unix_ms: now_ms(),
    });
    emit_state(snapshot(guard.call.as_ref()));
    drop(guard);
    spawn_ring_timeout(session_id, call_id.to_string(), VCallPhase::Incoming);
}

fn on_accept(session_id: u32, call_id: &str) {
    let call_id = {
        let mut guard = lock_inner();
        let Some(call) = guard.call.as_mut() else {
            return;
        };
        if call.phase != VCallPhase::Outgoing
            || call.chat_session_id != session_id
            || !call.is_inviter
            || (!call_id.is_empty() && call.call_id != call_id)
        {
            return;
        }
        call.phase = VCallPhase::Active;
        call.started_at = Instant::now();
        call.started_unix_ms = now_ms();
        call.call_id.clone()
    };
    let hangup_id = call_id.clone();
    if thread::Builder::new()
        .name("vcall-open".into())
        .spawn(move || inviter_begin_media(session_id, call_id))
        .is_err()
    {
        emit_error("视频会话创建失败");
        send_signal(session_id, "vcall_hangup", &hangup_id);
        let mut guard = lock_inner();
        clear_to_idle(&mut guard, true, false);
    }
}

fn inviter_begin_media(session_id: u32, call_id: String) {
    let peer_pass = lock_inner()
        .call
        .as_ref()
        .filter(|c| c.is_inviter && c.chat_session_id == session_id && c.call_id == call_id)
        .map(|c| c.peer_pass.clone())
        .unwrap_or_default();

    let media_id = match webrpc::open_inviter_videocall_session(session_id, Some(&peer_pass)) {
        Ok(id) => id,
        Err(err) => {
            eprintln!("vcall: open session failed: {err}");
            emit_error("视频会话创建失败");
            send_signal(session_id, "vcall_hangup", &call_id);
            let mut guard = lock_inner();
            clear_to_idle(&mut guard, true, false);
            return;
        }
    };

    {
        let mut guard = lock_inner();
        let Some(call) = guard.call.as_mut() else {
            webrpc::close_videocall_webrpc_session(media_id);
            return;
        };
        if !call.is_inviter || call.chat_session_id != session_id || call.call_id != call_id {
            webrpc::close_videocall_webrpc_session(media_id);
            return;
        }
        call.media_session_id = media_id;
    }

    send_session_signal(session_id, "start", &call_id, media_id);

    if let Err(err) = start_media(media_id, call_id.clone()) {
        eprintln!("vcall: start media failed: {err}");
        emit_error(&err);
        send_session_signal(session_id, "stop", &call_id, media_id);
        send_signal(session_id, "vcall_hangup", &call_id);
        webrpc::close_videocall_webrpc_session(media_id);
        let mut guard = lock_inner();
        clear_to_idle(&mut guard, true, false);
        return;
    }
    emit_state(snapshot(lock_inner().call.as_ref()));
}

fn on_session_start(chat_session_id: u32, call_id: &str, media_session_id: u32) {
    if media_session_id == 0 {
        return;
    }
    let call_id_owned = {
        let mut guard = lock_inner();
        let already = guard.call.as_ref().map(|c| {
            c.chat_session_id == chat_session_id
                && c.call_id == call_id
                && !c.is_inviter
                && c.media_session_id == media_session_id
        }).unwrap_or(false);
        if already && guard.media.is_some() {
            return;
        }
        let Some(call) = guard.call.as_mut() else {
            return;
        };
        if call.chat_session_id != chat_session_id || call.call_id != call_id || call.is_inviter {
            return;
        }
        call.media_session_id = media_session_id;
        call.phase = VCallPhase::Active;
        call.started_at = Instant::now();
        call.started_unix_ms = now_ms();
        call.call_id.clone()
    };

    webrpc::mark_videocall_session(media_session_id, chat_session_id);

    if let Err(err) = start_media(media_session_id, call_id_owned) {
        eprintln!("vcall: callee start media failed: {err}");
        emit_error(&err);
        send_session_signal(chat_session_id, "stop", call_id, media_session_id);
        send_signal(chat_session_id, "vcall_hangup", call_id);
        let mut guard = lock_inner();
        clear_to_idle(&mut guard, true, false);
    } else {
        emit_state(snapshot(lock_inner().call.as_ref()));
    }
}

fn on_session_stop(chat_session_id: u32, call_id: &str) {
    let mut guard = lock_inner();
    let Some(call) = guard.call.as_ref() else {
        return;
    };
    if call.chat_session_id != chat_session_id || call.call_id != call_id {
        return;
    }
    clear_to_idle(&mut guard, true, true);
}

fn on_end(session_id: u32, call_id: &str) {
    let mut guard = lock_inner();
    let Some(call) = guard.call.as_ref() else {
        return;
    };
    if call.chat_session_id != session_id {
        return;
    }
    if !call_id.is_empty() && call.call_id != call_id {
        return;
    }
    clear_to_idle(&mut guard, true, true);
}

fn spawn_ring_timeout(session_id: u32, call_id: String, wait: VCallPhase) {
    thread::Builder::new()
        .name("vcall-ring".into())
        .spawn(move || {
            thread::sleep(Duration::from_millis(RING_TIMEOUT_MS));
            let (op, sid, cid, media_id, is_inviter) = {
                let mut guard = lock_inner();
                let Some(call) = guard.call.as_ref() else {
                    return;
                };
                if call.chat_session_id != session_id
                    || call.call_id != call_id
                    || call.phase != wait
                {
                    return;
                }
                let op = if wait == VCallPhase::Outgoing {
                    "vcall_timeout"
                } else {
                    "vcall_reject"
                };
                let sid = call.chat_session_id;
                let cid = call.call_id.clone();
                let media_id = call.media_session_id;
                let is_inviter = call.is_inviter;
                clear_to_idle(&mut guard, true, false);
                if is_inviter && media_id > 0 {
                    webrpc::close_videocall_webrpc_session(media_id);
                }
                (op.to_string(), sid, cid, media_id, is_inviter)
            };
            let _ = (media_id, is_inviter);
            send_signal(sid, &op, &cid);
        })
        .ok();
}

fn start_media(media_session_id: u32, call_id: String) -> Result<(), String> {
    reset_buffers();
    let has_camera = lock_inner()
        .call
        .as_ref()
        .map(|c| c.has_camera)
        .unwrap_or(false);

    let host = cpal::default_host();
    let input_dev = host
        .default_input_device()
        .ok_or_else(|| "未找到麦克风。".to_string())?;
    let output_dev = host
        .default_output_device()
        .ok_or_else(|| "未找到扬声器。".to_string())?;
    let in_cfg = input_dev
        .default_input_config()
        .map_err(|e| format!("无法打开麦克风: {e}"))?;
    let out_cfg = output_dev
        .default_output_config()
        .map_err(|e| format!("无法打开扬声器: {e}"))?;

    let stop = Arc::new(AtomicBool::new(false));
    mute_flag().store(false, Ordering::Relaxed);

    let (audio_tx, audio_rx) = mpsc::sync_channel::<(u32, Vec<u8>)>(RECV_OPUS_QUEUE);
    *audio_recv_tx().lock().unwrap_or_else(|e| e.into_inner()) = Some(audio_tx);

    let (video_tx, video_rx) = mpsc::sync_channel::<(u32, Vec<u8>)>(8);
    *video_recv_tx().lock().unwrap_or_else(|e| e.into_inner()) = Some(video_tx);

    let play_buf = play_pcm_buf();
    let input = build_input_stream(&input_dev, &in_cfg, stop.clone())?;
    let output = build_output_stream(&output_dev, &out_cfg, play_buf, stop.clone())?;
    input.play().map_err(|e| format!("麦克风启动失败: {e}"))?;
    output.play().map_err(|e| format!("扬声器启动失败: {e}"))?;

    let audio_recv = thread::Builder::new()
        .name("vcall-audio-recv".into())
        .spawn({
            let stop = stop.clone();
            move || run_audio_recv_loop(audio_rx, stop)
        })
        .map_err(|e| format!("音频接收线程失败: {e}"))?;

    let video_recv = thread::Builder::new()
        .name("vcall-video-recv".into())
        .spawn({
            let stop = stop.clone();
            move || run_video_recv_loop(video_rx, stop)
        })
        .map_err(|e| format!("视频接收线程失败: {e}"))?;

    let audio_seq = Arc::new(AtomicU32::new(1));
    let video_seq = Arc::new(AtomicU32::new(1));
    let stream_start = now_ms();
    let budget = send_budget();

    let call_id_audio = call_id.clone();
    let call_id_video = call_id.clone();
    let audio_send = thread::Builder::new()
        .name("vcall-audio-send".into())
        .spawn({
            let stop = stop.clone();
            let budget = budget.clone();
            move || run_audio_send_loop(media_session_id, call_id_audio, audio_seq, stream_start, budget, stop)
        })
        .map_err(|e| format!("音频发送线程失败: {e}"))?;

    let video_send = if has_camera {
        Some(
            thread::Builder::new()
                .name("vcall-video-send".into())
                .spawn({
                    let stop = stop.clone();
                    let budget = budget.clone();
                    move || run_video_send_loop(media_session_id, call_id_video, video_seq, budget, stop)
                })
                .map_err(|e| format!("视频发送线程失败: {e}"))?,
        )
    } else {
        None
    };

    let mut guard = lock_inner();
    stop_media(&guard.media);
    guard.media = Some(LiveMedia {
        stop,
        _input: SendStream(input),
        _output: SendStream(output),
        _audio_recv: audio_recv,
        _audio_send: audio_send,
        _video_send: video_send,
        _video_recv: video_recv,
    });
    if let Some(call) = guard.call.as_mut() {
        call.muted = false;
        mute_flag().store(false, Ordering::Relaxed);
    }
    Ok(())
}

fn run_audio_recv_loop(rx: mpsc::Receiver<(u32, Vec<u8>)>, stop: Arc<AtomicBool>) {
    let queue = opus_queue();
    let play = play_pcm_buf();
    let Ok(mut decoder) = Decoder::new(SAMPLE_RATE, Channels::Mono) else {
        return;
    };
    while !stop.load(Ordering::SeqCst) {
        let mut got = false;
        while let Ok((seq, opus)) = rx.try_recv() {
            queue.lock().unwrap_or_else(|e| e.into_inner()).insert(seq, opus);
            got = true;
        }
        if let Some(packet) = queue.lock().unwrap_or_else(|e| e.into_inner()).pop_in_order() {
            let pcm = decode_opus(&mut decoder, &packet);
            play.lock().unwrap_or_else(|e| e.into_inner()).extend(pcm);
            got = true;
        }
        if !got {
            thread::sleep(Duration::from_millis(2));
        }
    }
}

fn run_video_recv_loop(rx: mpsc::Receiver<(u32, Vec<u8>)>, stop: Arc<AtomicBool>) {
    let mut decoder = match H264Decoder::new() {
        Ok(v) => v,
        Err(err) => {
            eprintln!("vcall: h264 decoder init failed: {err}");
            return;
        }
    };
    while !stop.load(Ordering::SeqCst) {
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok((_seq, h264)) => present_remote_h264(&mut decoder, &h264),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn present_remote_h264(decoder: &mut H264Decoder, bytes: &[u8]) {
    let decoded = match decoder.decode(bytes) {
        Ok(Some(yuv)) => {
            let (w, h) = yuv.dimensions();
            let mut rgb = vec![0u8; yuv.rgb8_len()];
            yuv.write_rgb8(&mut rgb);
            Some((w as u32, h as u32, rgb))
        }
        Ok(None) => None,
        Err(err) => {
            eprintln!("vcall: decode {err}");
            None
        }
    };
    let Some((width, height, rgb)) = decoded else {
        return;
    };
    let Ok(jpeg) = jpeg_from_rgb(width, height, &rgb) else {
        return;
    };
    if let Some(app) = webrpc::app_handle() {
        let _ = app.emit(
            "webrpc-vcall-remote-frame",
            VCallFrameEvent {
                width,
                height,
                jpeg: B64.encode(&jpeg),
            },
        );
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct VCallFrameEvent {
    width: u32,
    height: u32,
    jpeg: String,
}

fn run_audio_send_loop(
    session_id: u32,
    call_id: String,
    seq: Arc<AtomicU32>,
    stream_start: u64,
    budget: Arc<Mutex<SendBudget>>,
    stop: Arc<AtomicBool>,
) {
    let send_buf = send_pcm_buf();
    let mut encoder = match OpusEncoder::builder(SAMPLE_RATE, Channels::Mono, Application::Voip)
        .bitrate(Bitrate::Bits(OPUS_BITRATE))
        .build()
    {
        Ok(v) => v,
        Err(_) => return,
    };
    let mut opus_out = vec![0u8; OPUS_PACKET_MAX];
    while !stop.load(Ordering::SeqCst) {
        let frame = send_buf.lock().unwrap_or_else(|e| e.into_inner()).pop_front();
        let Some(frame) = frame else {
            thread::sleep(Duration::from_millis(2));
            continue;
        };
        if frame.len() != FRAME_SAMPLES {
            continue;
        }
        let Ok(opus_len) = encoder.encode(&frame, &mut opus_out) else {
            continue;
        };
        let n = seq.fetch_add(1, Ordering::Relaxed);
        let ts = now_ms().saturating_sub(stream_start) as u32;
        let Some(payload) = pack_audio_binary(&call_id, n, ts, &opus_out[..opus_len]) else {
            continue;
        };
        if budget.lock().unwrap_or_else(|e| e.into_inner()).charge(payload.len()) {
            let _ = webrpc::send_bytes_timeout(session_id, &payload, SEND_TIMEOUT_MS);
        }
    }
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn run_video_send_loop(
    session_id: u32,
    call_id: String,
    seq: Arc<AtomicU32>,
    budget: Arc<Mutex<SendBudget>>,
    stop: Arc<AtomicBool>,
) {
    let mut camera = match CameraCapture::open_default(MAX_EDGE) {
        Ok(v) => v,
        Err(err) => {
            eprintln!("vcall: camera unavailable: {err}");
            return;
        }
    };
    let mut encoder_slot: Option<Encoder> = None;
    let mut enc_size = (0u32, 0u32);
    let mut next_frame = Instant::now();
    while !stop.load(Ordering::SeqCst) {
        if Instant::now() < next_frame {
            thread::sleep(Duration::from_millis(5));
            continue;
        }
        next_frame = Instant::now() + FRAME_INTERVAL;
        let img = match camera.capture_rgba() {
            Ok(v) => v,
            Err(err) => {
                eprintln!("vcall: capture {err}");
                thread::sleep(Duration::from_millis(80));
                continue;
            }
        };
        let w = img.width();
        let h = img.height();
        let encoder = match encoder_for(&mut encoder_slot, &mut enc_size, w, h) {
            Ok(v) => v,
            Err(err) => {
                eprintln!("vcall: encoder {err}");
                continue;
            }
        };
        let (bytes, key) = match encode_h264(encoder, &img) {
            Ok(v) => v,
            Err(err) => {
                eprintln!("vcall: encode {err}");
                continue;
            }
        };
        let n = seq.fetch_add(1, Ordering::Relaxed);
        let Some(payload) = pack_video_binary(&call_id, n, w, h, key, &bytes) else {
            continue;
        };
        if !budget.lock().unwrap_or_else(|e| e.into_inner()).charge(payload.len()) {
            continue;
        }
        let _ = webrpc::send_bytes_timeout(session_id, &payload, SEND_TIMEOUT_MS);
        if let Some(app) = webrpc::app_handle() {
            if let Ok(jpeg) = jpeg_from_rgba(&img) {
                let _ = app.emit(
                    "webrpc-vcall-local-frame",
                    VCallFrameEvent {
                        width: w,
                        height: h,
                        jpeg: B64.encode(&jpeg),
                    },
                );
            }
        }
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn run_video_send_loop(
    _session_id: u32,
    _call_id: String,
    _seq: Arc<AtomicU32>,
    _budget: Arc<Mutex<SendBudget>>,
    _stop: Arc<AtomicBool>,
) {
}

fn encoder_for<'a>(
    slot: &'a mut Option<Encoder>,
    size: &mut (u32, u32),
    width: u32,
    height: u32,
) -> Result<&'a mut Encoder, String> {
    if slot.is_none() || *size != (width, height) {
        let config = EncoderConfig::new()
            .max_frame_rate(FrameRate::from_hz(TARGET_FPS as f32))
            .bitrate(BitRate::from_bps(VIDEO_BITRATE_BPS))
            .usage_type(UsageType::ScreenContentRealTime)
            .complexity(Complexity::Low)
            .skip_frames(true)
            .intra_frame_period(IntraFramePeriod::from_num_frames(KEY_EVERY));
        *slot = Some(
            Encoder::with_api_config(openh264::OpenH264API::from_source(), config)
                .map_err(|e| format!("H.264 编码器失败: {e}"))?,
        );
        *size = (width, height);
    }
    slot.as_mut().ok_or_else(|| "编码器未就绪".into())
}

fn encode_h264(encoder: &mut Encoder, img: &RgbaImage) -> Result<(Vec<u8>, bool), String> {
    let rgb = DynamicImage::ImageRgba8(img.clone()).to_rgb8();
    let src = RgbSliceU8::new(rgb.as_raw(), (rgb.width() as usize, rgb.height() as usize));
    let yuv = YUVBuffer::from_rgb_source(src);
    let bitstream = encoder
        .encode(&yuv)
        .map_err(|e| format!("H.264 编码失败: {e}"))?;
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

fn pack_audio_binary(call_id: &str, seq: u32, ts: u32, opus: &[u8]) -> Option<Vec<u8>> {
    let call_bytes = call_id.as_bytes();
    if call_bytes.len() > 255 || opus.len() > u16::MAX as usize {
        return None;
    }
    let mut payload = Vec::with_capacity(15 + call_bytes.len() + opus.len());
    payload.extend_from_slice(&[0, 0, 0, AUDIO_BIN_VER]);
    payload.push(call_bytes.len() as u8);
    payload.extend_from_slice(call_bytes);
    payload.extend_from_slice(&seq.to_le_bytes());
    payload.extend_from_slice(&ts.to_le_bytes());
    payload.extend_from_slice(&(opus.len() as u16).to_le_bytes());
    payload.extend_from_slice(opus);
    Some(payload)
}

fn parse_audio_binary(payload: &[u8]) -> Option<(String, u32, u32, Vec<u8>)> {
    if payload.len() < 15 || payload[0] != 0 || payload[1] != 0 || payload[2] != 0 || payload[3] != AUDIO_BIN_VER {
        return None;
    }
    let call_len = payload[4] as usize;
    let header = 5 + call_len + 4 + 4 + 2;
    if payload.len() < header {
        return None;
    }
    let call_id = std::str::from_utf8(&payload[5..5 + call_len]).ok()?.to_string();
    let mut off = 5 + call_len;
    let seq = u32::from_le_bytes(payload[off..off + 4].try_into().ok()?);
    off += 4;
    let ts = u32::from_le_bytes(payload[off..off + 4].try_into().ok()?);
    off += 4;
    let opus_len = u16::from_le_bytes(payload[off..off + 2].try_into().ok()?) as usize;
    off += 2;
    if payload.len() < off + opus_len {
        return None;
    }
    Some((call_id, seq, ts, payload[off..off + opus_len].to_vec()))
}

fn pack_video_binary(
    call_id: &str,
    seq: u32,
    width: u32,
    height: u32,
    key: bool,
    h264: &[u8],
) -> Option<Vec<u8>> {
    let call_bytes = call_id.as_bytes();
    if call_bytes.len() > 255 || h264.len() > u16::MAX as usize || width > u16::MAX as u32 || height > u16::MAX as u32 {
        return None;
    }
    let mut payload = Vec::with_capacity(20 + call_bytes.len() + h264.len());
    payload.extend_from_slice(&[0, 0, 0, VIDEO_BIN_VER]);
    payload.push(call_bytes.len() as u8);
    payload.extend_from_slice(call_bytes);
    payload.extend_from_slice(&seq.to_le_bytes());
    payload.extend_from_slice(&(width as u16).to_le_bytes());
    payload.extend_from_slice(&(height as u16).to_le_bytes());
    payload.push(if key { 1 } else { 0 });
    payload.extend_from_slice(&(h264.len() as u16).to_le_bytes());
    payload.extend_from_slice(h264);
    Some(payload)
}

fn parse_video_binary(payload: &[u8]) -> Option<(String, u32, &[u8])> {
    if payload.len() < 18 || payload[0] != 0 || payload[1] != 0 || payload[2] != 0 || payload[3] != VIDEO_BIN_VER {
        return None;
    }
    let call_len = payload[4] as usize;
    let header = 5 + call_len + 4 + 2 + 2 + 1 + 2;
    if payload.len() < header {
        return None;
    }
    let call_id = std::str::from_utf8(&payload[5..5 + call_len]).ok()?.to_string();
    let mut off = 5 + call_len;
    let seq = u32::from_le_bytes(payload[off..off + 4].try_into().ok()?);
    off += 4 + 2 + 2 + 1;
    let h264_len = u16::from_le_bytes(payload[off..off + 2].try_into().ok()?) as usize;
    off += 2;
    if payload.len() < off + h264_len {
        return None;
    }
    Some((call_id, seq, &payload[off..off + h264_len]))
}

fn decode_opus(decoder: &mut Decoder, opus: &[u8]) -> Vec<i16> {
    let mut buf = vec![0i16; OPUS_DECODE_MAX_SAMPLES];
    let Ok(samples) = decoder.decode(opus, &mut buf, DecodeMode::Normal) else {
        return vec![0i16; FRAME_SAMPLES];
    };
    if samples == 0 {
        return vec![0i16; FRAME_SAMPLES];
    }
    let mut out = buf[..samples.min(FRAME_SAMPLES)].to_vec();
    out.resize(FRAME_SAMPLES, 0);
    out
}

fn jpeg_from_rgb(width: u32, height: u32, rgb: &[u8]) -> Result<Vec<u8>, String> {
    use image::codecs::jpeg::JpegEncoder;
    let mut out = Vec::new();
    let mut enc = JpegEncoder::new_with_quality(&mut out, 75);
    enc.encode(rgb, width, height, image::ExtendedColorType::Rgb8)
        .map_err(|e| format!("JPEG 编码失败: {e}"))?;
    Ok(out)
}

fn jpeg_from_rgba(img: &RgbaImage) -> Result<Vec<u8>, String> {
    let rgb = DynamicImage::ImageRgba8(img.clone()).to_rgb8();
    jpeg_from_rgb(rgb.width(), rgb.height(), rgb.as_raw())
}

fn mute_flag() -> Arc<AtomicBool> {
    static CELL: OnceLock<Arc<AtomicBool>> = OnceLock::new();
    CELL.get_or_init(|| Arc::new(AtomicBool::new(false))).clone()
}

fn push_capture_frames(samples: &[i16]) {
    if samples.is_empty() || mute_flag().load(Ordering::Relaxed) {
        return;
    }
    let accum = capture_accum();
    let mut slot = accum.lock().unwrap_or_else(|e| e.into_inner());
    slot.extend_from_slice(samples);
    let send = send_pcm_buf();
    while slot.len() >= FRAME_SAMPLES {
        let frame: Vec<i16> = slot.drain(..FRAME_SAMPLES).collect();
        send.lock().unwrap_or_else(|e| e.into_inner()).push_back(frame);
    }
}

fn build_input_stream(
    device: &cpal::Device,
    config: &cpal::SupportedStreamConfig,
    stop: Arc<AtomicBool>,
) -> Result<Stream, String> {
    let rate = config.sample_rate().0;
    let channels = config.channels().max(1);
    let err_fn = |err| eprintln!("vcall input: {err}");
    let stream_config = config.config();
    match config.sample_format() {
        SampleFormat::F32 => device.build_input_stream(
            &stream_config,
            move |data: &[f32], _| capture_cb(data, rate, channels, &stop),
            err_fn,
            None,
        ),
        SampleFormat::I16 => device.build_input_stream(
            &stream_config,
            move |data: &[i16], _| capture_cb(data, rate, channels, &stop),
            err_fn,
            None,
        ),
        other => return Err(format!("麦克风格式不支持: {other}")),
    }
    .map_err(|e| format!("打开麦克风失败: {e}"))
}

fn capture_cb<T: Sample + FromSample<f32>>(data: &[T], rate: u32, channels: u16, stop: &AtomicBool)
where
    i16: FromSample<T>,
{
    if stop.load(Ordering::Relaxed) {
        return;
    }
    let ch = channels as usize;
    if ch == 0 || data.is_empty() {
        return;
    }
    let mut mono = Vec::with_capacity(data.len() / ch + 1);
    let mut i = 0;
    while i + ch <= data.len() {
        let mut acc = 0i32;
        for c in 0..ch {
            acc += i16::from_sample(data[i + c]) as i32;
        }
        mono.push((acc / ch as i32) as i16);
        i += ch;
    }
    push_capture_frames(&resample_mono_i16(&mono, rate, SAMPLE_RATE));
}

fn build_output_stream(
    device: &cpal::Device,
    config: &cpal::SupportedStreamConfig,
    play_buf: Arc<Mutex<VecDeque<i16>>>,
    stop: Arc<AtomicBool>,
) -> Result<Stream, String> {
    let rate = config.sample_rate().0;
    let channels = config.channels().max(1);
    let err_fn = |err| eprintln!("vcall output: {err}");
    let stream_config = config.config();
    match config.sample_format() {
        SampleFormat::F32 => device.build_output_stream(
            &stream_config,
            move |data: &mut [f32], _| render_cb(data, rate, channels, &play_buf, &stop),
            err_fn,
            None,
        ),
        SampleFormat::I16 => device.build_output_stream(
            &stream_config,
            move |data: &mut [i16], _| render_cb(data, rate, channels, &play_buf, &stop),
            err_fn,
            None,
        ),
        other => return Err(format!("扬声器格式不支持: {other}")),
    }
    .map_err(|e| format!("打开扬声器失败: {e}"))
}

fn render_cb<T: Sample + FromSample<i16>>(
    data: &mut [T],
    rate: u32,
    channels: u16,
    play_buf: &Mutex<VecDeque<i16>>,
    stop: &AtomicBool,
) {
    if stop.load(Ordering::Relaxed) {
        for s in data.iter_mut() {
            *s = T::from_sample(0i16);
        }
        return;
    }
    let ch = channels as usize;
    let frames = data.len() / ch.max(1);
    let need = ((frames as u64) * SAMPLE_RATE as u64 / rate.max(1) as u64) as usize + 1;
    let mut src: Vec<i16> = Vec::with_capacity(need);
    {
        let mut buf = play_buf.lock().unwrap_or_else(|e| e.into_inner());
        for _ in 0..need {
            src.push(buf.pop_front().unwrap_or(0));
        }
        while buf.len() > MAX_PLAY_SAMPLES {
            buf.pop_front();
        }
    }
    let pcm = if rate == SAMPLE_RATE {
        src
    } else {
        resample_mono_i16(&src, SAMPLE_RATE, rate)
    };
    for (i, frame) in pcm.iter().take(frames).enumerate() {
        let sample = T::from_sample(*frame);
        for c in 0..ch {
            let idx = i * ch + c;
            if idx < data.len() {
                data[idx] = sample;
            }
        }
    }
}

fn resample_mono_i16(input: &[i16], from: u32, to: u32) -> Vec<i16> {
    if input.is_empty() || from == 0 || to == 0 || from == to {
        return input.to_vec();
    }
    let out_len = ((input.len() as u64) * to as u64 / from as u64).max(1) as usize;
    let mut out = Vec::with_capacity(out_len);
    let last = input.len() - 1;
    for i in 0..out_len {
        let src = i as f64 * from as f64 / to as f64;
        let i0 = (src.floor() as usize).min(last);
        let i1 = (i0 + 1).min(last);
        let frac = src - i0 as f64;
        let s = input[i0] as f64 * (1.0 - frac) + input[i1] as f64 * frac;
        out.push(s.round().clamp(-32768.0, 32767.0) as i16);
    }
    out
}
