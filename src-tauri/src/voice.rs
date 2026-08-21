use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{self, SyncSender};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use aec3::nodes::audio::AudioFormat;
use aec3::pipelines::linear;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, Stream};
use serde::{Deserialize, Serialize};
use tauri::Emitter;

use crate::webrpc;

const SAMPLE_RATE: u32 = 16_000;
const FRAME_MS: u32 = 20;
const FRAME_SAMPLES: usize = (SAMPLE_RATE * FRAME_MS / 1000) as usize;
const AEC_FRAME_SAMPLES: usize = (SAMPLE_RATE / 100) as usize;
const AEC_INITIAL_DELAY_MS: i32 = 80;
const RING_TIMEOUT_MS: u64 = 45_000;
const PLAY_MAX_SAMPLES: usize = SAMPLE_RATE as usize / 2;
const VOICE_SEND_TIMEOUT_MS: i64 = 0;

enum VoiceProcMsg {
    Render(Vec<i16>),
    Capture(Vec<i16>),
}

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
    session_id: u32,
    call_id: String,
    peer_token: String,
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
    _aec: thread::JoinHandle<()>,
    _sender: thread::JoinHandle<()>,
}

struct VoiceInner {
    call: Option<Call>,
    audio: Option<LiveAudio>,
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

fn lock_inner() -> std::sync::MutexGuard<'static, VoiceInner> {
    inner().lock().unwrap_or_else(|err| err.into_inner())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|item| item.as_millis() as u64)
        .unwrap_or(0)
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
        session_id: call.session_id,
        call_id: call.call_id.clone(),
        peer_token: call.peer_token.clone(),
        muted: call.muted,
        started_at: call.started_unix_ms,
    }
}

fn stop_audio(audio: &Option<LiveAudio>) {
    if let Some(live) = audio {
        live.stop.store(true, Ordering::SeqCst);
    }
}

fn clear_to_idle(guard: &mut VoiceInner, emit: bool) {
    stop_audio(&guard.audio);
    guard.audio = None;
    guard.call = None;
    if emit {
        emit_state(VoiceUiState::idle());
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
struct FrameMessage<'a> {
    #[serde(rename = "type")]
    kind: u8,
    data: FrameData<'a>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FrameData<'a> {
    call_id: &'a str,
    seq: u32,
    audio: &'a str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InboundSignal {
    op: String,
    #[serde(default)]
    call_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InboundFrame {
    #[serde(default)]
    call_id: String,
    #[serde(default)]
    audio: String,
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

fn new_call_id(session_id: u32) -> String {
    format!("{session_id}-{}", now_ms())
}

#[tauri::command]
pub fn voice_invite(session_id: u32) -> Result<(), String> {
    if session_id == 0 {
        return Err("请先连接会话再发起语音通话。".into());
    }
    if webrpc::rpc_handle() == 0 {
        return Err("尚未登录。".into());
    }
    let peer = webrpc::peer_token_of(session_id);
    let call_id = new_call_id(session_id);
    {
        let mut guard = lock_inner();
        if guard.call.is_some() {
            return Err("当前已有语音通话，请先结束后再拨打。".into());
        }
        guard.call = Some(Call {
            phase: VoicePhase::Outgoing,
            session_id,
            call_id: call_id.clone(),
            peer_token: peer,
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
    let (session_id, call_id) = {
        let mut guard = lock_inner();
        let call = guard.call.as_mut().ok_or_else(|| "没有待接听的通话。".to_string())?;
        if call.phase != VoicePhase::Incoming {
            return Err("当前没有待接听的通话。".into());
        }
        call.phase = VoicePhase::Active;
        call.started_at = Instant::now();
        call.started_unix_ms = now_ms();
        (call.session_id, call.call_id.clone())
    };
    send_signal(session_id, "accept", &call_id);
    start_media(session_id, call_id)?;
    emit_state(snapshot(lock_inner().call.as_ref()));
    Ok(())
}

#[tauri::command]
pub fn voice_reject() -> Result<(), String> {
    let (session_id, call_id, op) = {
        let mut guard = lock_inner();
        let call = guard.call.take();
        stop_audio(&guard.audio);
        guard.audio = None;
        emit_state(VoiceUiState::idle());
        match call {
            Some(call) if call.phase == VoicePhase::Incoming => {
                (call.session_id, call.call_id, "reject")
            }
            Some(call) if call.phase == VoicePhase::Outgoing => {
                (call.session_id, call.call_id, "cancel")
            }
            Some(call) => (call.session_id, call.call_id, "hangup"),
            None => return Ok(()),
        }
    };
    send_signal(session_id, op, &call_id);
    Ok(())
}

#[tauri::command]
pub fn voice_hangup() -> Result<(), String> {
    voice_reject()
}

#[tauri::command]
pub fn voice_set_mute(muted: bool) -> Result<(), String> {
    mute_flag().store(muted, Ordering::SeqCst);
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
    clear_to_idle(&mut guard, true);
}

pub fn on_session_dead(session_id: u32) {
    let should_clear = {
        let guard = lock_inner();
        guard
            .call
            .as_ref()
            .map(|call| call.session_id == session_id)
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
    let op = parsed.op.trim();
    let call_id = parsed.call_id.trim();
    match op {
        "invite" => on_invite(session_id, call_id),
        "accept" => on_accept(session_id, call_id),
        "reject" | "cancel" | "hangup" | "busy" | "timeout" => {
            on_end(session_id, call_id, op == "busy")
        }
        _ => {}
    }
}

pub fn on_audio_frame(session_id: u32, data: serde_json::Value) {
    let parsed: InboundFrame = match serde_json::from_value(data) {
        Ok(v) => v,
        Err(_) => return,
    };
    let samples = {
        let guard = lock_inner();
        let Some(call) = guard.call.as_ref() else {
            return;
        };
        if call.phase != VoicePhase::Active
            || call.session_id != session_id
            || (!parsed.call_id.is_empty() && parsed.call_id != call.call_id)
        {
            return;
        }
        decode_pcm(&parsed.audio)
    };
    if samples.is_empty() {
        return;
    }
    push_playback(samples);
}

fn on_invite(session_id: u32, call_id: &str) {
    if call_id.is_empty() {
        return;
    }
    let peer = webrpc::peer_token_of(session_id);
    let mut guard = lock_inner();
    if let Some(call) = guard.call.as_ref() {
        if call.session_id == session_id && call.call_id == call_id {
            return;
        }
        drop(guard);
        send_signal(session_id, "busy", call_id);
        return;
    }
    guard.call = Some(Call {
        phase: VoicePhase::Incoming,
        session_id,
        call_id: call_id.to_string(),
        peer_token: peer,
        muted: false,
        started_at: Instant::now(),
        started_unix_ms: now_ms(),
    });
    emit_state(snapshot(guard.call.as_ref()));
    drop(guard);
    spawn_ring_timeout(session_id, call_id.to_string(), VoicePhase::Incoming);
}

fn on_accept(session_id: u32, call_id: &str) {
    {
        let mut guard = lock_inner();
        let Some(call) = guard.call.as_mut() else {
            return;
        };
        if call.phase != VoicePhase::Outgoing
            || call.session_id != session_id
            || (!call_id.is_empty() && call.call_id != call_id)
        {
            return;
        }
        call.phase = VoicePhase::Active;
        call.started_at = Instant::now();
        call.started_unix_ms = now_ms();
    }
    let call_id = {
        let guard = lock_inner();
        guard
            .call
            .as_ref()
            .map(|call| call.call_id.clone())
            .unwrap_or_default()
    };
    if let Err(err) = start_media(session_id, call_id) {
        eprintln!("voice: start media failed: {err}");
        shutdown();
        return;
    }
    emit_state(snapshot(lock_inner().call.as_ref()));
}

fn on_end(session_id: u32, call_id: &str, busy: bool) {
    let mut guard = lock_inner();
    let Some(call) = guard.call.as_ref() else {
        return;
    };
    if call.session_id != session_id {
        return;
    }
    if !call_id.is_empty() && call.call_id != call_id {
        return;
    }
    let _ = busy;
    clear_to_idle(&mut guard, true);
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
                if call.session_id != session_id || call.call_id != call_id || call.phase != wait_phase
                {
                    return;
                }
                let op = if wait_phase == VoicePhase::Outgoing {
                    "timeout"
                } else {
                    "reject"
                };
                let sid = call.session_id;
                let cid = call.call_id.clone();
                clear_to_idle(&mut guard, true);
                ( (op.to_string(), cid), sid )
            };
            send_signal(send_session, &send_op.0, &send_op.1);
        })
        .ok();
}

fn start_media(session_id: u32, call_id: String) -> Result<(), String> {
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
    let play_buf = playback_buf();
    {
        let mut buf = play_buf.lock().unwrap_or_else(|err| err.into_inner());
        buf.clear();
    }

    let (proc_tx, proc_rx) = mpsc::sync_channel::<VoiceProcMsg>(48);
    let (send_tx, send_rx) = mpsc::sync_channel::<Vec<i16>>(8);
    let muted = mute_flag();
    muted.store(false, Ordering::SeqCst);

    let input = build_input_stream(&input_dev, &in_cfg, proc_tx.clone(), stop.clone())?;
    let output = build_output_stream(
        &output_dev,
        &out_cfg,
        play_buf.clone(),
        proc_tx,
        stop.clone(),
    )?;
    input.play().map_err(|err| format!("麦克风启动失败: {err}"))?;
    output
        .play()
        .map_err(|err| format!("扬声器启动失败: {err}"))?;

    let aec = thread::Builder::new()
        .name("voice-aec".into())
        .spawn({
            let stop = stop.clone();
            let muted = muted.clone();
            move || run_aec_loop(proc_rx, send_tx, stop, muted)
        })
        .map_err(|err| format!("语音处理线程失败: {err}"))?;

    let seq = Arc::new(AtomicU32::new(1));
    let sender = thread::Builder::new()
        .name("voice-send".into())
        .spawn({
            let stop = stop.clone();
            let call_id = call_id.clone();
            move || {
                while !stop.load(Ordering::SeqCst) {
                    let Ok(frame) = send_rx.recv_timeout(Duration::from_millis(200)) else {
                        continue;
                    };
                    if stop.load(Ordering::SeqCst) || frame.is_empty() {
                        continue;
                    }
                    let bytes: Vec<u8> = frame.iter().flat_map(|s| s.to_le_bytes()).collect();
                    let audio = B64.encode(&bytes);
                    let n = seq.fetch_add(1, Ordering::Relaxed);
                    let Ok(payload) = serde_json::to_string(&FrameMessage {
                        kind: 5,
                        data: FrameData {
                            call_id: &call_id,
                            seq: n,
                            audio: &audio,
                        },
                    }) else {
                        continue;
                    };
                    let _ = webrpc::send_json_timeout(session_id, &payload, VOICE_SEND_TIMEOUT_MS);
                }
            }
        })
        .map_err(|err| format!("语音发送线程失败: {err}"))?;

    let mut guard = lock_inner();
    stop_audio(&guard.audio);
    guard.audio = Some(LiveAudio {
        stop,
        _input: SendStream(input),
        _output: SendStream(output),
        _aec: aec,
        _sender: sender,
    });
    if let Some(call) = guard.call.as_mut() {
        call.muted = false;
        muted.store(false, Ordering::SeqCst);
    }
    Ok(())
}

fn mute_flag() -> Arc<AtomicBool> {
    static CELL: OnceLock<Arc<AtomicBool>> = OnceLock::new();
    CELL.get_or_init(|| Arc::new(AtomicBool::new(false)))
        .clone()
}

fn playback_buf() -> Arc<Mutex<VecDeque<i16>>> {
    static CELL: OnceLock<Arc<Mutex<VecDeque<i16>>>> = OnceLock::new();
    CELL.get_or_init(|| Arc::new(Mutex::new(VecDeque::new())))
        .clone()
}

fn push_playback(samples: Vec<i16>) {
    let buf = playback_buf();
    let mut slot = buf.lock().unwrap_or_else(|err| err.into_inner());
    slot.extend(samples);
    while slot.len() > PLAY_MAX_SAMPLES {
        slot.pop_front();
    }
}

fn decode_pcm(audio: &str) -> Vec<i16> {
    let Ok(bytes) = B64.decode(audio.trim()) else {
        return Vec::new();
    };
    if bytes.len() < 2 {
        return Vec::new();
    }
    bytes
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect()
}

fn build_input_stream(
    device: &cpal::Device,
    config: &cpal::SupportedStreamConfig,
    tx: SyncSender<VoiceProcMsg>,
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
                capture_cb(data, rate, channels, &tx, &stop);
            },
            err_fn,
            None,
        ),
        SampleFormat::I16 => device.build_input_stream(
            &stream_config,
            move |data: &[i16], _| {
                capture_cb(data, rate, channels, &tx, &stop);
            },
            err_fn,
            None,
        ),
        SampleFormat::I32 => device.build_input_stream(
            &stream_config,
            move |data: &[i32], _| {
                capture_cb(data, rate, channels, &tx, &stop);
            },
            err_fn,
            None,
        ),
        SampleFormat::U8 => device.build_input_stream(
            &stream_config,
            move |data: &[u8], _| {
                capture_cb(data, rate, channels, &tx, &stop);
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
    tx: &SyncSender<VoiceProcMsg>,
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
    let frame = resample_mono_i16(&mono, rate, SAMPLE_RATE);
    if frame.is_empty() {
        return;
    }
    let _ = tx.try_send(VoiceProcMsg::Capture(frame));
}

fn build_output_stream(
    device: &cpal::Device,
    config: &cpal::SupportedStreamConfig,
    play_buf: Arc<Mutex<VecDeque<i16>>>,
    tx: SyncSender<VoiceProcMsg>,
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
                render_cb(data, rate, channels, &play_buf, &tx, &stop);
            },
            err_fn,
            None,
        ),
        SampleFormat::I16 => device.build_output_stream(
            &stream_config,
            move |data: &mut [i16], _| {
                render_cb(data, rate, channels, &play_buf, &tx, &stop);
            },
            err_fn,
            None,
        ),
        SampleFormat::I32 => device.build_output_stream(
            &stream_config,
            move |data: &mut [i32], _| {
                render_cb(data, rate, channels, &play_buf, &tx, &stop);
            },
            err_fn,
            None,
        ),
        other => return Err(format!("扬声器格式不支持: {other}")),
    }
    .map_err(|err| format!("打开扬声器失败: {err}"))
}

fn render_cb<T: Sample + FromSample<i16>>(
    data: &mut [T],
    rate: u32,
    channels: u16,
    play_buf: &Mutex<VecDeque<i16>>,
    tx: &SyncSender<VoiceProcMsg>,
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
    let need = ((frames as u64) * SAMPLE_RATE as u64 / rate.max(1) as u64) as usize + 2;
    let src = {
        let mut slot = play_buf.lock().unwrap_or_else(|err| err.into_inner());
        let mut take = Vec::with_capacity(need.min(slot.len()));
        while take.len() < need {
            match slot.pop_front() {
                Some(v) => take.push(v),
                None => break,
            }
        }
        take
    };
    let mut far_end = src;
    far_end.resize(need, 0);
    let _ = tx.try_send(VoiceProcMsg::Render(far_end.clone()));
    let resampled = if far_end.iter().all(|s| *s == 0) {
        vec![0i16; frames]
    } else {
        let mut out = resample_mono_i16(&far_end, SAMPLE_RATE, rate);
        if out.len() < frames {
            out.resize(frames, 0);
        }
        out
    };
    for (i, frame) in resampled.iter().take(frames).enumerate() {
        let sample = T::from_sample(*frame);
        for c in 0..ch {
            let idx = i * ch + c;
            if idx < data.len() {
                data[idx] = sample;
            }
        }
    }
}

fn run_aec_loop(
    proc_rx: mpsc::Receiver<VoiceProcMsg>,
    send_tx: SyncSender<Vec<i16>>,
    stop: Arc<AtomicBool>,
    muted: Arc<AtomicBool>,
) {
    let format = AudioFormat::ten_ms(SAMPLE_RATE, 1);
    let mut pipeline = match linear::builder(format, format)
        .initial_delay_ms(AEC_INITIAL_DELAY_MS)
        .build()
    {
        Ok(pipeline) => Some(pipeline),
        Err(err) => {
            eprintln!("voice aec init: {err}");
            None
        }
    };
    let n = AEC_FRAME_SAMPLES;
    debug_assert_eq!(n, format.sample_count());
    let mut render_buf = Vec::<f32>::new();
    let mut capture_buf = Vec::<f32>::new();
    let mut send_buf = Vec::<i16>::new();
    let mut out = vec![0.0f32; n];
    let mut raw_send = Vec::<i16>::new();

    while !stop.load(Ordering::SeqCst) {
        let msg = match proc_rx.recv_timeout(Duration::from_millis(50)) {
            Ok(msg) => msg,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        };
        match msg {
            VoiceProcMsg::Render(samples) => {
                if let Some(pipeline) = pipeline.as_mut() {
                    render_buf.extend(samples.iter().copied().map(i16_to_f32));
                    while render_buf.len() >= n {
                        let frame: Vec<f32> = render_buf.drain(..n).collect();
                        if let Err(err) = pipeline.handle_render_frame(&frame) {
                            eprintln!("voice aec render: {err}");
                        }
                    }
                }
            }
            VoiceProcMsg::Capture(samples) => {
                if let Some(pipeline) = pipeline.as_mut() {
                    capture_buf.extend(samples.iter().copied().map(i16_to_f32));
                    while capture_buf.len() >= n {
                        let frame: Vec<f32> = capture_buf.drain(..n).collect();
                        match pipeline.process_capture_frame(&frame, &mut out) {
                            Ok(true) => {
                                if !muted.load(Ordering::Relaxed) {
                                    send_buf.extend(out.iter().copied().map(f32_to_i16));
                                }
                            }
                            Ok(false) => {}
                            Err(err) => eprintln!("voice aec capture: {err}"),
                        }
                        while send_buf.len() >= FRAME_SAMPLES {
                            let chunk: Vec<i16> = send_buf.drain(..FRAME_SAMPLES).collect();
                            let _ = send_tx.try_send(chunk);
                        }
                    }
                } else if !muted.load(Ordering::Relaxed) {
                    raw_send.extend(samples);
                    while raw_send.len() >= FRAME_SAMPLES {
                        let chunk: Vec<i16> = raw_send.drain(..FRAME_SAMPLES).collect();
                        let _ = send_tx.try_send(chunk);
                    }
                }
            }
        }
    }
}

fn i16_to_f32(sample: i16) -> f32 {
    sample as f32 / 32768.0
}

fn f32_to_i16(sample: f32) -> i16 {
    (sample * 32767.0).clamp(i16::MIN as f32, i16::MAX as f32) as i16
}

fn resample_mono_i16(input: &[i16], from: u32, to: u32) -> Vec<i16> {
    if input.is_empty() {
        return Vec::new();
    }
    if from == 0 || to == 0 {
        return input.to_vec();
    }
    if from == to {
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
