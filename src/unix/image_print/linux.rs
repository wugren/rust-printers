use image::DynamicImage;

use crate::common::{
    base::job::PrinterJobOptions,
    traits::platform::PlatformActions,
};

pub fn print_image(
    printer_system_name: &str,
    image: DynamicImage,
    print_name: Option<&str>,
    page_count: u32,
    print_width: Option<f64>,
    print_height: Option<f64>,
) -> Result<u64, &'static str> {
    let image_bytes = super::image_to_png_bytes(&image)?;
    let copies = super::normalize_page_count(page_count).to_string();

    let mut owned_options = vec![
        (String::from("document-format"), String::from("image/png")),
        (String::from("copies"), copies),
        (String::from("print-scaling"), String::from("fit")),
    ];

    if let (Some(width), Some(height)) = (print_width, print_height)
        && let Some(media) = super::media_custom_mm(width, height)
    {
        owned_options.push((String::from("media"), media));
    }

    let raw_properties: Vec<(&str, &str)> = owned_options
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();

    <crate::Platform as PlatformActions>::print(
        printer_system_name,
        &image_bytes,
        PrinterJobOptions {
            name: print_name,
            raw_properties: &raw_properties,
        },
    )
}
