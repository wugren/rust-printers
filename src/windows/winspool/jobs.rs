#![allow(non_snake_case)]
#![allow(non_camel_case_types)]

use libc::{ c_ulong, c_void};
use std::{slice};
use windows::core::{BOOL, PCWSTR, PWSTR};
use windows::Win32::Graphics::Printing::*;
use crate::{
    common::traits::platform::PlatformPrinterJobGetters,
    windows::utils::{
        date::{calculate_system_time, get_current_epoch},
        strings::{str_to_wide_string, wchar_t_to_string},
    },
};
use crate::common::base::job::PrinterJob;

impl PlatformPrinterJobGetters for JOB_INFO_1W {
    fn get_id(&self) -> u64 {
        self.JobId.into()
    }

    fn get_name(&self) -> String {
        wchar_t_to_string(self.pDocument)
    }

    fn get_state(&self) -> u64 {
        self.Status.into()
    }

    fn get_printer(&self) -> String {
        wchar_t_to_string(self.pPrinterName)
    }

    fn get_media_type(&self) -> String {
        wchar_t_to_string(self.pDatatype)
    }

    fn get_created_at(&self) -> std::time::SystemTime {
        calculate_system_time(
            self.Submitted.wYear,
            self.Submitted.wMonth,
            self.Submitted.wDay,
            self.Submitted.wHour,
            self.Submitted.wMinute,
            self.Submitted.wSecond,
            self.Submitted.wMilliseconds,
        )
    }

    fn get_processed_at(&self) -> Option<std::time::SystemTime> {
        Some(self.get_created_at())
    }

    fn get_completed_at(&self) -> Option<std::time::SystemTime> {
        Some(self.get_created_at())
    }
}

/**
 * Open printer utility
 */
fn open_printer(printer_name: &str) -> Result<*mut c_void, &'static str> {
    let printer_name = str_to_wide_string(printer_name);
    let mut printer_handle = PRINTER_HANDLE::default();

    match unsafe {
        OpenPrinterW(
            PCWSTR(printer_name.as_ptr()),
            &mut printer_handle,
            None
        )
    } {
        Ok(()) => {
            Ok(printer_handle.Value)
        }
        Err(_) => {
            Err("OpenPrinterW failed")
        }
    }
}

/**
 * Print a buffer as RAW datatype with winspool WritePrinterx
 */
pub fn print_buffer(
    printer_name: &str,
    job_name: Option<&str>,
    buffer: &[u8],
    options: &[(&str, &str)],
) -> Result<u64, &'static str> {
    unsafe {
        let printer_handle = open_printer(printer_name);
        if let Err(err) = printer_handle {
            return Err(err);
        }
        let printer_handle = PRINTER_HANDLE {
            Value: printer_handle.unwrap()
        };

        let mut copies = 1;
        let mut data_type = "RAW";

        for option in options {
            match option.0 {
                "copies" => copies = option.1.parse().unwrap_or(copies),
                "document-format" => data_type = option.1,
                _ => {}
            }
        }

        let mut pDatatype = str_to_wide_string(data_type);
        let mut pDocName =
            str_to_wide_string(job_name.unwrap_or(get_current_epoch().to_string().as_str()));

        let doc_info = DOC_INFO_1W {
            pDocName: PWSTR(pDocName.as_mut_ptr()),
            pDatatype: PWSTR(pDatatype.as_mut_ptr()),
            pOutputFile: PWSTR::null(),
        };

        let job_id = StartDocPrinterW(printer_handle, 1, &doc_info);
        if job_id == 0 {
            let _ = ClosePrinter(printer_handle);
            return Err("StartDocPrinterW failed");
        }

        for _ in 0..copies {
            if StartPagePrinter(printer_handle) != BOOL::from(false) {
                let mut bytes_written: c_ulong = 0;
                let _ = WritePrinter(
                    printer_handle,
                    buffer.as_ptr() as *mut c_void,
                    buffer.len() as c_ulong,
                    &mut bytes_written,
                );
                let _ = EndPagePrinter(printer_handle);
            }
        }

        let _ = EndDocPrinter(printer_handle);
        let _ = ClosePrinter(printer_handle);

        Ok(job_id as u64)
    }
}

/**
 * Retrieve print jobs of a specific printer with EnumJobsW
 */
pub fn enum_printer_jobs(printer_name: &str) -> Result<Vec<PrinterJob>, &'static str> {
    let printer_handle = open_printer(printer_name)?;
    let printer_handle = PRINTER_HANDLE {
        Value: printer_handle
    };

    let mut bytes_needed: u32 = 0;
    let mut jobs_count: u32 = 0;

    // First call to determine the required buffer size
    let first_call_result = unsafe {
        EnumJobsW(
            printer_handle,
            0,
            0xFFFFFFFF,
            1,
            None,
            &mut bytes_needed,
            &mut jobs_count,
        )
    };

    if first_call_result.is_err() || bytes_needed == 0 {
        let _ = unsafe { ClosePrinter(printer_handle) };
        return Ok(vec![]);
    }

    // Allocate memory based on bytes_needed
    let mut buffer = vec![0u8; bytes_needed as usize];

    // Second call to actually retrieve job info
    let second_call_result = unsafe {
        EnumJobsW(
            printer_handle,
            0,
            0xFFFFFFFF,
            1,
            Some(buffer.as_mut()),
            &mut bytes_needed,
            &mut jobs_count,
        )
    };

    let _ = unsafe { ClosePrinter(printer_handle) };

    if second_call_result.is_err() {
        return Err("EnumJobsW failed");
    }

    // Convert raw buffer into Vec<JOB_INFO_1W>
    let jobs: &[JOB_INFO_1W] = unsafe {
        slice::from_raw_parts(buffer.as_ptr() as *const JOB_INFO_1W, jobs_count as usize)
    };

    let jobs: Vec<PrinterJob> = jobs.iter().map(|job| PrinterJob::from_platform_printer_job_getters(job)).collect();
    Ok(jobs)
}

/**
 * Change job state
 */
pub fn set_job_state(printer_name: &str, command: u64, job_id: u64) -> Result<(), &'static str> {
    unsafe {
        let printer_handle = open_printer(printer_name)?;
        let printer_handle = PRINTER_HANDLE {
            Value: printer_handle
        };

        let result = SetJobW(
            printer_handle,
            job_id as c_ulong,
            0,
            None,
            command as c_ulong,
        );

        let _ = ClosePrinter(printer_handle);

        if result == BOOL::from(false) {
            Err("SetJobW failed")
        } else {
            Ok(())
        }
    }
}
