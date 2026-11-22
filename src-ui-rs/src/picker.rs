use std::path::PathBuf;

pub async fn pick_folder(title: &str) -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .set_title(title)
        .pick_folder()
        .await
        .map(|h| h.path().to_path_buf())
}

pub async fn pick_file(title: &str) -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .set_title(title)
        .pick_file()
        .await
        .map(|h| h.path().to_path_buf())
}

pub async fn pick_save_file(title: &str, default_name: &str) -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .set_title(title)
        .set_file_name(default_name)
        .add_filter("ZIP Archive", &["zip"])
        .save_file()
        .await
        .map(|h| h.path().to_path_buf())
}

pub fn format_size(bytes: u64) -> String {
    if bytes == 0 {
        return "0 B".to_string();
    }
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let i = ((bytes as f64).log(1024.0) as usize).min(UNITS.len() - 1);
    format!("{:.2} {}", bytes as f64 / 1024f64.powi(i as i32), UNITS[i])
}
