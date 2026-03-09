use std::{
    env,
    fs::File,
    io::Write,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

pub fn save_tmp_file(buffer: &[u8]) -> Option<PathBuf> {
    save_tmp_file_with_ext(buffer, "")
}

pub fn save_tmp_file_with_ext(buffer: &[u8], extension: &str) -> Option<PathBuf> {
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();

    let extension = extension.trim().trim_start_matches('.');
    let file_name = if extension.is_empty() {
        time.to_string()
    } else {
        format!("{time}.{extension}")
    };

    let file_path = env::temp_dir().join(file_name);

    let mut tmp_file = File::create(&file_path).unwrap();
    let save = tmp_file.write(buffer);

    if save.is_ok() {
        Some(file_path)
    } else {
        None
    }
}
