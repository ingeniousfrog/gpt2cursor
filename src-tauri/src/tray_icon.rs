use image::{imageops::FilterType, Rgba, RgbaImage};
use tauri::image::Image;

const TRAY_PX: u32 = 44;
const CORNER_RADIUS: f32 = 10.0;

pub fn load_tray_icon() -> Result<Image<'static>, String> {
    let decoded = image::load_from_memory(include_bytes!("../icons/icon.png"))
        .map_err(|err| format!("tray icon decode failed: {err}"))?;
    let fitted = resize_to_fit(decoded.to_rgba8(), TRAY_PX);
    let masked = mask_rounded_corners(fitted, CORNER_RADIUS);
    Ok(Image::new_owned(masked.into_raw(), TRAY_PX, TRAY_PX))
}

fn resize_to_fit(img: RgbaImage, size: u32) -> RgbaImage {
    let (width, height) = img.dimensions();
    if width == 0 || height == 0 {
        return RgbaImage::from_pixel(size, size, Rgba([0, 0, 0, 0]));
    }

    let scale = (size as f32 / width as f32).min(size as f32 / height as f32);
    let target_w = ((width as f32 * scale).round() as u32).max(1);
    let target_h = ((height as f32 * scale).round() as u32).max(1);
    let resized = image::imageops::resize(&img, target_w, target_h, FilterType::Lanczos3);

    let mut canvas = RgbaImage::from_pixel(size, size, Rgba([0, 0, 0, 0]));
    let offset_x = i64::from((size - target_w) / 2);
    let offset_y = i64::from((size - target_h) / 2);
    image::imageops::overlay(&mut canvas, &resized, offset_x, offset_y);
    canvas
}

fn mask_rounded_corners(img: RgbaImage, radius: f32) -> RgbaImage {
    let (width, height) = img.dimensions();
    let mut out = img;
    for y in 0..height {
        for x in 0..width {
            if !inside_rounded_rect(x as f32 + 0.5, y as f32 + 0.5, width as f32, height as f32, radius)
            {
                out.put_pixel(x, y, Rgba([0, 0, 0, 0]));
            }
        }
    }
    out
}

fn inside_rounded_rect(x: f32, y: f32, width: f32, height: f32, radius: f32) -> bool {
    if x < radius && y < radius {
        let dx = x - radius;
        let dy = y - radius;
        return dx * dx + dy * dy <= radius * radius;
    }
    if x > width - radius && y < radius {
        let dx = x - (width - radius);
        let dy = y - radius;
        return dx * dx + dy * dy <= radius * radius;
    }
    if x < radius && y > height - radius {
        let dx = x - radius;
        let dy = y - (height - radius);
        return dx * dx + dy * dy <= radius * radius;
    }
    if x > width - radius && y > height - radius {
        let dx = x - (width - radius);
        let dy = y - (height - radius);
        return dx * dx + dy * dy <= radius * radius;
    }
    true
}
