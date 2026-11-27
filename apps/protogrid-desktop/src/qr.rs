//! QR decoding for the desktop import dialog: no camera, just an image file.

use std::path::Path;

pub fn decode_image(path: &Path) -> Result<String, String> {
    let image = image::open(path).map_err(|error| format!("изображение: {error}"))?;
    let gray = image.to_luma8();
    let mut prepared = rqrr::PreparedImage::prepare(gray);
    let mut values = Vec::new();
    for grid in prepared.detect_grids() {
        if let Ok((_, content)) = grid.decode() {
            let value = content.trim();
            if !value.is_empty() {
                values.push(value.to_string());
            }
        }
    }
    if values.is_empty() {
        Err("QR-код на изображении не найден".into())
    } else {
        Ok(values.join("\n"))
    }
}
