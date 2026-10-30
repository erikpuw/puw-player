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

/// Extracts a soft ambient glow color `(r, g, b)` from an RGBA pixel buffer by weighting
/// mid-luminance, saturated pixels. Falls back to the pixel average when the cover is monochrome.
pub fn dominant_rgb(rgba: &[u8]) -> Option<(u8, u8, u8)> {
    if rgba.len() < 4 {
        return None;
    }
    let step = (rgba.len() / 4 / 1024).max(1) * 4;
    let mut r_sum = 0.0f64;
    let mut g_sum = 0.0f64;
    let mut b_sum = 0.0f64;
    let mut w_sum = 0.0f64;
    let mut avg_r = 0u64;
    let mut avg_g = 0u64;
    let mut avg_b = 0u64;
    let mut count = 0u64;

    let mut i = 0;
    while i + 3 < rgba.len() {
        let a = rgba[i + 3];
        if a >= 128 {
            let r = rgba[i] as f64;
            let g = rgba[i + 1] as f64;
            let b = rgba[i + 2] as f64;
            avg_r += rgba[i] as u64;
            avg_g += rgba[i + 1] as u64;
            avg_b += rgba[i + 2] as u64;
            count += 1;

            let max = r.max(g).max(b);
            let min = r.min(g).min(b);
            let sat = if max > 0.0 { (max - min) / max } else { 0.0 };
            let lum = (r * 0.299 + g * 0.587 + b * 0.114) / 255.0;
            if lum > 0.12 && lum < 0.88 {
                let w = 0.15 + sat * sat * 2.5;
                r_sum += r * w;
                g_sum += g * w;
                b_sum += b * w;
                w_sum += w;
            }
        }
        i += step;
    }
    if w_sum > 0.0 {
        Some((
            (r_sum / w_sum).round().clamp(0.0, 255.0) as u8,
            (g_sum / w_sum).round().clamp(0.0, 255.0) as u8,
            (b_sum / w_sum).round().clamp(0.0, 255.0) as u8,
        ))
    } else {
        Some((
            avg_r.checked_div(count)? as u8,
            avg_g.checked_div(count)? as u8,
            avg_b.checked_div(count)? as u8,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dominant_rgb_weights_saturated_pixels() {
        // 3 dark gray pixels + 1 vivid warm orange pixel
        let rgba: [u8; 16] = [
            30, 30, 30, 255,
            32, 32, 32, 255,
            28, 28, 28, 255,
            230, 110, 40, 255,
        ];
        let (r, g, b) = dominant_rgb(&rgba).expect("dominant rgb");
        assert!(r > 150 && r > g && g > b, "expected warm dominant tone, got ({r}, {g}, {b})");
        assert_eq!(dominant_rgb(&[]), None);
    }
}
