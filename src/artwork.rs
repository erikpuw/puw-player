use image::{DynamicImage, RgbaImage};

pub fn fit_cover(width: u32, height: u32, max: u32) -> Option<(u32, u32)> {
    if width == 0 || height == 0 || max == 0 {
        return None;
    }
    let scale = (max as f64 / width.max(height) as f64).min(1.0);
    Some((
        (width as f64 * scale).round().max(1.0) as u32,
        (height as f64 * scale).round().max(1.0) as u32,
    ))
}

/// Project a neighboring cover onto a trapezoid. Transparent margins preserve the image ratio.
pub fn perspective(source: &DynamicImage, right_side: bool) -> RgbaImage {
    let fit = source.thumbnail(256, 256).to_rgba8();
    let mut canvas = RgbaImage::new(180, 256);
    for x in 0..180 {
        let depth = if right_side {
            x as f32 / 179.0
        } else {
            1.0 - x as f32 / 179.0
        };
        let inset = (depth * 38.0) as u32;
        let h = 256 - 2 * inset;
        for y in inset..256 - inset {
            let sx = x * 256 / 180;
            let sy = (y - inset) * 256 / h;
            let ox = (256 - fit.width()) / 2;
            let oy = (256 - fit.height()) / 2;
            if sx >= ox && sx < ox + fit.width() && sy >= oy && sy < oy + fit.height() {
                canvas.put_pixel(x, y, *fit.get_pixel(sx - ox, sy - oy));
            }
        }
    }
    canvas
}

pub fn load_cover(bytes: &[u8]) -> Option<DynamicImage> {
    // Bound both compressed and decoded sizes for untrusted embedded artwork.
    if bytes.len() > 20 * 1024 * 1024 {
        return None;
    }
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    reader.decode().ok()
}
