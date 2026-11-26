//! Brand artwork embedded into the desktop client.
//!
//! The PNG is converted to raw RGBA once at build time (see
//! `assets/icon.rgba`) so the app needs no image-decoding dependency.

use eframe::egui;

const ICON_SIZE: u32 = 256;
static ICON_RGBA: &[u8] = include_bytes!("../assets/icon.rgba");

pub fn color_image() -> egui::ColorImage {
    egui::ColorImage::from_rgba_unmultiplied([ICON_SIZE as usize, ICON_SIZE as usize], ICON_RGBA)
}

pub fn icon_data(size: u32) -> egui::IconData {
    egui::IconData {
        rgba: icon_rgba(size),
        width: size,
        height: size,
    }
}

/// Returns the brand icon at `size` pixels. The source is 256x256, so
/// smaller sizes are box-filtered and larger ones are bilinearly scaled.
pub fn icon_rgba(size: u32) -> Vec<u8> {
    if size == ICON_SIZE {
        return ICON_RGBA.to_vec();
    }
    if size < ICON_SIZE {
        downscale(size)
    } else {
        upscale(size)
    }
}

fn downscale(size: u32) -> Vec<u8> {
    let mut out = vec![0_u8; (size * size * 4) as usize];
    let ratio = ICON_SIZE as f64 / size as f64;
    for y in 0..size {
        let y0 = (y as f64 * ratio).floor() as u32;
        let y1 = (((y + 1) as f64 * ratio).ceil() as u32).min(ICON_SIZE);
        for x in 0..size {
            let x0 = (x as f64 * ratio).floor() as u32;
            let x1 = (((x + 1) as f64 * ratio).ceil() as u32).min(ICON_SIZE);
            let mut sum = [0.0_f64; 4];
            let mut count = 0.0_f64;
            for sy in y0..y1 {
                for sx in x0..x1 {
                    let index = ((sy * ICON_SIZE + sx) * 4) as usize;
                    let alpha = ICON_RGBA[index + 3] as f64 / 255.0;
                    sum[0] += ICON_RGBA[index] as f64 * alpha;
                    sum[1] += ICON_RGBA[index + 1] as f64 * alpha;
                    sum[2] += ICON_RGBA[index + 2] as f64 * alpha;
                    sum[3] += alpha;
                    count += 1.0;
                }
            }
            let alpha = sum[3] / count;
            let index = ((y * size + x) * 4) as usize;
            if alpha > 0.0 {
                out[index] = (sum[0] / sum[3]).round() as u8;
                out[index + 1] = (sum[1] / sum[3]).round() as u8;
                out[index + 2] = (sum[2] / sum[3]).round() as u8;
            }
            out[index + 3] = (alpha * 255.0).round() as u8;
        }
    }
    out
}

fn upscale(size: u32) -> Vec<u8> {
    let mut out = vec![0_u8; (size * size * 4) as usize];
    let scale = if size <= 1 {
        0.0
    } else {
        (ICON_SIZE - 1) as f64 / (size - 1) as f64
    };
    for y in 0..size {
        let sy = y as f64 * scale;
        let y0 = sy.floor() as u32;
        let y1 = (y0 + 1).min(ICON_SIZE - 1);
        let fy = sy - y0 as f64;
        for x in 0..size {
            let sx = x as f64 * scale;
            let x0 = sx.floor() as u32;
            let x1 = (x0 + 1).min(ICON_SIZE - 1);
            let fx = sx - x0 as f64;
            let mut accum = [0.0_f64; 4];
            for (sx, sy, weight) in [
                (x0, y0, (1.0 - fx) * (1.0 - fy)),
                (x1, y0, fx * (1.0 - fy)),
                (x0, y1, (1.0 - fx) * fy),
                (x1, y1, fx * fy),
            ] {
                let index = ((sy * ICON_SIZE + sx) * 4) as usize;
                let alpha = ICON_RGBA[index + 3] as f64 / 255.0;
                accum[0] += ICON_RGBA[index] as f64 * alpha * weight;
                accum[1] += ICON_RGBA[index + 1] as f64 * alpha * weight;
                accum[2] += ICON_RGBA[index + 2] as f64 * alpha * weight;
                accum[3] += alpha * weight;
            }
            let index = ((y * size + x) * 4) as usize;
            if accum[3] > 0.0 {
                out[index] = (accum[0] / accum[3]).round() as u8;
                out[index + 1] = (accum[1] / accum[3]).round() as u8;
                out[index + 2] = (accum[2] / accum[3]).round() as u8;
            }
            out[index + 3] = (accum[3] * 255.0).round() as u8;
        }
    }
    out
}
