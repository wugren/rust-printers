use printers::get_printers;

fn main() {
    let printers = get_printers();
    if printers.is_empty() {
        println!("No printers found.");
        return;
    }

    println!("Found {} printer(s):", printers.len());
    for (index, printer) in printers.iter().enumerate() {
        println!(
            "{}. name='{}', system_name='{}', state={:?}, location='{}', driver='{}'",
            index + 1,
            printer.name,
            printer.system_name,
            printer.state,
            printer.location,
            printer.driver_name
        );
        let caps = printer.get_printer_caps();
        println!("  Capabilities: {:?}", caps);
    }
}
