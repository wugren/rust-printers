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
        .as_nanos();

    let extension = extension.trim().trim_start_matches('.');
    let file_name = if extension.is_empty() {
        format!("{}-{time}", std::process::id())
    } else {
        format!("{}-{time}.{extension}", std::process::id())
    };

    let file_path = env::temp_dir().join(file_name);

    let mut tmp_file = File::create(&file_path).ok()?;
    let save = tmp_file.write_all(buffer).and_then(|_| tmp_file.sync_all());

    if save.is_ok() {
        Some(file_path)
    } else {
        None
    }
}
