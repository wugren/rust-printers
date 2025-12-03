#![allow(non_snake_case)]
#![allow(non_camel_case_types)]

use libc::{c_int, c_uint, c_ulong, c_void, wchar_t};
use std::{ptr, slice};
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Graphics::Gdi::{CreateDCW, DeleteDC, GetDeviceCaps, DEVMODEW, HORZRES, LOGPIXELSX, LOGPIXELSY, PHYSICALHEIGHT, PHYSICALOFFSETX, PHYSICALOFFSETY, PHYSICALWIDTH, VERTRES};
use windows::Win32::Graphics::Printing::{EnumPrintersW, GetDefaultPrinterW, PRINTER_INFO_2W};
use windows::Win32::Storage::Xps::{DeviceCapabilitiesW, DC_FIELDS, DC_ORIENTATION, DC_PAPERS, DC_SIZE, PRINTER_DEVICE_CAPABILITIES};
use crate::{
    common::traits::platform::PlatformPrinterGetters,
    windows::utils::{
        memory::{alloc_s, dealloc_s},
        strings::{str_to_wide_string, wchar_t_to_string},
    },
};
use crate::common::base::printer::Printer;
use crate::common::traits::platform::DeviceCaps;

impl PlatformPrinterGetters for PRINTER_INFO_2W {
    fn get_name(&self) -> String {
        wchar_t_to_string(self.pPrinterName)
    }
    fn get_is_default(&self) -> bool {
        let mut name_size: c_ulong = 0;
        unsafe {
            GetDefaultPrinterW(None, &mut name_size);
            let mut buffer: Vec<u16> = vec![0; name_size as usize];
            GetDefaultPrinterW(Some(PWSTR(buffer.as_mut_ptr())), &mut name_size);
            wchar_t_to_string(self.pPrinterName) == wchar_t_to_string(PWSTR(buffer.as_mut_ptr()))
        }
    }
    fn get_system_name(&self) -> String {
        wchar_t_to_string(self.pPrinterName)
    }
    fn get_marker_and_model(&self) -> String {
        wchar_t_to_string(self.pDriverName)
    }
    fn get_is_shared(&self) -> bool {
        (self.Attributes & 0x00000008) == 8
    }
    fn get_uri(&self) -> String {
        "".to_string()
    }
    fn get_location(&self) -> String {
        wchar_t_to_string(self.pLocation)
    }
    fn get_state(&self) -> u64 {
        self.Status as u64
    }
    fn get_port_name(&self) -> String {
        wchar_t_to_string(self.pPortName)
    }
    fn get_processor(&self) -> String {
        wchar_t_to_string(self.pPrintProcessor)
    }
    fn get_description(&self) -> String {
        wchar_t_to_string(self.pComment)
    }
    fn get_data_type(&self) -> String {
        wchar_t_to_string(self.pDatatype)
    }
    fn get_state_reasons(&self) -> Vec<String> {
        // NOTE: These reasons are virtual descriptions based on printer status
        return [
            (0x00000000, "ready"),
            (0x00000001, "paused"),
            (0x00000002, "error"),
            (0x00000004, "pending_deletion"),
            (0x00000008, "paper_jam"),
            (0x00000010, "paper_out"),
            (0x00000020, "manual_feed"),
            (0x00000040, "paper_problem"),
            (0x00000080, "offline"),
            (0x00000100, "io_active"),
            (0x00000200, "busy"),
            (0x00000400, "printing"),
            (0x00000800, "output_bin_full"),
            (0x00001000, "not_available"),
            (0x00002000, "waiting"),
            (0x00004000, "processing"),
            (0x00008000, "initializing"),
            (0x00010000, "warming_up"),
            (0x00020000, "toner_low"),
            (0x00040000, "no_toner"),
            (0x00080000, "page_punt"),
            (0x00100000, "user_intervention"),
            (0x00200000, "out_of_memory"),
            (0x00400000, "door_open"),
            (0x00800000, "server_unknown"),
            (0x01000000, "power_save"),
        ]
        .iter()
        .filter(|v| self.Status & v.0 != 0)
        .map(|v| v.1.to_string())
        .collect();
    }

    fn get_device_caps(&self) -> DeviceCaps {
        get_device_caps(self.get_name().as_str())
    }
}

//获取打印机的dpi
pub fn get_device_caps(printer_name: &str) -> DeviceCaps {
    let printer_name_wide = str_to_wide_string(printer_name);
    let device = str_to_wide_string("WINSPOOL");
    let device_name = PCWSTR(printer_name_wide.as_ptr());
    let port_name = PCWSTR::null(); // 使用默认端口
    unsafe {
        let hdc = CreateDCW(PCWSTR(device.as_ptr()), device_name, port_name, None);
        let dpi_x = GetDeviceCaps(Some(hdc), LOGPIXELSX);  // 水平 DPI
        let dpi_y = GetDeviceCaps(Some(hdc), LOGPIXELSY);
        let page_width = GetDeviceCaps(Some(hdc), PHYSICALWIDTH);
        let page_height = GetDeviceCaps(Some(hdc), PHYSICALHEIGHT);
        let print_table_width = GetDeviceCaps(Some(hdc), HORZRES);
        let print_table_height = GetDeviceCaps(Some(hdc), VERTRES);
        let margin_left = GetDeviceCaps(Some(hdc), PHYSICALOFFSETX);
        let margin_top = GetDeviceCaps(Some(hdc), PHYSICALOFFSETY);
        let margin_right = page_width - print_table_width - margin_left;
        let margin_bottom = page_height - print_table_height - margin_top;
        DeleteDC(hdc);
        DeviceCaps {
            dpi_x,
            dpi_y,
            page_width,
            page_height,
            print_table_width,
            print_table_height,
            margin_top,
            margin_left,
            margin_right,
            margin_bottom,
        }
    }
}

/**
 * Returns all available printer using EnumPrintersW
 */
pub fn enum_printers(name: Option<&str>) -> Vec<Printer> {
    let mut bytes_needed: u32 = 0;
    let mut count_printers: u32 = 0;

    // Store wide name in a variable so it lives long enough
    let name_wide: Option<Vec<u16>> = name.map(str_to_wide_string);
    let name_ptr = match &name_wide {
        Some(vec) => vec.as_ptr(),
        None => ptr::null(),
    };

    let result = unsafe {
        EnumPrintersW(
            0x00000002 | 0x00000004,
            PCWSTR(name_ptr),
            2,
            None,
            &mut bytes_needed,
            &mut count_printers,
        )
    };

    if result.is_ok() || bytes_needed == 0 {
        return vec![];
    }

    let mut buffer = vec![0u8; bytes_needed as usize];

    let result = unsafe {
        EnumPrintersW(
            0x00000002 | 0x00000004,
            PCWSTR(name_ptr),
            2,
            Some(buffer.as_mut()),
            &mut bytes_needed,
            &mut count_printers,
        )
    };
    if result.is_err() {
        return vec![];
    }


    let printers = unsafe {
        slice::from_raw_parts(buffer.as_ptr() as *const PRINTER_INFO_2W, count_printers as usize)
    };
    printers.iter().map(|p| Printer::from_platform_printer_getters(p)).collect()
}

pub fn get_default_printer_name() -> String {
    let mut name_size: u32 = 0;
    unsafe {
        GetDefaultPrinterW(None, &mut name_size);
        let mut buffer: Vec<u16> = vec![0; name_size as usize];
        GetDefaultPrinterW(Some(PWSTR(buffer.as_mut_ptr())), &mut name_size);
        wchar_t_to_string(PWSTR(buffer.as_mut_ptr()))
    }
}
/**
 * Returns the default printer filtering all printers
 */
pub fn get_default_printer() -> Option<Printer> {
    let printer_name = get_default_printer_name();
    enum_printers(None).into_iter().find(|p| p.name == printer_name)
}
