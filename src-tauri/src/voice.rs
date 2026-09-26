use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, Stream};
use ropus::{Application, Bitrate, Channels, DecodeMode, Decoder, Encoder};
use serde::{Deserialize, Serialize};
use tauri::Emitter;

use crate::webrpc;

const SAMPLE_RATE: u32 = 16_000;
const FRAME_MS: u32 = 20;
const FRAME_SAMPLES: usize = (SAMPLE_RATE * FRAME_MS / 1000) as usize;
const RING_TIMEOUT_MS: u64 = 45_000;
const VOICE_SEND_TIMEOUT_MS: i64 = 0;
const VOICE_BIN_VER: u8 = 2;
const OPUS_PACKET_MAX: usize = 4000;
const OPUS_DECODE_MAX_SAMPLES: usize = (SAMPLE_RATE as usize * 120) / 1000;
const OPUS_BITRATE: u32 = 32_000;

/// 发送/接收 PCM 环形队列最多保留 2 秒
const MAX_BUFFER_SEC: u32 = 2;
const MAX_SEND_FRAMES: usize = (MAX_BUFFER_SEC * 1000 / FRAME_MS) as usize;
const MAX_PLAY_SAMPLES: usize = (SAMPLE_RATE * MAX_BUFFER_SEC) as usize;
const RECV_OPUS_QUEUE: usize = MAX_SEND_FRAMES;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum VoicePhase {
    Idle,
    Outgoing,
    Incoming,
    Active,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceUiState {
    pub phase: VoicePhase,
    pub session_id: u32,
    pub call_id: String,
    pub peer_token: String,
    pub muted: bool,
    pub started_at: u64,
}

impl VoiceUiState {
    fn idle() -> Self {
        Self {
            phase: VoicePhase::Idle,
            session_id: 0,
            call_id: String::new(),
            peer_token: String::new(),
            muted: false,
            started_at: 0,
        }
    }
}

struct Call {
    phase: VoicePhase,
    chat_session_id: u32,
    voice_session_id: u32,
    call_id: String,
    peer_token: String,
    peer_pass: String,
    is_inviter: bool,
    muted: bool,
    started_at: Instant,
    started_unix_ms: u64,
}

#[allow(dead_code)]
struct SendStream(Stream);
unsafe impl Send for SendStream {}

struct LiveAudio {
    stop: Arc<AtomicBool>,
    _input: SendStream,
    _output: SendStream,
    _recv: thread::JoinHandle<()>,
    _send: thread::JoinHandle<()>,
}

struct VoiceInner {
    call: Option<Call>,
    audio: Option<LiveAudio>,
}

/// 发送侧：采集线程只写入，发送线程只读出。
struct SendPcmBuffer {
    frames: VecDeque<Vec<i16>>,
}

impl SendPcmBuffer {
    fn push(&mut self, frame: Vec<i16>) {
        self.frames.push_back(frame);
        while self.frames.len() > MAX_SEND_FRAMES {
            self.frames.pop_front();
        }
    }

    fn pop(&mut self) -> Option<Vec<i16>> {
        self.frames.pop_front()
    }

    fn clear(&mut self) {
        self.frames.clear();
    }
}

/// 播放侧：接收/解压线程只写入，声卡回调只读出。
struct PlayPcmBuffer {
    samples: VecDeque<i16>,
}

impl PlayPcmBuffer {
    fn push_frame(&mut self, frame: &[i16]) {
        self.samples.extend(frame);
        while self.samples.len() > MAX_PLAY_SAMPLES {
            self.samples.pop_front();
        }
    }

    fn take(&mut self, count: usize) -> Vec<i16> {
        let mut out = Vec::with_capacity(count);
        for _ in 0..count {
            match self.samples.pop_front() {
                Some(v) => out.push(v),
                None => out.push(0),
            }
        }
        out
    }

    fn clear(&mut self) {
        self.samples.clear();
    }
}

/// 接收侧 Opus 包按 seq 排队，解压线程按序取包。
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
                if self.next_seq.is_some_and(|next| oldest >= next) {
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

fn inner() -> &'static Mutex<VoiceInner> {
    static CELL: OnceLock<Mutex<VoiceInner>> = OnceLock::new();
    CELL.get_or_init(|| {
        Mutex::new(VoiceInner {
            call: None,
            audio: None,
        })
    })
}

fn send_pcm_buf() -> Arc<Mutex<SendPcmBuffer>> {
    static CELL: OnceLock<Arc<Mutex<SendPcmBuffer>>> = OnceLock::new();
    CELL.get_or_init(|| {
        Arc::new(Mutex::new(SendPcmBuffer {
            frames: VecDeque::new(),
        }))
    })
    .clone()
}

fn play_pcm_buf() -> Arc<Mutex<PlayPcmBuffer>> {
    static CELL: OnceLock<Arc<Mutex<PlayPcmBuffer>>> = OnceLock::new();
    CELL.get_or_init(|| {
        Arc::new(Mutex::new(PlayPcmBuffer {
            samples: VecDeque::new(),
        }))
    })
    .clone()
}

fn opus_queue() -> Arc<Mutex<OpusQueue>> {
    static CELL: OnceLock<Arc<Mutex<OpusQueue>>> = OnceLock::new();
    CELL.get_or_init(|| Arc::new(Mutex::new(OpusQueue::new())))
        .clone()
}

fn capture_accum() -> Arc<Mutex<Vec<i16>>> {
    static CELL: OnceLock<Arc<Mutex<Vec<i16>>>> = OnceLock::new();
    CELL.get_or_init(|| Arc::new(Mutex::new(Vec::new())))
        .clone()
}

fn recv_tx() -> Arc<Mutex<Option<SyncSender<(u32, Vec<u8>)>>>> {
    static CELL: OnceLock<Arc<Mutex<Option<SyncSender<(u32, Vec<u8>)>>>>> = OnceLock::new();
    CELL.get_or_init(|| Arc::new(Mutex::new(None)))
        .clone()
}

fn lock_inner() -> std::sync::MutexGuard<'static, VoiceInner> {
    inner().lock().unwrap_or_else(|err| err.into_inner())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|item| item.as_millis() as u64)
        .unwrap_or(0)
}

fn emit_voice_error(msg: &str) {
    if let Some(app) = webrpc::app_handle() {
        let _ = app.emit("webrpc-voice-error", msg);
    }
}

fn emit_state(state: VoiceUiState) {
    if let Some(app) = webrpc::app_handle() {
        let _ = app.emit("webrpc-voice-state", state);
    }
}

fn snapshot(call: Option<&Call>) -> VoiceUiState {
    let Some(call) = call else {
        return VoiceUiState::idle();
    };
    VoiceUiState {
        phase: call.phase,
        session_id: call.chat_session_id,
        call_id: call.call_id.clone(),
        peer_token: call.peer_token.clone(),
        muted: call.muted,
        started_at: call.started_unix_ms,
    }
}

fn reset_all_buffers() {
    send_pcm_buf()
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .clear();
    play_pcm_buf()
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .clear();
    opus_queue()
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .clear();
    capture_accum()
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .clear();
}

fn stop_audio(audio: &Option<LiveAudio>) {
    if let Some(live) = audio {
        live.stop.store(true, Ordering::SeqCst);
    }
}

fn clear_to_idle(guard: &mut VoiceInner, emit: bool, close_voice_webrpc: bool) {
    let (voice_id, is_inviter) = guard
        .call
        .as_ref()
        .map(|call| (call.voice_session_id, call.is_inviter))
        .unwrap_or((0, false));
    stop_audio(&guard.audio);
    guard.audio = None;
    guard.call = None;
    *recv_tx().lock().unwrap_or_else(|err| err.into_inner()) = None;
    reset_all_buffers();
    if close_voice_webrpc && is_inviter && voice_id > 0 {
        webrpc::close_voice_webrpc_session(voice_id);
    }
    if emit {
        emit_state(VoiceUiState::idle());
    }
}

fn pack_voice_binary(call_id: &str, seq: u32, timestamp_ms: u32, opus: &[u8]) -> Option<Vec<u8>> {
    let call_bytes = call_id.as_bytes();
    if call_bytes.len() > 255 || opus.len() > u16::MAX as usize {
        return None;
    }
    let mut payload = Vec::with_capacity(15 + call_bytes.len() + opus.len());
    payload.extend_from_slice(&[0, 0, 0, VOICE_BIN_VER]);
    payload.push(call_bytes.len() as u8);
    payload.extend_from_slice(call_bytes);
    payload.extend_from_slice(&seq.to_le_bytes());
    payload.extend_from_slice(&timestamp_ms.to_le_bytes());
    payload.extend_from_slice(&(opus.len() as u16).to_le_bytes());
    payload.extend_from_slice(opus);
    Some(payload)
}

fn parse_voice_binary(payload: &[u8]) -> Option<(String, u32, u32, Vec<u8>)> {
    if payload.len() < 15
        || payload[0] != 0
        || payload[1] != 0
        || payload[2] != 0
        || payload[3] != VOICE_BIN_VER
    {
        return None;
    }
    let call_len = payload[4] as usize;
    let header = 5 + call_len + 4 + 4 + 2;
    if payload.len() < header {
        return None;
    }
    let call_id = std::str::from_utf8(&payload[5..5 + call_len])
        .ok()?
        .to_string();
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InboundSignal {
    op: String,
    #[serde(default)]
    call_id: String,
}

#[derive(Serialize)]
struct VoiceSessionSignal<'a> {
    #[serde(rename = "type")]
    kind: u8,
    data: VoiceSessionData<'a>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VoiceSessionData<'a> {
    op: &'a str,
    call_id: &'a str,
    session_id: u32,
}

fn send_voice_session_signal(
    chat_session_id: u32,
    op: &str,
    call_id: &str,
    voice_session_id: u32,
) {
    if chat_session_id == 0 || voice_session_id == 0 {
        return;
    }
    let Ok(payload) = serde_json::to_string(&VoiceSessionSignal {
        kind: 11,
        data: VoiceSessionData {
            op,
            call_id,
            session_id: voice_session_id,
        },
    }) else {
        return;
    };
    let _ = webrpc::send_json_timeout(chat_session_id, &payload, 10_000);
}

fn send_signal(session_id: u32, op: &str, call_id: &str) {
    let Ok(payload) = serde_json::to_string(&SignalMessage {
        kind: 6,
        data: SignalData { op, call_id },
    }) else {
        return;
    };
    let _ = webrpc::send_json_timeout(session_id, &payload, 10_000);
}

fn new_call_id(chat_session_id: u32) -> String {
    format!("{chat_session_id}-{}", now_ms())
}

fn end_call_with_signal(op: &str) -> Result<(), String> {
    let (chat_session_id, call_id, voice_id, is_inviter) = {
        let mut guard = lock_inner();
        let Some(call) = guard.call.take() else {
            return Ok(());
        };
        stop_audio(&guard.audio);
        guard.audio = None;
        *recv_tx().lock().unwrap_or_else(|err| err.into_inner()) = None;
        reset_all_buffers();
        (
            call.chat_session_id,
            call.call_id,
            call.voice_session_id,
            call.is_inviter,
        )
    };
    emit_state(VoiceUiState::idle());
    if voice_id > 0 {
        send_voice_session_signal(chat_session_id, "stop", &call_id, voice_id);
    }
    send_signal(chat_session_id, op, &call_id);
    if is_inviter && voice_id > 0 {
        webrpc::close_voice_webrpc_session(voice_id);
    }
    Ok(())
}

pub fn is_busy() -> bool {
    lock_inner().call.is_some()
}

#[tauri::command]
pub fn voice_invite(session_id: u32, peer_pass: Option<String>) -> Result<(), String> {
    if session_id == 0 {
        return Err("请先连接会话再发起语音通话。".into());
    }
    if webrpc::rpc_handle() == 0 {
        return Err("尚未登录。".into());
    }
    if crate::videocall::is_busy() {
        return Err("当前已有视频通话，请先结束。".into());
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
    {
        let mut guard = lock_inner();
        if guard.call.is_some() {
            return Err("当前已有语音通话，请先结束后再拨打。".into());
        }
        guard.call = Some(Call {
            phase: VoicePhase::Outgoing,
            chat_session_id: session_id,
            voice_session_id: 0,
            call_id: call_id.clone(),
            peer_token: peer,
            peer_pass: pass,
            is_inviter: true,
            muted: false,
            started_at: Instant::now(),
            started_unix_ms: now_ms(),
        });
        emit_state(snapshot(guard.call.as_ref()));
    }
    send_signal(session_id, "invite", &call_id);
    spawn_ring_timeout(session_id, call_id, VoicePhase::Outgoing);
    Ok(())
}

#[tauri::command]
pub fn voice_accept() -> Result<(), String> {
    let (chat_session_id, call_id) = {
        let mut guard = lock_inner();
        let call = guard
            .call
            .as_mut()
            .ok_or_else(|| "没有待接听的通话。".to_string())?;
        if call.phase != VoicePhase::Incoming {
            return Err("当前没有待接听的通话。".into());
        }
        call.phase = VoicePhase::Active;
        call.started_at = Instant::now();
        call.started_unix_ms = now_ms();
        (call.chat_session_id, call.call_id.clone())
    };
    send_signal(chat_session_id, "accept", &call_id);
    emit_state(snapshot(lock_inner().call.as_ref()));
    Ok(())
}

#[tauri::command]
pub fn voice_reject() -> Result<(), String> {
    let op = {
        let guard = lock_inner();
        match guard.call.as_ref().map(|call| call.phase) {
            Some(VoicePhase::Incoming) => "reject",
            Some(VoicePhase::Outgoing) => "cancel",
            _ => "hangup",
        }
    };
    end_call_with_signal(op)
}

#[tauri::command]
pub fn voice_hangup() -> Result<(), String> {
    end_call_with_signal("hangup")
}

#[tauri::command]
pub fn voice_set_mute(muted: bool) -> Result<(), String> {
    mute_flag().store(muted, Ordering::Relaxed);
    let mut guard = lock_inner();
    let Some(call) = guard.call.as_mut() else {
        return Ok(());
    };
    if call.phase != VoicePhase::Active {
        return Ok(());
    }
    call.muted = muted;
    emit_state(snapshot(Some(call)));
    Ok(())
}

#[tauri::command]
pub fn voice_state() -> VoiceUiState {
    snapshot(lock_inner().call.as_ref())
}

pub fn shutdown() {
    let mut guard = lock_inner();
    clear_to_idle(&mut guard, true, true);
}

pub fn on_chat_session_dead(chat_session_id: u32) {
    let should_clear = {
        let guard = lock_inner();
        guard
            .call
            .as_ref()
            .map(|call| call.chat_session_id == chat_session_id)
            .unwrap_or(false)
    };
    if should_clear {
        let mut guard = lock_inner();
        clear_to_idle(&mut guard, true, true);
    }
}

pub fn on_voice_transport_dead(voice_session_id: u32) {
    let should_clear = {
        let guard = lock_inner();
        guard
            .call
            .as_ref()
            .map(|call| call.voice_session_id == voice_session_id)
            .unwrap_or(false)
    };
    if should_clear {
        let mut guard = lock_inner();
        clear_to_idle(&mut guard, true, false);
    }
}

pub fn on_voice_session_signal(chat_session_id: u32, data: serde_json::Value) {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct VoiceSessionPayload {
        op: String,
        #[serde(default)]
        call_id: String,
        #[serde(default)]
        session_id: u32,
    }
    let parsed: VoiceSessionPayload = match serde_json::from_value(data) {
        Ok(v) => v,
        Err(_) => return,
    };
    let op = parsed.op.trim();
    let call_id = parsed.call_id.trim();
    if call_id.is_empty() {
        return;
    }
    match op {
        "start" => on_voice_session_start(chat_session_id, call_id, parsed.session_id),
        "stop" => on_voice_session_stop(chat_session_id, call_id),
        _ => {}
    }
}

pub fn on_signal(session_id: u32, data: serde_json::Value) {
    let parsed: InboundSignal = match serde_json::from_value(data) {
        Ok(v) => v,
        Err(_) => return,
    };
    let op = parsed.op.trim();
    let call_id = parsed.call_id.trim();
    if op.starts_with("vcall_") {
        return;
    }
    match op {
        "invite" => on_invite(session_id, call_id),
        "accept" => on_accept(session_id, call_id),
        "reject" | "cancel" | "hangup" | "busy" | "timeout" => {
            on_end(session_id, call_id, op == "busy")
        }
        _ => {}
    }
}

/// webrpc 收包线程：只负责把 Opus 帧丢给接收队列，不做解压。
pub fn on_audio_binary(session_id: u32, payload: &[u8]) {
    let Some((call_id, seq, _ts, opus)) = parse_voice_binary(payload) else {
        return;
    };
    let accepted = {
        let guard = lock_inner();
        let Some(call) = guard.call.as_ref() else {
            return;
        };
        call.phase == VoicePhase::Active
            && call.voice_session_id == session_id
            && call.voice_session_id > 0
            && (call_id.is_empty() || call_id == call.call_id)
    };
    if !accepted {
        return;
    }
    let recv_cell = recv_tx();
    let tx_slot = recv_cell.lock().unwrap_or_else(|err| err.into_inner());
    let Some(tx) = tx_slot.as_ref() else {
        return;
    };
    match tx.try_send((seq, opus)) {
        Ok(()) => {}
        Err(TrySendError::Full(_)) => {}
        Err(TrySendError::Disconnected(_)) => {}
    }
}

fn on_invite(session_id: u32, call_id: &str) {
    if call_id.is_empty() {
        return;
    }
    let peer = webrpc::peer_token_of(session_id);
    let mut guard = lock_inner();
    if let Some(call) = guard.call.as_ref() {
        if call.chat_session_id == session_id && call.call_id == call_id {
            return;
        }
        drop(guard);
        send_signal(session_id, "busy", call_id);
        return;
    }
    guard.call = Some(Call {
        phase: VoicePhase::Incoming,
        chat_session_id: session_id,
        voice_session_id: 0,
        call_id: call_id.to_string(),
        peer_token: peer,
        peer_pass: String::new(),
        is_inviter: false,
        muted: false,
        started_at: Instant::now(),
        started_unix_ms: now_ms(),
    });
    emit_state(snapshot(guard.call.as_ref()));
    drop(guard);
    spawn_ring_timeout(session_id, call_id.to_string(), VoicePhase::Incoming);
}

fn on_accept(session_id: u32, call_id: &str) {
    let call_id = {
        let mut guard = lock_inner();
        let Some(call) = guard.call.as_mut() else {
            return;
        };
        if call.phase != VoicePhase::Outgoing
            || call.chat_session_id != session_id
            || !call.is_inviter
            || (!call_id.is_empty() && call.call_id != call_id)
        {
            return;
        }
        call.phase = VoicePhase::Active;
        call.started_at = Instant::now();
        call.started_unix_ms = now_ms();
        call.call_id.clone()
    };

    let hangup_call_id = call_id.clone();
    if thread::Builder::new()
        .name("voice-open".into())
        .spawn(move || inviter_begin_voice(session_id, call_id))
        .is_err()
    {
        eprintln!("voice: spawn inviter_begin_voice failed");
        emit_voice_error("语音会话创建失败");
        send_signal(session_id, "hangup", &hangup_call_id);
        let mut guard = lock_inner();
        clear_to_idle(&mut guard, true, false);
    }
}

fn inviter_begin_voice(session_id: u32, call_id: String) {
    let peer_pass = {
        let guard = lock_inner();
        guard
            .call
            .as_ref()
            .filter(|call| {
                call.is_inviter
                    && call.chat_session_id == session_id
                    && (call_id.is_empty() || call.call_id == call_id)
            })
            .map(|call| call.peer_pass.clone())
            .unwrap_or_default()
    };

    let voice_session_id =
        match webrpc::open_inviter_voice_session(session_id, Some(&peer_pass)) {
            Ok(id) => id,
            Err(err) => {
                eprintln!("voice: open voice session failed: {err}");
                emit_voice_error("语音会话创建失败");
                send_signal(session_id, "hangup", &call_id);
                let mut guard = lock_inner();
                clear_to_idle(&mut guard, true, false);
                return;
            }
        };

    {
        let mut guard = lock_inner();
        let Some(call) = guard.call.as_mut() else {
            webrpc::close_voice_webrpc_session(voice_session_id);
            return;
        };
        if !call.is_inviter
            || call.chat_session_id != session_id
            || (!call_id.is_empty() && call.call_id != call_id)
        {
            webrpc::close_voice_webrpc_session(voice_session_id);
            return;
        }
        call.voice_session_id = voice_session_id;
    }

    send_voice_session_signal(session_id, "start", &call_id, voice_session_id);

    if let Err(err) = start_media(voice_session_id, call_id.clone()) {
        eprintln!("voice: start media failed: {err}");
        emit_voice_error(&err);
        send_voice_session_signal(session_id, "stop", &call_id, voice_session_id);
        send_signal(session_id, "hangup", &call_id);
        webrpc::close_voice_webrpc_session(voice_session_id);
        let mut guard = lock_inner();
        clear_to_idle(&mut guard, true, false);
        return;
    }
    emit_state(snapshot(lock_inner().call.as_ref()));
}

fn on_voice_session_start(chat_session_id: u32, call_id: &str, voice_session_id: u32) {
    if voice_session_id == 0 {
        return;
    }
    let call_id_owned = {
        let mut guard = lock_inner();
        let already_active = guard
            .call
            .as_ref()
            .map(|call| {
                call.chat_session_id == chat_session_id
                    && call.call_id == call_id
                    && !call.is_inviter
                    && call.voice_session_id == voice_session_id
            })
            .unwrap_or(false);
        if already_active && guard.audio.is_some() {
            return;
        }
        let Some(call) = guard.call.as_mut() else {
            return;
        };
        if call.chat_session_id != chat_session_id || call.call_id != call_id || call.is_inviter {
            return;
        }
        call.voice_session_id = voice_session_id;
        call.phase = VoicePhase::Active;
        call.started_at = Instant::now();
        call.started_unix_ms = now_ms();
        call.call_id.clone()
    };

    if let Err(err) = start_media(voice_session_id, call_id_owned) {
        eprintln!("voice: callee start media failed: {err}");
        emit_voice_error(&err);
        send_voice_session_signal(chat_session_id, "stop", call_id, voice_session_id);
        send_signal(chat_session_id, "hangup", call_id);
        let mut guard = lock_inner();
        clear_to_idle(&mut guard, true, false);
        return;
    }
    emit_state(snapshot(lock_inner().call.as_ref()));
}

fn on_voice_session_stop(chat_session_id: u32, call_id: &str) {
    let mut guard = lock_inner();
    let Some(call) = guard.call.as_ref() else {
        return;
    };
    if call.chat_session_id != chat_session_id || call.call_id != call_id {
        return;
    }
    clear_to_idle(&mut guard, true, true);
}

fn on_end(session_id: u32, call_id: &str, busy: bool) {
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
    let _ = busy;
    clear_to_idle(&mut guard, true, true);
}

fn spawn_ring_timeout(session_id: u32, call_id: String, wait_phase: VoicePhase) {
    thread::Builder::new()
        .name("voice-ring".into())
        .spawn(move || {
            thread::sleep(Duration::from_millis(RING_TIMEOUT_MS));
            let (send_op, send_session) = {
                let mut guard = lock_inner();
                let Some(call) = guard.call.as_ref() else {
                    return;
                };
                if call.chat_session_id != session_id || call.call_id != call_id || call.phase != wait_phase
                {
                    return;
                }
                let op = if wait_phase == VoicePhase::Outgoing {
                    "timeout"
                } else {
                    "reject"
                };
                let sid = call.chat_session_id;
                let cid = call.call_id.clone();
                let voice_id = call.voice_session_id;
                let is_inviter = call.is_inviter;
                clear_to_idle(&mut guard, true, false);
                if is_inviter && voice_id > 0 {
                    webrpc::close_voice_webrpc_session(voice_id);
                }
                ((op.to_string(), cid), sid)
            };
            send_signal(send_session, &send_op.0, &send_op.1);
        })
        .ok();
}

fn start_media(session_id: u32, call_id: String) -> Result<(), String> {
    reset_all_buffers();

    let host = cpal::default_host();
    let input_dev = host
        .default_input_device()
        .ok_or_else(|| "未找到麦克风。请在系统设置中允许 File2File 使用麦克风。".to_string())?;
    let output_dev = host
        .default_output_device()
        .ok_or_else(|| "未找到扬声器。".to_string())?;
    let in_cfg = input_dev
        .default_input_config()
        .map_err(|err| format!("无法打开麦克风: {err}"))?;
    let out_cfg = output_dev
        .default_output_config()
        .map_err(|err| format!("无法打开扬声器: {err}"))?;

    let stop = Arc::new(AtomicBool::new(false));
    mute_flag().store(false, Ordering::Relaxed);

    let (opus_tx, opus_rx) = mpsc::sync_channel::<(u32, Vec<u8>)>(RECV_OPUS_QUEUE);
    *recv_tx().lock().unwrap_or_else(|err| err.into_inner()) = Some(opus_tx);

    let play_buf = play_pcm_buf();
    let input = build_input_stream(&input_dev, &in_cfg, stop.clone())?;
    let output = build_output_stream(&output_dev, &out_cfg, play_buf, stop.clone())?;
    input.play().map_err(|err| format!("麦克风启动失败: {err}"))?;
    output
        .play()
        .map_err(|err| format!("扬声器启动失败: {err}"))?;

    let recv = thread::Builder::new()
        .name("voice-recv".into())
        .spawn({
            let stop = stop.clone();
            move || run_recv_loop(opus_rx, stop)
        })
        .map_err(|err| format!("语音接收线程失败: {err}"))?;

    let seq = Arc::new(AtomicU32::new(1));
    let stream_start = now_ms();
    let send = thread::Builder::new()
        .name("voice-send".into())
        .spawn({
            let stop = stop.clone();
            move || run_send_loop(session_id, call_id, seq, stream_start, stop)
        })
        .map_err(|err| format!("语音发送线程失败: {err}"))?;

    let mut guard = lock_inner();
    stop_audio(&guard.audio);
    guard.audio = Some(LiveAudio {
        stop,
        _input: SendStream(input),
        _output: SendStream(output),
        _recv: recv,
        _send: send,
    });
    if let Some(call) = guard.call.as_mut() {
        call.muted = false;
        mute_flag().store(false, Ordering::Relaxed);
    }
    Ok(())
}

/// 接收线程：从队列取 Opus → 按序解压 → 写入播放 PCM 缓冲（最多 2 秒）。
fn run_recv_loop(opus_rx: mpsc::Receiver<(u32, Vec<u8>)>, stop: Arc<AtomicBool>) {
    let queue = opus_queue();
    let play = play_pcm_buf();
    let Ok(mut decoder) = Decoder::new(SAMPLE_RATE, Channels::Mono) else {
        eprintln!("voice: opus decoder init failed");
        return;
    };

    while !stop.load(Ordering::SeqCst) {
        let mut got = false;
        while let Ok((seq, opus)) = opus_rx.try_recv() {
            queue
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .insert(seq, opus);
            got = true;
        }

        let opus = queue
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .pop_in_order();
        if let Some(packet) = opus {
            let pcm = decode_opus(&mut decoder, &packet);
            play.lock()
                .unwrap_or_else(|err| err.into_inner())
                .push_frame(&pcm);
            got = true;
        }

        if !got {
            thread::sleep(Duration::from_millis(2));
        }
    }
}

/// 发送线程：从发送 PCM 缓冲取帧 → Opus 编码 → webrpc 送出。
fn run_send_loop(
    session_id: u32,
    call_id: String,
    seq: Arc<AtomicU32>,
    stream_start: u64,
    stop: Arc<AtomicBool>,
) {
    let send_buf = send_pcm_buf();
    let mut encoder = match Encoder::builder(SAMPLE_RATE, Channels::Mono, Application::Voip)
        .bitrate(Bitrate::Bits(OPUS_BITRATE))
        .vbr(true)
        .dtx(false)
        .build()
    {
        Ok(v) => v,
        Err(err) => {
            eprintln!("voice: opus encoder init failed: {err}");
            return;
        }
    };
    let mut opus_out = vec![0u8; OPUS_PACKET_MAX];

    while !stop.load(Ordering::SeqCst) {
        let frame = send_buf
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .pop();
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
        let Some(payload) = pack_voice_binary(&call_id, n, ts, &opus_out[..opus_len]) else {
            continue;
        };
        let _ = webrpc::send_bytes_timeout(session_id, &payload, VOICE_SEND_TIMEOUT_MS);
    }
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

fn mute_flag() -> Arc<AtomicBool> {
    static CELL: OnceLock<Arc<AtomicBool>> = OnceLock::new();
    CELL.get_or_init(|| Arc::new(AtomicBool::new(false)))
        .clone()
}

fn push_capture_frames(samples: &[i16]) {
    if samples.is_empty() || mute_flag().load(Ordering::Relaxed) {
        return;
    }
    let accum = capture_accum();
    let mut slot = accum.lock().unwrap_or_else(|err| err.into_inner());
    slot.extend_from_slice(samples);
    let send = send_pcm_buf();
    while slot.len() >= FRAME_SAMPLES {
        let frame: Vec<i16> = slot.drain(..FRAME_SAMPLES).collect();
        send.lock()
            .unwrap_or_else(|err| err.into_inner())
            .push(frame);
    }
}

fn build_input_stream(
    device: &cpal::Device,
    config: &cpal::SupportedStreamConfig,
    stop: Arc<AtomicBool>,
) -> Result<Stream, String> {
    let rate = config.sample_rate().0;
    let channels = config.channels().max(1);
    let err_fn = |err| eprintln!("voice input: {err}");
    let stream_config = config.config();
    match config.sample_format() {
        SampleFormat::F32 => device.build_input_stream(
            &stream_config,
            move |data: &[f32], _| {
                capture_cb(data, rate, channels, &stop);
            },
            err_fn,
            None,
        ),
        SampleFormat::I16 => device.build_input_stream(
            &stream_config,
            move |data: &[i16], _| {
                capture_cb(data, rate, channels, &stop);
            },
            err_fn,
            None,
        ),
        SampleFormat::I32 => device.build_input_stream(
            &stream_config,
            move |data: &[i32], _| {
                capture_cb(data, rate, channels, &stop);
            },
            err_fn,
            None,
        ),
        SampleFormat::U8 => device.build_input_stream(
            &stream_config,
            move |data: &[u8], _| {
                capture_cb(data, rate, channels, &stop);
            },
            err_fn,
            None,
        ),
        other => return Err(format!("麦克风格式不支持: {other}")),
    }
    .map_err(|err| format!("打开麦克风失败: {err}"))
}

fn capture_cb<T: Sample + FromSample<f32>>(
    data: &[T],
    rate: u32,
    channels: u16,
    stop: &AtomicBool,
) where
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
    let pcm = resample_mono_i16(&mono, rate, SAMPLE_RATE);
    push_capture_frames(&pcm);
}

fn build_output_stream(
    device: &cpal::Device,
    config: &cpal::SupportedStreamConfig,
    play_buf: Arc<Mutex<PlayPcmBuffer>>,
    stop: Arc<AtomicBool>,
) -> Result<Stream, String> {
    let rate = config.sample_rate().0;
    let channels = config.channels().max(1);
    let err_fn = |err| eprintln!("voice output: {err}");
    let stream_config = config.config();
    match config.sample_format() {
        SampleFormat::F32 => device.build_output_stream(
            &stream_config,
            move |data: &mut [f32], _| {
                render_cb(data, rate, channels, &play_buf, &stop);
            },
            err_fn,
            None,
        ),
        SampleFormat::I16 => device.build_output_stream(
            &stream_config,
            move |data: &mut [i16], _| {
                render_cb(data, rate, channels, &play_buf, &stop);
            },
            err_fn,
            None,
        ),
        SampleFormat::I32 => device.build_output_stream(
            &stream_config,
            move |data: &mut [i32], _| {
                render_cb(data, rate, channels, &play_buf, &stop);
            },
            err_fn,
            None,
        ),
        other => return Err(format!("扬声器格式不支持: {other}")),
    }
    .map_err(|err| format!("打开扬声器失败: {err}"))
}

/// 声卡回调：只从播放缓冲取 PCM，不做解压/网络。
fn render_cb<T: Sample + FromSample<i16>>(
    data: &mut [T],
    rate: u32,
    channels: u16,
    play_buf: &Mutex<PlayPcmBuffer>,
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
    let src = play_buf
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .take(need);
    let mut pcm = if rate == SAMPLE_RATE {
        src
    } else {
        resample_mono_i16(&src, SAMPLE_RATE, rate)
    };
    if pcm.len() < frames {
        pcm.resize(frames, 0);
    }
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
    if input.is_empty() {
        return Vec::new();
    }
    if from == 0 || to == 0 || from == to {
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
