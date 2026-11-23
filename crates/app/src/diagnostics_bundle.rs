use anyhow::Result;
use serde::Serialize;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use crate::state::AppCore;

/// Redacts sensitive information from log or configuration text, including the local OS username,
/// home directory path, authkey parameters, and gacha URLs.
pub fn redact_text(input: &str) -> String {
    let mut text = input.to_string();

    // 1. Scrub Home directory
    if let Some(home) = dirs::home_dir() {
        let home_str = home.to_string_lossy().to_string();
        if !home_str.is_empty() {
            #[cfg(target_os = "windows")]
            let home_replacement = "%USERPROFILE%";
            #[cfg(not(target_os = "windows"))]
            let home_replacement = "~";
            text = text.replace(&home_str, home_replacement);
        }
    }

    // 2. Scrub Username
    if let Ok(user) = std::env::var("USER").or_else(|_| std::env::var("USERNAME")) {
        if user.len() >= 2 && user != "root" && user != "default" {
            text = text.replace(&user, "[REDACTED_USER]");
        }
    }

    // 3. Scrub authkey=... query parameters
    if let Ok(re_authkey) = regex::Regex::new(r#"(?i)authkey=[^&\s"'\\]+"#) {
        text = re_authkey
            .replace_all(&text, "authkey=[REDACTED]")
            .to_string();
    }

    // 4. Scrub gacha URLs
    if let Ok(re_gacha) =
        regex::Regex::new(r#"(?i)https?://[^\s"'<>\\]*(?:gacha|webcaches|event)[^\s"'<>\\]*"#)
    {
        text = re_gacha
            .replace_all(&text, "[REDACTED_GACHA_URL]")
            .to_string();
    }

    text
}

#[derive(Debug, Serialize)]
pub struct SystemInfoReport {
    pub yago_version: String,
    pub os: String,
    pub arch: String,
    pub active_runners_count: usize,
    pub games: Vec<GameSummary>,
}

#[derive(Debug, Serialize)]
pub struct GameSummary {
    pub id: String,
    pub install_status: String,
    pub total_mods: usize,
    pub enabled_mods: usize,
}

/// Generates a sanitized .zip bug report bundle containing redacted logs, settings, and system metrics.
pub async fn create_bug_report_bundle(core: &AppCore, dest_zip: &Path) -> Result<PathBuf> {
    if let Some(parent) = dest_zip.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let file = File::create(dest_zip)?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    // 1. system_info.json (No mod names or user personal paths)
    let dbs = core.game_dbs.lock().await;
    let mut game_summaries = Vec::new();
    for (gid, db) in dbs.iter() {
        let status = db
            .games
            .get(gid)
            .map(|g| format!("{:?}", g.install_status))
            .unwrap_or_else(|| "Unknown".to_string());
        let total_mods = db.mods.len();
        let enabled_mods = db
            .games
            .get(gid)
            .and_then(|g| {
                db.profiles
                    .get(&uuid::Uuid::parse_str(&g.active_profile_id).unwrap_or_default())
            })
            .map(|p| p.enabled_mod_ids.len())
            .unwrap_or(0);

        game_summaries.push(GameSummary {
            id: gid.clone(),
            install_status: status,
            total_mods,
            enabled_mods,
        });
    }

    let runners_count = core.list_runners().await.map(|r| r.len()).unwrap_or(0);

    let system_info = SystemInfoReport {
        yago_version: env!("CARGO_PKG_VERSION").to_string(),
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        active_runners_count: runners_count,
        games: game_summaries,
    };
    drop(dbs);

    let sys_bytes = serde_json::to_vec_pretty(&system_info)?;
    zip.start_file("system_info.json", options)?;
    zip.write_all(&sys_bytes)?;

    // 2. settings_redacted.json
    let settings = core.get_settings().await.unwrap_or_default();
    let settings_str = serde_json::to_string_pretty(&settings)?;
    let redacted_settings = redact_text(&settings_str);
    zip.start_file("settings_redacted.json", options)?;
    zip.write_all(redacted_settings.as_bytes())?;

    // 3. Up to 3 most recent yago.log* files and any crash-*.log files from logs/
    let logs_dir = core.app_data_dir.join("logs");
    if logs_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&logs_dir) {
            let mut yago_logs = Vec::new();
            let mut crash_logs = Vec::new();

            for entry in entries.filter_map(|e| e.ok()) {
                let p = entry.path();
                if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
                    if name.starts_with("yago.log") {
                        yago_logs.push(p);
                    } else if name.starts_with("crash-") {
                        crash_logs.push(p);
                    }
                }
            }

            yago_logs.sort();
            let recent_yago: Vec<_> = yago_logs.iter().rev().take(3).cloned().collect();
            for log_path in recent_yago.iter().chain(crash_logs.iter()) {
                if let Ok(content) = std::fs::read_to_string(log_path) {
                    let redacted = redact_text(&content);
                    let file_name = log_path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("log.txt");
                    zip.start_file(format!("logs/{}", file_name), options)?;
                    zip.write_all(redacted.as_bytes())?;
                }
            }
        }
    }

    // 4. Tail (last 200 lines) of d3d11_log.txt for installed games if present
    let dbs = core.game_dbs.lock().await;
    for (gid, db) in dbs.iter() {
        if let Some(config) = db.games.get(gid) {
            let d3d_path = config.install_path.join("d3d11_log.txt");
            if d3d_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&d3d_path) {
                    let lines: Vec<&str> = content.lines().collect();
                    let tail_start = lines.len().saturating_sub(200);
                    let tail_lines = lines[tail_start..].join("\n");
                    let redacted = redact_text(&tail_lines);
                    zip.start_file(format!("logs/d3d11_log_{}.txt", gid), options)?;
                    zip.write_all(redacted.as_bytes())?;
                }
            }
        }
    }
    drop(dbs);

    zip.finish()?;
    Ok(dest_zip.to_path_buf())
}

impl AppCore {
    /// Creates and exports a redacted bug report archive to dest_zip, defaulting to
    /// `app_data_dir/logs/yago-bug-report-<timestamp>.zip` if unspecified.
    pub async fn export_bug_report(&self, dest_zip: Option<String>) -> Result<String, String> {
        let out_path = if let Some(p) = dest_zip {
            PathBuf::from(p)
        } else {
            let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
            let logs_dir = self.app_data_dir.join("logs");
            let _ = std::fs::create_dir_all(&logs_dir);
            logs_dir.join(format!("yago-bug-report-{}.zip", timestamp))
        };

        let result_path = create_bug_report_bundle(self, &out_path)
            .await
            .map_err(|e| e.to_string())?;

        Ok(result_path.display().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init::init_with_dir;
    use tempfile::tempdir;

    #[test]
    fn test_redact_text_scrubs_sensitive_data() {
        let mut sample =
            "Error occurred at /dummy/path with authkey=deadbeef123456&region=os_usa".to_string();
        if let Some(home) = dirs::home_dir() {
            sample = format!(
                "Path in user home: {}/Games/Genshin. authkey=secret_12345",
                home.display()
            );
        }

        let redacted = redact_text(&sample);
        assert!(!redacted.contains("authkey=secret_12345"));
        assert!(redacted.contains("authkey=[REDACTED]"));

        if let Some(home) = dirs::home_dir() {
            assert!(!redacted.contains(&home.to_string_lossy().to_string()));
        }

        let gacha_url = "Fetch failed: https://hk4e-api-os.hoyoverse.com/event/gacha_info/api/getGachaLog?authkey=secret_token&lang=en";
        let redacted_url = redact_text(gacha_url);
        assert!(!redacted_url.contains("secret_token"));
        assert!(redacted_url.contains("[REDACTED_GACHA_URL]"));
    }

    #[tokio::test]
    async fn test_create_bug_report_bundle() {
        let dir = tempdir().unwrap();
        let core = init_with_dir(dir.path().to_path_buf()).await.unwrap();

        let zip_path = dir.path().join("test_report.zip");
        let result_path = create_bug_report_bundle(&core, &zip_path).await.unwrap();
        assert_eq!(result_path, zip_path);
        assert!(zip_path.exists());

        // Verify zip contains system_info.json and settings_redacted.json
        let file = File::open(&zip_path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();

        assert!(archive.by_name("system_info.json").is_ok());
        assert!(archive.by_name("settings_redacted.json").is_ok());
    }
}
