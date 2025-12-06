use std::mem;
use image::DynamicImage;
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Graphics::Gdi::{CreateCompatibleBitmap, CreateCompatibleDC, CreateDCW, DeleteDC, DeleteObject, GetDeviceCaps, SelectObject, SetDIBits, SetStretchBltMode, StretchBlt, BITMAPINFO, BITMAPINFOHEADER, DEVMODEW, DIB_RGB_COLORS, DM_OUT_BUFFER, DM_PAPERLENGTH, DM_PAPERWIDTH, HALFTONE, HGDIOBJ, HORZRES, LOGPIXELSY, PHYSICALOFFSETX, PHYSICALOFFSETY, RGBQUAD, SRCCOPY, VERTRES};
use windows::Win32::Graphics::Printing::{ClosePrinter, DocumentPropertiesW, EndDocPrinter, EndPagePrinter, OpenPrinterW, StartDocPrinterW, StartPagePrinter, DOC_INFO_1W, PRINTER_HANDLE};
use windows::Win32::Storage::Xps::{EndDoc, EndPage, StartDocW, StartPage, DOCINFOW};
use windows::Win32::UI::WindowsAndMessaging::IDOK;
use crate::common::base::job::{PrinterJobOptions, PrinterJobState};
use crate::common::base::printer::PrinterState;
use crate::common::base::{job::PrinterJob, printer::Printer};
use crate::common::traits::platform::{DeviceCaps, PlatformActions, PlatformPrinterGetters};
use crate::windows::utils::strings::str_to_wide_string;
use crate::windows::winspool::info::get_device_caps;

mod utils;
mod winspool;

impl PlatformActions for crate::Platform {
    fn get_printers() -> Vec<Printer> {
         winspool::info::enum_printers(None)
    }

    fn get_printer_caps(printer_system_name: &str) -> DeviceCaps {
        get_device_caps(printer_system_name)
    }

    fn print(
        printer_system_name: &str,
        buffer: &[u8],
        options: PrinterJobOptions,
    ) -> Result<u64, &'static str> {
        winspool::jobs::print_buffer(
            printer_system_name,
            options.name,
            buffer,
            options.raw_properties,
        )
    }

    fn print_file(
        printer_system_name: &str,
        file_path: &str,
        options: PrinterJobOptions,
    ) -> Result<u64, &'static str> {
        let buffer = utils::file::get_file_as_bytes(file_path);
        if buffer.is_some() {
            Self::print(printer_system_name, &buffer.unwrap(), options)
        } else {
            Err("failed to read file")
        }
    }

    fn print_image(
        printer_system_name: &str,
        image: DynamicImage,
        print_name: Option<&str>,
        page_count: u32,
        print_width: Option<f64>,
        print_height: Option<f64>,
    ) -> Result<u64, &'static str> {
        let printer_name_wide = str_to_wide_string(printer_system_name);
        let mut printer_handle = PRINTER_HANDLE::default();

        let result = unsafe {
            OpenPrinterW(
                PCWSTR(printer_name_wide.as_ptr()),
                &mut printer_handle,
                None
            )
        };

        if result.is_err() {
            return Err("Failed to open printer");
        }

        // 将DynamicImage转换为BGRA格式
        let rgba_image = image.to_rgba8();
        let (img_width, img_height) = rgba_image.dimensions();

        // 创建设备上下文
        let device = str_to_wide_string("WINSPOOL");
        let hdc = unsafe {
            if print_height.is_some() || print_width.is_some() {
                let size_needed = DocumentPropertiesW(None, printer_handle, PCWSTR(printer_name_wide.as_ptr()), None, None, 0);
                if size_needed <= 0 {
                    return Err("Failed to get device mode size");
                }
                
                let mut devmode_buffer = vec![0u8; size_needed as usize];
                let devmode_ptr = devmode_buffer.as_mut_ptr() as *mut DEVMODEW;
                let result = DocumentPropertiesW(None, printer_handle, PCWSTR(printer_name_wide.as_ptr()), Some(devmode_ptr), None, DM_OUT_BUFFER.0);
                if result != IDOK.0 {
                    return Err("Failed to get device mode");
                }
                let devmode = &mut *devmode_ptr;
                if let Some(height) = print_height {
                    devmode.dmFields = DM_PAPERLENGTH;
                    devmode.Anonymous1.Anonymous1.dmPaperLength = (height * 10f64) as i16;
                }
                if let Some(width) = print_width {
                    devmode.dmFields |= DM_PAPERWIDTH;
                    devmode.Anonymous1.Anonymous1.dmPaperWidth = (width * 10f64) as i16;
                }
                CreateDCW(PCWSTR(device.as_ptr()), PCWSTR(printer_name_wide.as_ptr()), PCWSTR::null(), Some(devmode_ptr))
            } else {
                CreateDCW(PCWSTR(device.as_ptr()), PCWSTR(printer_name_wide.as_ptr()), PCWSTR::null(), None)
            }
        };

        if hdc.is_invalid() {
            let _ = unsafe { ClosePrinter(printer_handle) };
            return Err("Failed to create device context");
        }

        // 获取打印机分辨率
        let width = unsafe { GetDeviceCaps(Some(hdc), HORZRES) };
        let height = unsafe { GetDeviceCaps(Some(hdc), VERTRES) };

        // 开始文档
        let mut doc_name = utils::strings::str_to_wide_string(
            print_name.unwrap_or("Image Print Job")
        );

        let doc_info = DOCINFOW {
            cbSize: 0,
            lpszDocName: PCWSTR(doc_name.as_mut_ptr()),
            lpszOutput: Default::default(),
            lpszDatatype: Default::default(),
            fwType: 0,
        };
        let job_id = unsafe {
            StartDocW(hdc, &doc_info)
        };
        if job_id == 0 {
            unsafe {
                let _ = DeleteDC(hdc);
                let _ = ClosePrinter(printer_handle);
            }
            return Err("Failed to start document");
        }

        for _ in 0..page_count {
            unsafe {
                let _ = StartPage(hdc);
            };

            // 创建兼容的内存DC
            let mem_dc = unsafe { CreateCompatibleDC(Some(hdc)) };
            if mem_dc.is_invalid() {
                // 清理资源
                unsafe {
                    let _ = EndPage(hdc);
                    let _ = EndDoc(hdc);
                    let _ = DeleteDC(hdc);
                    let _ = ClosePrinter(printer_handle);
                }
                return Err("Failed to create compatible DC");
            }

            // 创建兼容的位图
            let bitmap = unsafe {
                CreateCompatibleBitmap(hdc, img_width as i32, img_height as i32)
            };
            if bitmap.is_invalid() {
                unsafe {
                    let _ = EndDoc(hdc);
                    let _ = DeleteDC(mem_dc);
                    let _ = DeleteDC(hdc);
                    let _ = ClosePrinter(printer_handle);
                }
                return Err("Failed to create compatible bitmap");
            }

            // 选择位图到内存DC
            let old_bitmap = unsafe { SelectObject(mem_dc, HGDIOBJ::from(bitmap)) };

            // 设置位图信息
            let bi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: img_width as i32,
                    biHeight: -(img_height as i32), // 负值表示顶部到底部的扫描线
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: 0, // BI_RGB
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [RGBQUAD::default(); 1],
            };

            // 将图像数据设置到位图中
            let dib_result = unsafe {
                SetDIBits(
                    Some(mem_dc),
                    bitmap,
                    0,
                    img_height,
                    rgba_image.as_ptr() as *const std::ffi::c_void,
                    &bi,
                    DIB_RGB_COLORS,
                )
            };

            if dib_result == 0 {
                unsafe {
                    SelectObject(mem_dc, old_bitmap);
                    let _ = DeleteObject(HGDIOBJ::from(bitmap));
                    let _ = DeleteDC(mem_dc);
                    let _ = EndPage(hdc);
                    let _ = EndDoc(hdc);
                    let _ = DeleteDC(hdc);
                    let _ = ClosePrinter(printer_handle);
                }
                return Err("Failed to set DIB bits");
            }

            // 计算居中位置
            let x_pos = (width - img_width as i32) / 2;
            let y_pos = 0; // 置顶

            // 设置拉伸模式
            unsafe { SetStretchBltMode(hdc, HALFTONE) };

            // 绘制图像到打印机DC
            let stretch_result = unsafe {
                StretchBlt(
                    hdc,
                    x_pos,
                    y_pos,
                    img_width as i32,
                    img_height as i32,
                    Some(mem_dc),
                    0,
                    0,
                    img_width as i32,
                    img_height as i32,
                    SRCCOPY,
                )
            };

            if !stretch_result.as_bool() {
                unsafe {
                    SelectObject(mem_dc, old_bitmap);
                    let _ = DeleteObject(HGDIOBJ::from(bitmap));
                    let _ = DeleteDC(mem_dc);
                    let _ = EndPage(hdc);
                    let _ = EndDoc(hdc);
                    let _ = DeleteDC(hdc);
                    let _ = ClosePrinter(printer_handle);
                }
                return Err("Failed to stretch blit image");
            }
            unsafe {
                SelectObject(mem_dc, old_bitmap);
                let _ = DeleteObject(HGDIOBJ::from(bitmap));
                let _ = DeleteDC(mem_dc);
                let _ = EndPage(hdc);
            }
        }

        // 清理GDI对象
        unsafe {
            let _ = EndDoc(hdc);
            let _ = DeleteDC(hdc);
            let _ = ClosePrinter(printer_handle);
        }

        Ok(job_id as u64)
    }

    fn get_printer_jobs(printer_name: &str, active_only: bool) -> Vec<PrinterJob> {
        winspool::jobs::enum_printer_jobs(printer_name)
            .unwrap_or_default()
            .into_iter()
            .filter(|j| {
                if active_only {
                    j.state == PrinterJobState::PENDING
                        || j.state == PrinterJobState::PROCESSING
                        || j.state == PrinterJobState::PAUSED
                } else {
                    true
                }
            })
            .collect()
    }

    fn get_default_printer() -> Option<Printer> {
        winspool::info::get_default_printer()
    }

    fn get_printer_by_name(name: &str) -> Option<Printer> {
        winspool::info::enum_printers(None)
            .into_iter()
            .find(|p| p.name == name || p.system_name == name)
    }

    fn parse_printer_state(platform_state: u64, state_reasons: &str) -> PrinterState {
        if state_reasons.contains("offline") || state_reasons.contains("pending_deletion") {
            return PrinterState::OFFLINE;
        }

        match platform_state {
            s if s == 0 || s & (0x00000100 | 0x00004000) != 0 => PrinterState::READY,
            s if s & 0x00000400 != 0 => PrinterState::PRINTING,
            s if s & (0x00000001 | 0x00000002 | 0x00000008 | 0x00000010 | 0x00000020) != 0 => {
                PrinterState::PAUSED
            }
            s if s & (0x00000080 | 0x00400000 | 0x00001000 | 0x00000004) != 0 => {
                PrinterState::OFFLINE
            }
            _ => PrinterState::UNKNOWN,
        }
    }

    fn parse_printer_job_state(platform_state: u64) -> PrinterJobState {
        match platform_state {
            1 | 8 => PrinterJobState::PAUSED,
            4 | 256 => PrinterJobState::CANCELLED,
            16 | 2048 | 8192 => PrinterJobState::PROCESSING,
            32 | 64 | 512 | 1024 => PrinterJobState::PENDING,
            128 | 496 => PrinterJobState::COMPLETED,
            _ => PrinterJobState::UNKNOWN,
        }
    }

    fn set_job_state(
        printer_name: &str,
        job_id: u64,
        state: PrinterJobState,
    ) -> Result<(), &'static str> {
        return match state {
            PrinterJobState::PAUSED => winspool::jobs::set_job_state(printer_name, 1, job_id),
            PrinterJobState::PENDING => winspool::jobs::set_job_state(printer_name, 4, job_id),
            PrinterJobState::CANCELLED => winspool::jobs::set_job_state(printer_name, 5, job_id),
            PrinterJobState::PROCESSING => winspool::jobs::set_job_state(printer_name, 2, job_id),
            _ => Err("Operation canot be defined"),
        };
    }
}
