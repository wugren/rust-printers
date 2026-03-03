use printers::common::base::job::PrinterJobOptions;
use printers::get_printer_by_name;
use std::env;

fn main() {
    // Usage:
    // cargo run --example print_file_by_name -- "Your Printer Name" "/path/to/file.txt"
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: {} <printer_name> <file_path>", args[0]);
        return;
    }

    let printer_name = &args[1];
    let file_path = &args[2];

    let Some(printer) = get_printer_by_name(printer_name) else {
        eprintln!("Printer not found: {printer_name}");
        return;
    };

    println!("Using printer: {} ({})", printer.name, printer.system_name);

    let options = PrinterJobOptions {
        name: Some("Rust file print job"),
        raw_properties: &[("copies", "1")],
    };

    match printer.print_file(file_path, options) {
        Ok(job_id) => println!("Print file submitted, job_id={job_id}"),
        Err(err) => eprintln!("Failed to print file: {err}"),
    }
}
