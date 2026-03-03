use image::{DynamicImage, ImageFormat};
use std::io::Cursor;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(all(target_family = "unix", not(target_os = "macos")))]
mod linux;

pub fn print_image(
    printer_system_name: &str,
    image: DynamicImage,
    print_name: Option<&str>,
    page_count: u32,
    print_width: Option<f64>,
    print_height: Option<f64>,
) -> Result<u64, &'static str> {
    #[cfg(target_os = "macos")]
    {
        return macos::print_image(
            printer_system_name,
            image,
            print_name,
            page_count,
            print_width,
            print_height,
        );
    }

    #[cfg(all(target_family = "unix", not(target_os = "macos")))]
    {
        return linux::print_image(
            printer_system_name,
            image,
            print_name,
            page_count,
            print_width,
            print_height,
        );
    }

    #[allow(unreachable_code)]
    Err("Unsupported unix platform")
}

fn image_to_png_bytes(image: DynamicImage) -> Result<Vec<u8>, &'static str> {
    let mut bytes = Vec::new();
    let mut cursor = Cursor::new(&mut bytes);

    image
        .write_to(&mut cursor, ImageFormat::Png)
        .map_err(|_| "Failed to encode image")?;

    Ok(bytes)
}

fn normalize_page_count(page_count: u32) -> u32 {
    page_count.max(1)
}

fn media_custom_mm(width: f64, height: f64) -> Option<String> {
    if width <= 0.0 || height <= 0.0 {
        return None;
    }

    Some(format!(
        "Custom.{}x{}mm",
        fmt_mm(width),
        fmt_mm(height)
    ))
}

fn fmt_mm(value: f64) -> String {
    let formatted = format!("{value:.2}");
    formatted
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}
