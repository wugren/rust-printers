use image::{ImageReader};
use printers::get_printers;

fn main() {
    sfo_log::Logger::new("test-sys-printer").set_log_to_file(true).start().unwrap();

    let start = std::time::Instant::now();
    let printers = get_printers();
    log::info!("spend time {}", start.elapsed().as_millis());
    for printer in printers {
        log::info!("{:?}", printer);
        let caps = printer.get_printer_caps();
        log::info!("{:?}", caps);
        if printer.name == "JK-402A" {
            let image = ImageReader::open("C:\\work\\rust-printers\\cover_1765814682178_20251216000441A096.png").unwrap();
            let image = image.decode().unwrap();
            let print_height = image.height() as f64 / 8f64 * 10f64;
            let print_width = image.width() as f64 / 8f64;
            let _ = printer.print_image(image.clone(), None, 1, None, Some(print_height));
        }
    }
}
