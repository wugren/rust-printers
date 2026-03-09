use image::DynamicImage;

use crate::common::{
    base::job::PrinterJobOptions,
    traits::platform::{PlatformActions, PlatformPrinterGetters},
};

pub fn print_image(
    printer_system_name: &str,
    image: DynamicImage,
    print_name: Option<&str>,
    page_count: u32,
    print_width: Option<f64>,
    print_height: Option<f64>,
) -> Result<u64, &'static str> {
    let png_bytes = super::image_to_png_bytes(&image)?;
    let png_path = crate::unix::utils::file::save_tmp_file_with_ext(&png_bytes, "png")
        .ok_or("Failed to create temp file")?;
    let png_path = png_path.to_str().ok_or("Failed to create temp file path")?;

    let copies = super::normalize_page_count(page_count).to_string();
    let custom_media = match (print_width, print_height) {
        (Some(width), Some(height)) => super::media_custom_mm(width, height),
        _ => None,
    };
    let printer_resolution = query_printer_resolution(printer_system_name);

    let mut last_error = "Failed to print image";

    for document_format in [None, Some("image/png")] {
        match print_file_with_optional_media_fallback(
            printer_system_name,
            png_path,
            print_name,
            &copies,
            custom_media.as_deref(),
            document_format,
            printer_resolution.as_deref(),
        ) {
            Ok(job_id) => return Ok(job_id),
            Err(error) => last_error = error,
        }
    }

    Err(last_error)
}

fn print_file_with_optional_media_fallback(
    printer_system_name: &str,
    file_path: &str,
    print_name: Option<&str>,
    copies: &str,
    custom_media: Option<&str>,
    document_format: Option<&str>,
    printer_resolution: Option<&str>,
) -> Result<u64, &'static str> {
    let first_options =
        build_print_options(copies, custom_media, document_format, printer_resolution);
    let first_try =
        print_file_with_options(printer_system_name, file_path, print_name, &first_options);

    if first_try.is_ok() || custom_media.is_none() {
        return first_try;
    }

    let fallback_options = build_print_options(copies, None, document_format, printer_resolution);
    print_file_with_options(
        printer_system_name,
        file_path,
        print_name,
        &fallback_options,
    )
}

fn build_print_options(
    copies: &str,
    custom_media: Option<&str>,
    document_format: Option<&str>,
    printer_resolution: Option<&str>,
) -> Vec<(String, String)> {
    let mut options = vec![(String::from("copies"), copies.to_owned())];

    if let Some(resolution) = printer_resolution {
        options.push((String::from("printer-resolution"), String::from(resolution)));
    }

    if let Some(format) = document_format {
        options.push((String::from("document-format"), String::from(format)));
    }

    if let Some(media) = custom_media {
        options.push((String::from("media"), String::from(media)));
        options.push((String::from("print-scaling"), String::from("fit")));
    }

    options
}

fn print_file_with_options(
    printer_system_name: &str,
    file_path: &str,
    print_name: Option<&str>,
    options: &[(String, String)],
) -> Result<u64, &'static str> {
    let raw_properties: Vec<(&str, &str)> = options
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();

    <crate::Platform as PlatformActions>::print_file(
        printer_system_name,
        file_path,
        PrinterJobOptions {
            name: print_name,
            raw_properties: &raw_properties,
        },
    )
}

fn query_printer_resolution(printer_system_name: &str) -> Option<String> {
    let dests = crate::unix::cups::dests::get_dests().unwrap_or_default();
    let resolution = dests
        .iter()
        .find(|dest| {
            dest.get_name() == printer_system_name || dest.get_system_name() == printer_system_name
        })
        .and_then(query_dest_resolution);

    crate::unix::cups::dests::free(dests);
    resolution.map(format_resolution)
}

fn query_dest_resolution(dest: &crate::unix::cups::dests::CupsDestT) -> Option<(i32, i32)> {
    if let Some(resolution) = crate::unix::cups::attrs::query_printer_dpi(dest) {
        return Some(resolution);
    }

    for key in [
        "printer-resolution-default",
        "printer-resolution-supported",
        "printer-resolution",
        "Resolution",
    ] {
        let value = dest.get_option_value(key);
        if let Some(resolution) = parse_resolution_value(value.as_str()) {
            return Some(resolution);
        }
    }

    None
}

fn format_resolution((x, y): (i32, i32)) -> String {
    if x == y {
        format!("{x}dpi")
    } else {
        format!("{x}x{y}dpi")
    }
}

fn parse_resolution_value(value: &str) -> Option<(i32, i32)> {
    let text = value.trim().to_ascii_lowercase();
    if text.is_empty() {
        return None;
    }

    let text = text.strip_suffix("dpi").unwrap_or(text.as_str()).trim();
    if let Some((x_part, y_part)) = text.split_once('x') {
        let x = parse_first_i32(x_part)?;
        let y = parse_first_i32(y_part)?;
        if x > 0 && y > 0 {
            return Some((x, y));
        }
        return None;
    }

    let dpi = parse_first_i32(text)?;
    if dpi > 0 {
        Some((dpi, dpi))
    } else {
        None
    }
}

fn parse_first_i32(text: &str) -> Option<i32> {
    let mut started = false;
    let mut digits = String::new();

    for ch in text.chars() {
        if ch.is_ascii_digit() {
            started = true;
            digits.push(ch);
        } else if started {
            break;
        }
    }

    if digits.is_empty() {
        None
    } else {
        digits.parse::<i32>().ok()
    }
}
