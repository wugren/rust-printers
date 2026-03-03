use libc::{c_char, c_int};
use std::{ffi::CString, os::raw::c_void, ptr};

use crate::unix::cups::dests::CupsDestT;

const CUPS_IPP_OK: c_int = 0x0000;
const CUPS_IPP_OP_GET_PRINTER_ATTRIBUTES: c_int = 0x000B;

const CUPS_IPP_TAG_ZERO: c_int = 0x00;
const CUPS_IPP_TAG_OPERATION: c_int = 0x01;
const CUPS_IPP_TAG_KEYWORD: c_int = 0x44;
const CUPS_IPP_TAG_URI: c_int = 0x45;
const CUPS_IPP_TAG_RESOLUTION: c_int = 0x32;

#[link(name = "cups")]
unsafe extern "C" {
    unsafe fn cupsDoRequest(
        http: *mut c_void,
        request: *mut c_void,
        resource: *const c_char,
    ) -> *mut c_void;
    unsafe fn cupsLastError() -> c_int;

    unsafe fn ippNewRequest(op: c_int) -> *mut c_void;
    unsafe fn ippDelete(req: *mut c_void);

    unsafe fn ippAddString(
        req: *mut c_void,
        group: c_int,
        value_tag: c_int,
        name: *const c_char,
        lang: *const c_char,
        value: *const c_char,
    );

    unsafe fn ippAddStrings(
        req: *mut c_void,
        group: c_int,
        value_tag: c_int,
        name: *const c_char,
        num_values: c_int,
        lang: *const c_char,
        values: *const *const c_char,
    );

    unsafe fn ippFindAttribute(
        response: *mut c_void,
        name: *const c_char,
        value_tag: c_int,
    ) -> *mut c_void;
    unsafe fn ippGetCount(attr: *mut c_void) -> c_int;
    unsafe fn ippGetValueTag(attr: *mut c_void) -> c_int;
    unsafe fn ippGetString(
        attr: *mut c_void,
        idx: c_int,
        language: *mut *const c_char,
    ) -> *const c_char;
    unsafe fn ippGetResolution(
        attr: *mut c_void,
        idx: c_int,
        yres: *mut c_int,
        units: *mut c_int,
    ) -> c_int;
}

pub fn query_printer_dpi(dest: &CupsDestT) -> Option<(i32, i32)> {
    unsafe {
        let req = ippNewRequest(CUPS_IPP_OP_GET_PRINTER_ATTRIBUTES);
        if req.is_null() {
            return None;
        }

        add_printer_uri(req, dest);
        add_requested_attributes(req);

        let resource = CString::new("/").ok()?;
        let response = cupsDoRequest(ptr::null_mut(), req, resource.as_ptr());
        if response.is_null() || cupsLastError() != CUPS_IPP_OK {
            if !response.is_null() {
                ippDelete(response);
            }
            return None;
        }

        let dpi = query_default_resolution(response)
            .or_else(|| query_highest_supported_resolution(response))
            .or_else(|| query_urf_resolution(response));

        ippDelete(response);
        dpi
    }
}

unsafe fn add_printer_uri(req: *mut c_void, dest: &CupsDestT) {
    let uri = dest.get_option_value("printer-uri-supported");
    if let (Ok(name), Ok(value)) = (CString::new("printer-uri"), CString::new(uri)) {
        unsafe {
            ippAddString(
                req,
                CUPS_IPP_TAG_OPERATION,
                CUPS_IPP_TAG_URI,
                name.as_ptr(),
                ptr::null(),
                value.as_ptr(),
            );
        }
    }
}

unsafe fn add_requested_attributes(req: *mut c_void) {
    let name = match CString::new("requested-attributes") {
        Ok(v) => v,
        Err(_) => return,
    };

    let attributes = [
        "printer-resolution-default",
        "printer-resolution-supported",
        "urf-supported",
    ];

    let values: Vec<CString> = attributes
        .iter()
        .filter_map(|item| CString::new(*item).ok())
        .collect();

    if values.len() != attributes.len() {
        return;
    }

    let ptrs: Vec<*const c_char> = values.iter().map(|v| v.as_ptr()).collect();

    unsafe {
        ippAddStrings(
            req,
            CUPS_IPP_TAG_OPERATION,
            CUPS_IPP_TAG_KEYWORD,
            name.as_ptr(),
            ptrs.len() as c_int,
            ptr::null(),
            ptrs.as_ptr(),
        );
    }
}

unsafe fn query_default_resolution(response: *mut c_void) -> Option<(i32, i32)> {
    let name = CString::new("printer-resolution-default").ok()?;
    unsafe {
        parse_resolution_attr(
            ippFindAttribute(response, name.as_ptr(), CUPS_IPP_TAG_ZERO),
            false,
        )
    }
}

unsafe fn query_highest_supported_resolution(response: *mut c_void) -> Option<(i32, i32)> {
    let name = CString::new("printer-resolution-supported").ok()?;
    unsafe {
        parse_resolution_attr(
            ippFindAttribute(response, name.as_ptr(), CUPS_IPP_TAG_ZERO),
            true,
        )
    }
}

unsafe fn query_urf_resolution(response: *mut c_void) -> Option<(i32, i32)> {
    let name = CString::new("urf-supported").ok()?;
    let attr = unsafe { ippFindAttribute(response, name.as_ptr(), CUPS_IPP_TAG_ZERO) };
    if attr.is_null() {
        return None;
    }

    let count = unsafe { ippGetCount(attr) };
    let mut best = 0;

    for idx in 0..count {
        let value_ptr = unsafe { ippGetString(attr, idx, ptr::null_mut()) };
        if value_ptr.is_null() {
            continue;
        }

        let text = unsafe { std::ffi::CStr::from_ptr(value_ptr) }
            .to_string_lossy()
            .to_ascii_lowercase();

        for token in text.split(',') {
            let token = token.trim();
            if !token.starts_with("rs") {
                continue;
            }

            let max = token
                .chars()
                .map(|c| if c.is_ascii_digit() { c } else { ' ' })
                .collect::<String>()
                .split_ascii_whitespace()
                .filter_map(|n| n.parse::<i32>().ok())
                .max()
                .unwrap_or_default();

            if max > best {
                best = max;
            }
        }
    }

    if best > 0 {
        Some((best, best))
    } else {
        None
    }
}

unsafe fn parse_resolution_attr(attr: *mut c_void, pick_highest: bool) -> Option<(i32, i32)> {
    if attr.is_null() {
        return None;
    }

    let count = unsafe { ippGetCount(attr) };
    if count <= 0 {
        return None;
    }

    let value_tag = unsafe { ippGetValueTag(attr) };
    let mut best: Option<(i32, i32)> = None;

    for idx in 0..count {
        let current = if value_tag == CUPS_IPP_TAG_RESOLUTION {
            let mut yres = 0;
            let mut units = 0;
            let xres = unsafe { ippGetResolution(attr, idx, &mut yres, &mut units) };
            normalize_resolution(xres, yres)
        } else {
            let value_ptr = unsafe { ippGetString(attr, idx, ptr::null_mut()) };
            if value_ptr.is_null() {
                None
            } else {
                let text = unsafe { std::ffi::CStr::from_ptr(value_ptr) }
                    .to_string_lossy()
                    .into_owned();
                parse_resolution_value(text.as_str())
            }
        };

        if let Some((x, y)) = current {
            if !pick_highest {
                return Some((x, y));
            }

            match best {
                Some((bx, by)) => {
                    if (x * y, x, y) > (bx * by, bx, by) {
                        best = Some((x, y));
                    }
                }
                None => best = Some((x, y)),
            }
        }
    }

    best
}

fn normalize_resolution(x: i32, y: i32) -> Option<(i32, i32)> {
    if x > 0 && y > 0 {
        Some((x, y))
    } else if x > 0 {
        Some((x, x))
    } else {
        None
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
