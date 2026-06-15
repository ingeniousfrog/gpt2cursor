use image::{imageops::FilterType, Rgba, RgbaImage};
use tauri::image::Image;

const TRAY_PX: u32 = 44;
const CORNER_RADIUS: f32 = 10.0;

pub fn load_tray_icon() -> Result<Image<'static>, String> {
    let decoded = image::load_from_memory(include_bytes!("../icons/icon.png"))
        .map_err(|err| format!("tray icon decode failed: {err}"))?;
    let cropped = crop_to_content(decoded.to_rgba8());
    let resized = image::imageops::resize(&cropped, TRAY_PX, TRAY_PX, FilterType::Lanczos3);
    let masked = mask_rounded_corners(resized, CORNER_RADIUS);
    Ok(Image::new_owned(masked.into_raw(), TRAY_PX, TRAY_PX))
}

fn crop_to_content(img: RgbaImage) -> RgbaImage {
    let (width, height) = img.dimensions();
    let mut min_x = width;
    let mut min_y = height;
    let mut max_x = 0;
    let mut max_y = 0;

    for y in 0..height {
        for x in 0..width {
            let pixel = img.get_pixel(x, y);
            if is_content_pixel(pixel) {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }

    if max_x <= min_x || max_y <= min_y {
        return img;
    }

    let content_w = max_x - min_x + 1;
    let content_h = max_y - min_y + 1;
    let pad_x = (content_w as f32 * 0.08).round() as u32;
    let pad_y = (content_h as f32 * 0.08).round() as u32;
    let left = min_x.saturating_sub(pad_x);
    let top = min_y.saturating_sub(pad_y);
    let right = (max_x + pad_x + 1).min(width);
    let bottom = (max_y + pad_y + 1).min(height);
    image::imageops::crop_imm(&img, left, top, right - left, bottom - top).to_image()
}

fn is_content_pixel(pixel: &Rgba<u8>) -> bool {
    if pixel[3] < 24 {
        return false;
    }
    let luminance = (pixel[0] as u16 + pixel[1] as u16 + pixel[2] as u16) / 3;
    luminance > 28
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
