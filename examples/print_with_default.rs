use printers::common::base::job::PrinterJobOptions;
use printers::get_default_printer;

fn main() {
    let Some(printer) = get_default_printer() else {
        eprintln!("No default printer found.");
        return;
    };

    println!("Using default printer: {}", printer.name);

    let content = b"Hello from rust-printers!\n";
    match printer.print(content, PrinterJobOptions::none()) {
        Ok(job_id) => println!("Print job submitted, job_id={job_id}"),
        Err(err) => eprintln!("Failed to print: {err}"),
    }
}
