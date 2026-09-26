//! 默认摄像头采集（Win / macOS）。

use image::{imageops::FilterType, RgbaImage};
use nokhwa::pixel_format::RgbFormat;
use nokhwa::utils::{CameraIndex, RequestedFormat, RequestedFormatType};
use nokhwa::Camera;

pub struct CameraCapture {
    camera: Camera,
    max_edge: u32,
}

impl CameraCapture {
    pub fn open_default(max_edge: u32) -> Result<Self, String> {
        let index = CameraIndex::Index(0);
        let requested = RequestedFormat::new::<RgbFormat>(RequestedFormatType::AbsoluteHighestResolution);
        let mut camera = Camera::new(index, requested).map_err(|err| format!("打开摄像头失败: {err}"))?;
        camera
            .open_stream()
            .map_err(|err| format!("启动摄像头失败: {err}"))?;
        Ok(Self {
            camera,
            max_edge: max_edge.max(160),
        })
    }

    pub fn capture_rgba(&mut self) -> Result<RgbaImage, String> {
        let frame = self
            .camera
            .frame()
            .map_err(|err| format!("读取摄像头画面失败: {err}"))?;
        let decoded = frame
            .decode_image::<RgbFormat>()
            .map_err(|err| format!("解码摄像头画面失败: {err}"))?;
        let width = decoded.width();
        let height = decoded.height();
        if width == 0 || height == 0 {
            return Err("摄像头画面无效".into());
        }
        let rgb = decoded.as_raw();
        let mut rgba = RgbaImage::new(width, height);
        for y in 0..height {
            for x in 0..width {
                let i = ((y * width + x) * 3) as usize;
                if i + 2 >= rgb.len() {
                    continue;
                }
                rgba.put_pixel(x, y, image::Rgba([rgb[i], rgb[i + 1], rgb[i + 2], 255]));
            }
        }
        Ok(scale_to_max_edge(rgba, self.max_edge))
    }
}

fn scale_to_max_edge(img: RgbaImage, max_edge: u32) -> RgbaImage {
    let w = img.width();
    let h = img.height();
    if w == 0 || h == 0 {
        return img;
    }
    let edge = w.max(h);
    if edge <= max_edge {
        return img;
    }
    let scale = max_edge as f32 / edge as f32;
    let nw = ((w as f32 * scale).round() as u32).max(2) & !1;
    let nh = ((h as f32 * scale).round() as u32).max(2) & !1;
    image::imageops::resize(&img, nw, nh, FilterType::Triangle)
}

pub fn probe_camera() -> bool {
    CameraCapture::open_default(640).is_ok()
}
