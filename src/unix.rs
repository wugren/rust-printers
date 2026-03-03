use cups::dests::get_dests;
use std::str;
use image::DynamicImage;
use crate::common::{
    base::{
        job::{PrinterJob, PrinterJobOptions, PrinterJobState},
        printer::{Printer, PrinterState},
    },
    traits::platform::{PlatformActions, PlatformPrinterGetters},
};
use crate::DeviceCaps;

mod cups;
mod image_print;
mod utils;

impl PlatformActions for crate::Platform {
    fn get_printers() -> Vec<Printer> {
        let dests = get_dests().unwrap_or_default();
        let printers = dests
            .iter()
            .map(|p| Printer::from_platform_printer_getters(p))
            .collect();

        cups::dests::free(dests);
        printers
    }

    fn get_printer_caps(printer_system_name: &str) -> DeviceCaps {
        let dests = get_dests().unwrap_or_default();
        let caps = dests
            .iter()
            .find(|d| d.get_name() == printer_system_name || d.get_system_name() == printer_system_name)
            .map(build_device_caps)
            .unwrap_or_else(default_device_caps);

        cups::dests::free(dests);
        caps
    }

    fn print(
        printer_system_name: &str,
        buffer: &[u8],
        options: PrinterJobOptions,
    ) -> Result<u64, &'static str> {
        let path = utils::file::save_tmp_file(buffer);
        if let Some(file_path) = path {
            Self::print_file(printer_system_name, file_path.to_str().unwrap(), options)
        } else {
            Err("Failed to create temp file")
        }
    }

    fn print_file(
        printer_system_name: &str,
        file_path: &str,
        options: PrinterJobOptions,
    ) -> Result<u64, &'static str> {
        cups::jobs::print_file(
            printer_system_name,
            file_path,
            options.name,
            options.raw_properties,
        )
    }

    fn print_image(
        printer_system_name: &str,
        buffer: DynamicImage,
        print_name: Option<&str>,
        page_count: u32,
        print_width: Option<f64>,
        print_height: Option<f64>,
    ) -> Result<u64, &'static str> {
        image_print::print_image(
            printer_system_name,
            buffer,
            print_name,
            page_count,
            print_width,
            print_height,
        )
    }
    
    fn get_printer_jobs(printer_name: &str, active_only: bool) -> Vec<PrinterJob> {
        cups::jobs::get_printer_jobs(printer_name, active_only)
            .unwrap_or_default()
            .iter()
            .map(|j| PrinterJob::from_platform_printer_job_getters(j))
            .collect()
    }

    fn get_default_printer() -> Option<Printer> {
        let dests = get_dests().unwrap_or_default();
        let dest = dests
            .iter()
            .find(|d| d.get_is_default())
            .map(|d| Printer::from_platform_printer_getters(d));

        cups::dests::free(dests);
        dest
    }

    fn get_printer_by_name(printer_name: &str) -> Option<Printer> {
        let dests = get_dests().unwrap_or_default();
        let dest = dests
            .iter()
            .find(|d| d.get_name() == printer_name || d.get_system_name() == printer_name)
            .map(|d| Printer::from_platform_printer_getters(d));

        cups::dests::free(dests);
        dest
    }

    fn parse_printer_state(platform_state: u64, state_reasons: &str) -> PrinterState {
        if state_reasons.contains("offline-report") {
            return PrinterState::OFFLINE;
        }

        match platform_state {
            3 => PrinterState::READY,
            4 => PrinterState::PRINTING,
            5 => PrinterState::PAUSED,
            _ => PrinterState::UNKNOWN,
        }
    }

    fn parse_printer_job_state(platform_state: u64) -> PrinterJobState {
        match platform_state {
            3 => PrinterJobState::PENDING,
            4 | 6 => PrinterJobState::PAUSED,
            5 => PrinterJobState::PROCESSING,
            7 | 8 => PrinterJobState::CANCELLED,
            9 => PrinterJobState::COMPLETED,
            _ => PrinterJobState::UNKNOWN,
        }
    }

    fn set_job_state(
        printer_name: &str,
        job_id: u64,
        state: PrinterJobState,
    ) -> Result<(), &'static str> {
        let result = match state {
            PrinterJobState::PENDING => cups::jobs::restart_job(printer_name, job_id as i32),
            PrinterJobState::PROCESSING => cups::jobs::release_job(printer_name, job_id as i32),
            PrinterJobState::PAUSED => cups::jobs::hold_job(printer_name, job_id as i32),
            PrinterJobState::CANCELLED => cups::jobs::cancel_job(printer_name, job_id as i32),
            _ => false,
        };

        if result {
            Ok(())
        } else {
            Err("cups method failed")
        }
    }
}

const DEFAULT_DPI: i32 = 300;
const MM_PER_INCH: f64 = 25.4;

fn default_device_caps() -> DeviceCaps {
    DeviceCaps {
        dpi_x: DEFAULT_DPI,
        dpi_y: DEFAULT_DPI,
        page_width: 0,
        page_height: 0,
        print_table_width: 0,
        print_table_height: 0,
        margin_top: 0,
        margin_left: 0,
        margin_right: 0,
        margin_bottom: 0,
    }
}

fn build_device_caps(dest: &cups::dests::CupsDestT) -> DeviceCaps {
    let (dpi_x, dpi_y) = parse_printer_dpi(dest).unwrap_or((DEFAULT_DPI, DEFAULT_DPI));
    let (page_width, page_height) = parse_page_size_mm(dest)
        .map(|(w_mm, h_mm)| (mm_to_px(w_mm, dpi_x), mm_to_px(h_mm, dpi_y)))
        .unwrap_or((0, 0));

    DeviceCaps {
        dpi_x,
        dpi_y,
        page_width,
        page_height,
        print_table_width: page_width,
        print_table_height: page_height,
        margin_top: 0,
        margin_left: 0,
        margin_right: 0,
        margin_bottom: 0,
    }
}

#[cfg(target_os = "macos")]
fn parse_printer_dpi(dest: &cups::dests::CupsDestT) -> Option<(i32, i32)> {
    if let Some(resolution) = cups::attrs::query_printer_dpi(dest) {
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

#[cfg(all(target_family = "unix", not(target_os = "macos")))]
fn parse_printer_dpi(dest: &cups::dests::CupsDestT) -> Option<(i32, i32)> {
    if let Some(resolution) = cups::attrs::query_printer_dpi(dest) {
        return Some(resolution);
    }

    for key in [
        "printer-resolution-default",
        "printer-resolution-supported",
        "printer-resolution",
        "DefaultResolution",
        "Resolution",
    ] {
        let value = dest.get_option_value(key);
        if let Some(resolution) = parse_resolution_value(value.as_str()) {
            return Some(resolution);
        }
    }

    None
}

#[cfg(target_os = "macos")]
fn parse_page_size_mm(dest: &cups::dests::CupsDestT) -> Option<(f64, f64)> {
    for key in ["PageSize", "media-default", "media", "DefaultPaperSize"] {
        let value = dest.get_option_value(key);
        if let Some(size) = parse_media_size_mm(value.as_str()) {
            return Some(size);
        }
    }

    None
}

#[cfg(all(target_family = "unix", not(target_os = "macos")))]
fn parse_page_size_mm(dest: &cups::dests::CupsDestT) -> Option<(f64, f64)> {
    for key in ["media-default", "media", "PageSize"] {
        let value = dest.get_option_value(key);
        if let Some(size) = parse_media_size_mm(value.as_str()) {
            return Some(size);
        }
    }

    None
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

fn parse_media_size_mm(value: &str) -> Option<(f64, f64)> {
    let text = value.trim().to_ascii_lowercase();
    if text.is_empty() {
        return None;
    }

    if let Some(size) = parse_dimension_suffix_mm(text.as_str()) {
        return Some(size);
    }

    if let Some(size) = parse_dimension_suffix_in(text.as_str()) {
        return Some(size);
    }

    parse_named_media_mm(text.as_str())
}

fn parse_dimension_suffix_mm(text: &str) -> Option<(f64, f64)> {
    let base = text.strip_suffix("mm")?;
    let (w, h) = parse_xy_numbers(base)?;
    Some((w, h))
}

fn parse_dimension_suffix_in(text: &str) -> Option<(f64, f64)> {
    let base = text.strip_suffix("in")?;
    let (w, h) = parse_xy_numbers(base)?;
    Some((w * MM_PER_INCH, h * MM_PER_INCH))
}

fn parse_xy_numbers(text: &str) -> Option<(f64, f64)> {
    let (left, right) = text.rsplit_once('x')?;
    let w = parse_trailing_f64(left)?;
    let h = parse_leading_f64(right)?;
    if w > 0.0 && h > 0.0 {
        Some((w, h))
    } else {
        None
    }
}

fn parse_trailing_f64(text: &str) -> Option<f64> {
    let end = text
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .count();
    if end == 0 {
        return None;
    }

    let start = text.len().checked_sub(end)?;
    text[start..].parse::<f64>().ok()
}

fn parse_leading_f64(text: &str) -> Option<f64> {
    let len = text
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .count();
    if len == 0 {
        return None;
    }

    text[..len].parse::<f64>().ok()
}

fn parse_named_media_mm(text: &str) -> Option<(f64, f64)> {
    let normalized: String = text
        .chars()
        .filter(|c| *c != '-' && *c != '_' && !c.is_ascii_whitespace())
        .collect();

    match normalized.as_str() {
        "a3" => Some((297.0, 420.0)),
        "a4" => Some((210.0, 297.0)),
        "a5" => Some((148.0, 210.0)),
        "letter" => Some((215.9, 279.4)),
        "legal" => Some((215.9, 355.6)),
        _ => None,
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

fn mm_to_px(mm: f64, dpi: i32) -> i32 {
    ((mm / MM_PER_INCH) * dpi as f64).round() as i32
}
