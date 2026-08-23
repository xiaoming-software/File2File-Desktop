use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use core_graphics::display::{CGDisplay, CGPoint};
use core_graphics::event::{
    CGEvent, CGEventTapLocation, CGEventType, CGMouseButton, EventField, ScrollEventUnit,
};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_media_rs::cm_sample_buffer::CMSampleBuffer;
use core_media_rs::cm_time::CMTime;
use enigo::{Button, Direction};
use image::RgbaImage;
use screencapturekit::shareable_content::SCShareableContent;
use screencapturekit::stream::configuration::pixel_format::PixelFormat;
use screencapturekit::stream::configuration::SCStreamConfiguration;
use screencapturekit::stream::content_filter::SCContentFilter;
use screencapturekit::stream::output_trait::SCStreamOutputTrait;
use screencapturekit::stream::output_type::SCStreamOutputType;
use screencapturekit::stream::SCStream;

const PERMISSION_HINT: &str =
    "采集屏幕失败。请在「系统设置 → 隐私与安全性 → 屏幕录制」中允许 File2File，然后完全退出再打开。";

pub struct MacCapturer {
    stream: SCStream,
    latest: Arc<Mutex<Option<RgbaImage>>>,
}

impl MacCapturer {
    pub fn start(max_edge: u32, fps: u32) -> Result<Self, String> {
        let mut displays = SCShareableContent::get()
            .map_err(|err| {
                eprintln!("desktop: SCShareableContent {err}");
                PERMISSION_HINT.to_string()
            })?
            .displays();
        if displays.is_empty() {
            return Err(PERMISSION_HINT.into());
        }
        let main_id = CGDisplay::main().id;
        let display = displays
            .iter()
            .find(|item| item.display_id() == main_id)
            .cloned()
            .unwrap_or_else(|| displays.remove(0));
        let src_w = display.width().max(2);
        let src_h = display.height().max(2);
        let (width, height) = scaled_even(src_w, src_h, max_edge);
        let filter = SCContentFilter::new().with_display_excluding_windows(&display, &[]);
        let interval = CMTime {
            value: 1,
            timescale: fps.max(1) as i32,
            flags: 1,
            epoch: 0,
        };
        let config = SCStreamConfiguration::new()
            .set_width(width)
            .and_then(|c| c.set_height(height))
            .and_then(|c| c.set_pixel_format(PixelFormat::BGRA))
            .and_then(|c| c.set_shows_cursor(false))
            .and_then(|c| c.set_scales_to_fit(true))
            .and_then(|c| c.set_queue_depth(3))
            .and_then(|c| c.set_minimum_frame_interval(&interval))
            .map_err(|err| format!("配置屏幕采集失败: {err}"))?;
        let latest = Arc::new(Mutex::new(None));
        let mut stream = SCStream::new(&filter, &config);
        stream.add_output_handler(FrameSink { latest: latest.clone() }, SCStreamOutputType::Screen);
        stream.start_capture().map_err(|err| {
            eprintln!("desktop: start_capture {err}");
            PERMISSION_HINT.to_string()
        })?;
        Ok(Self { stream, latest })
    }

    pub fn take(&self) -> Option<RgbaImage> {
        self.latest.lock().unwrap_or_else(|err| err.into_inner()).take()
    }

    pub fn wait_first(&self, timeout: Duration) -> Result<RgbaImage, String> {
        let deadline = std::time::Instant::now() + timeout;
        while std::time::Instant::now() < deadline {
            if let Some(img) = self.take() {
                return Ok(img);
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        Err(PERMISSION_HINT.into())
    }

    pub fn stop(&mut self) {
        let _ = self.stream.stop_capture();
    }
}

fn scaled_even(w: u32, h: u32, max_edge: u32) -> (u32, u32) {
    let edge = w.max(h).max(2);
    let (mut out_w, mut out_h) = if edge > max_edge {
        let scale = max_edge as f32 / edge as f32;
        (
            ((w as f32 * scale).round() as u32).max(2),
            ((h as f32 * scale).round() as u32).max(2),
        )
    } else {
        (w, h)
    };
    out_w &= !1;
    out_h &= !1;
    (out_w.max(2), out_h.max(2))
}

struct FrameSink {
    latest: Arc<Mutex<Option<RgbaImage>>>,
}

impl SCStreamOutputTrait for FrameSink {
    fn did_output_sample_buffer(&self, sample_buffer: CMSampleBuffer, of_type: SCStreamOutputType) {
        if of_type != SCStreamOutputType::Screen {
            return;
        }
        if let Some(img) = rgba_from_sample(&sample_buffer) {
            *self.latest.lock().unwrap_or_else(|err| err.into_inner()) = Some(img);
        }
    }
}

fn rgba_from_sample(sample: &CMSampleBuffer) -> Option<RgbaImage> {
    use core_video_rs::cv_pixel_buffer::lock::LockTrait;
    let buffer = sample.get_pixel_buffer().ok()?;
    let width = buffer.get_width();
    let height = buffer.get_height();
    if width == 0 || height == 0 {
        return None;
    }
    let stride = buffer.get_bytes_per_row() as usize;
    let row_bytes = (width as usize).saturating_mul(4);
    let guard = buffer.lock().ok()?;
    let src = guard.as_slice();
    if stride < row_bytes || src.len() < stride.saturating_mul(height as usize) {
        return None;
    }
    let mut rgba = vec![0u8; row_bytes.saturating_mul(height as usize)];
    for y in 0..height as usize {
        let src_row = &src[y * stride..y * stride + row_bytes];
        let dst_row = &mut rgba[y * row_bytes..y * row_bytes + row_bytes];
        for x in 0..width as usize {
            let i = x * 4;
            dst_row[i] = src_row[i + 2];
            dst_row[i + 1] = src_row[i + 1];
            dst_row[i + 2] = src_row[i];
            dst_row[i + 3] = src_row[i + 3];
        }
    }
    RgbaImage::from_raw(width, height, rgba)
}

/// Quartz / CGEvent 使用主屏左上角为 (0,0)，单位是点。
pub fn logical_screen() -> (i32, i32, u32, u32) {
    let bounds = CGDisplay::main().bounds();
    (
        0,
        0,
        bounds.size.width.max(1.0) as u32,
        bounds.size.height.max(1.0) as u32,
    )
}

fn event_source() -> Result<CGEventSource, String> {
    CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| "无法创建鼠标事件".to_string())
}

fn last_point() -> &'static Mutex<(i32, i32)> {
    static CELL: OnceLock<Mutex<(i32, i32)>> = OnceLock::new();
    CELL.get_or_init(|| Mutex::new((0, 0)))
}

fn remember_point(x: i32, y: i32) {
    *last_point().lock().unwrap_or_else(|err| err.into_inner()) = (x, y);
}

pub fn last_xy() -> (i32, i32) {
    *last_point().lock().unwrap_or_else(|err| err.into_inner())
}

fn quartz_point(x: i32, y: i32) -> CGPoint {
    CGPoint::new(x as f64, y as f64)
}

fn mouse_button(button: Button) -> Result<CGMouseButton, String> {
    Ok(match button {
        Button::Left => CGMouseButton::Left,
        Button::Right => CGMouseButton::Right,
        Button::Middle => CGMouseButton::Center,
        _ => return Err("不支持的鼠标按键".into()),
    })
}

fn drag_type(button: Button) -> CGEventType {
    match button {
        Button::Right => CGEventType::RightMouseDragged,
        Button::Middle => CGEventType::OtherMouseDragged,
        _ => CGEventType::LeftMouseDragged,
    }
}

pub fn move_abs(x: i32, y: i32, drag: Option<Button>) -> Result<(), String> {
    remember_point(x, y);
    let src = event_source()?;
    let (ty, btn) = match drag {
        Some(button) => (drag_type(button), mouse_button(button)?),
        None => (CGEventType::MouseMoved, CGMouseButton::Left),
    };
    let event = CGEvent::new_mouse_event(src, ty, quartz_point(x, y), btn)
        .map_err(|_| "移动鼠标失败".to_string())?;
    event.post(CGEventTapLocation::HID);
    Ok(())
}

fn mouse_type(button: Button, down: bool) -> Result<(CGEventType, CGMouseButton), String> {
    Ok(match (button, down) {
        (Button::Left, true) => (CGEventType::LeftMouseDown, CGMouseButton::Left),
        (Button::Left, false) => (CGEventType::LeftMouseUp, CGMouseButton::Left),
        (Button::Right, true) => (CGEventType::RightMouseDown, CGMouseButton::Right),
        (Button::Right, false) => (CGEventType::RightMouseUp, CGMouseButton::Right),
        (Button::Middle, true) => (CGEventType::OtherMouseDown, CGMouseButton::Center),
        (Button::Middle, false) => (CGEventType::OtherMouseUp, CGMouseButton::Center),
        _ => return Err("不支持的鼠标按键".into()),
    })
}

pub fn button_at(button: Button, direction: Direction, x: i32, y: i32, clicks: i64) -> Result<(), String> {
    remember_point(x, y);
    let src = event_source()?;
    let dest = quartz_point(x, y);
    if matches!(direction, Direction::Press | Direction::Click) {
        let (ty, btn) = mouse_type(button, true)?;
        let event = CGEvent::new_mouse_event(src.clone(), ty, dest, btn)
            .map_err(|_| "按下鼠标失败".to_string())?;
        event.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, clicks);
        event.post(CGEventTapLocation::HID);
    }
    if matches!(direction, Direction::Release | Direction::Click) {
        let (ty, btn) = mouse_type(button, false)?;
        let event = CGEvent::new_mouse_event(src, ty, dest, btn)
            .map_err(|_| "松开鼠标失败".to_string())?;
        event.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, clicks);
        event.post(CGEventTapLocation::HID);
    }
    Ok(())
}

pub fn scroll(dx: i32, dy: i32) -> Result<(), String> {
    let src = event_source()?;
    if dy != 0 {
        let event = CGEvent::new_scroll_event(src.clone(), ScrollEventUnit::LINE, 1, -dy, 0, 0)
            .map_err(|_| "滚轮失败".to_string())?;
        event.post(CGEventTapLocation::HID);
    }
    if dx != 0 {
        let event = CGEvent::new_scroll_event(src, ScrollEventUnit::LINE, 2, 0, -dx, 0)
            .map_err(|_| "滚轮失败".to_string())?;
        event.post(CGEventTapLocation::HID);
    }
    Ok(())
}
