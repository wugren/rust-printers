use std::env;

use printers::get_printer_by_name;

const MM_PER_INCH: f64 = 25.4;
const DEFAULT_DPI: i32 = 300;

fn main() {
    // Usage:
    // cargo run --example print_image_by_name -- "Your Printer Name" "/path/to/image.png"
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: {} <printer_name> <image_path>", args[0]);
        return;
    }

    let printer_name = &args[1];
    let image_path = &args[2];

    let Some(printer) = get_printer_by_name(printer_name) else {
        eprintln!("Printer not found: {printer_name}");
        return;
    };

    let image = match image::open(image_path) {
        Ok(image) => image,
        Err(err) => {
            eprintln!("Failed to open image file: {err}");
            return;
        }
    };

    let caps = printer.get_printer_caps();
    let dpi_x = if caps.dpi_x > 0 {
        caps.dpi_x
    } else {
        DEFAULT_DPI
    };
    let dpi_y = if caps.dpi_y > 0 {
        caps.dpi_y
    } else {
        DEFAULT_DPI
    };

    let print_width = px_to_mm(image.width(), dpi_x);
    let print_height = px_to_mm(image.height(), dpi_y);

    println!("Using printer: {} ({})", printer.name, printer.system_name);
    println!(
        "DPI: {}x{}, image: {}x{} px, size: {:.2}x{:.2} mm",
        dpi_x,
        dpi_y,
        image.width(),
        image.height(),
        print_width,
        print_height
    );

    match printer.print_image(
        image,
        Some("Rust image print job"),
        1,
        Some(print_width),
        Some(print_height),
    ) {
        Ok(job_id) => println!("Image print submitted, job_id={job_id}"),
        Err(err) => eprintln!("Failed to print image: {err}"),
    }
}

fn px_to_mm(px: u32, dpi: i32) -> f64 {
    (px as f64 / dpi as f64) * MM_PER_INCH
}
