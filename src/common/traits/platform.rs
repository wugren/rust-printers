use crate::common::base::{
    job::{PrinterJobOptions, PrinterJobState},
    printer::{Printer, PrinterState},
};
use std::time::SystemTime;
use image::DynamicImage;

#[derive(Clone, Debug)]
pub struct DeviceCaps {
    pub dpi_x: i32,
    pub dpi_y: i32,
    pub page_width: i32,
    pub page_height: i32,
    pub print_table_width: i32,
    pub print_table_height: i32,
    pub margin_top: i32,
    pub margin_left: i32,
    pub margin_right: i32,
    pub margin_bottom: i32,
}

pub trait PlatformPrinterGetters {
    fn get_name(&self) -> String;
    fn get_system_name(&self) -> String;
    fn get_marker_and_model(&self) -> String;
    fn get_is_shared(&self) -> bool;
    fn get_uri(&self) -> String;
    fn get_location(&self) -> String;
    fn get_state(&self) -> u64;
    fn get_state_reasons(&self) -> Vec<String>;
    fn get_port_name(&self) -> String;
    fn get_processor(&self) -> String;
    fn get_description(&self) -> String;
    fn get_data_type(&self) -> String;
}

pub trait PlatformPrinterJobGetters {
    fn get_id(&self) -> u64;
    fn get_name(&self) -> String;
    fn get_state(&self) -> u64;
    fn get_printer(&self) -> String;
    fn get_media_type(&self) -> String;
    fn get_created_at(&self) -> SystemTime;
    fn get_processed_at(&self) -> Option<SystemTime>;
    fn get_completed_at(&self) -> Option<SystemTime>;
}

pub trait PlatformActions {
    fn get_printers() -> Vec<Printer>;

    fn get_printer_caps(printer_system_name: &str) -> DeviceCaps;
    fn print(
        printer_system_name: &str,
        buffer: &[u8],
        options: PrinterJobOptions,
    ) -> Result<u64, &'static str>;
    fn print_file(
        printer_system_name: &str,
        file_path: &str,
        options: PrinterJobOptions,
    ) -> Result<u64, &'static str>;
    fn print_image(
        printer_system_name: &str,
        buffer: DynamicImage,
        print_name: Option<&str>,
        page_count: u32,
        print_width: Option<f64>,
        print_height: Option<f64>,
    ) -> Result<u64, &'static str>;
    fn get_printer_jobs(
        printer_name: &str,
        active_only: bool,
    ) -> Vec<crate::common::base::job::PrinterJob>;
    fn get_default_printer() -> Option<Printer>;
    fn get_printer_by_name(printer_name: &str) -> Option<Printer>;
    fn parse_printer_state(platform_state: u64, state_reasons: &str) -> PrinterState;
    fn parse_printer_job_state(platform_state: u64) -> PrinterJobState;
    fn set_job_state(
        printer_name: &str,
        job_id: u64,
        state: PrinterJobState,
    ) -> Result<(), &'static str>;
}
