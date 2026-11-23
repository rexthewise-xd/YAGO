use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub use tracing_appender::non_blocking::WorkerGuard;

/// Configures structured daily-rolling file logging (yago.log) + stderr subscriber,
/// and prunes log files older than 7 days.
pub fn init_logging(app_data_dir: &Path) -> WorkerGuard {
    let logs_dir = app_data_dir.join("logs");
    let _ = fs::create_dir_all(&logs_dir);

    // Prune logs older than 7 days on startup
    prune_old_logs(&logs_dir, Duration::from_secs(7 * 24 * 3600));

    // Daily rolling appender: writes to yago.log in logs/
    let file_appender = tracing_appender::rolling::daily(&logs_dir, "yago.log");
    let (non_blocking_file, guard) = tracing_appender::non_blocking(file_appender);

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let subscriber = tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .with(
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_writer(non_blocking_file),
        );

    let _ = subscriber.try_init();

    guard
}

/// Prunes rolling log files older than `max_age`
fn prune_old_logs(logs_dir: &Path, max_age: Duration) {
    let now = SystemTime::now();
    if let Ok(entries) = fs::read_dir(logs_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if let Some(file_name) = path.file_name().and_then(|s| s.to_str()) {
                if file_name.starts_with("yago.log.") || file_name.starts_with("crash-") {
                    if let Ok(meta) = entry.metadata() {
                        if let Ok(modified) = meta.modified() {
                            if let Ok(age) = now.duration_since(modified) {
                                if age > max_age {
                                    let _ = fs::remove_file(path);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Installs a global panic hook that captures panic details and backtrace to a timestamped crash log
pub fn install_panic_hook(app_data_dir: PathBuf) {
    let prev_hook = std::panic::take_hook();

    std::panic::set_hook(Box::new(move |info| {
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
        let logs_dir = app_data_dir.join("logs");
        let _ = fs::create_dir_all(&logs_dir);
        let crash_log_path = logs_dir.join(format!("crash-{}.log", timestamp));

        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Unknown panic payload".to_string()
        };

        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "Unknown location".to_string());

        let os = std::env::consts::OS;
        let arch = std::env::consts::ARCH;
        let version = env!("CARGO_PKG_VERSION");
        let backtrace = std::backtrace::Backtrace::force_capture();

        let crash_content = format!(
            "YAGO Crash Report\n\
             =================\n\
             Version:  {}\n\
             Platform: {} ({})\n\
             Time:     {}\n\
             Location: {}\n\
             Payload:  {}\n\n\
             Backtrace:\n\
             {}\n",
            version, os, arch, timestamp, location, payload, backtrace
        );

        let _ = fs::write(&crash_log_path, &crash_content);
        tracing::error!(
            "FATAL PANIC at {}: {}\nCrash report saved to {:?}",
            location,
            payload,
            crash_log_path
        );

        prev_hook(info);
    }));
}

/// Searches for the most recent unacknowledged crash log in app_data_dir/logs/
pub fn find_recent_crash_log(app_data_dir: &Path) -> Option<PathBuf> {
    let logs_dir = app_data_dir.join("logs");
    if !logs_dir.exists() {
        return None;
    }

    let mut crash_logs = Vec::new();
    if let Ok(entries) = fs::read_dir(logs_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                if name.starts_with("crash-") && name.ends_with(".log") && !name.contains(".seen.")
                {
                    crash_logs.push(path);
                }
            }
        }
    }

    crash_logs.sort();
    crash_logs.pop()
}

/// Acknowledges a crash log by renaming it to *.seen.log so it is only presented once
pub fn acknowledge_crash_log(path: &Path) -> Result<(), std::io::Error> {
    if !path.exists() {
        return Ok(());
    }

    let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
    if filename.starts_with("crash-") && filename.ends_with(".log") && !filename.contains(".seen.")
    {
        let new_name = filename.replace(".log", ".seen.log");
        let new_path = path.with_file_name(new_name);
        fs::rename(path, new_path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_find_recent_crash_log_and_acknowledge() {
        let dir = tempdir().unwrap();
        let logs_dir = dir.path().join("logs");
        fs::create_dir_all(&logs_dir).unwrap();

        // Initially no crash logs
        assert!(find_recent_crash_log(dir.path()).is_none());

        // Create older crash log
        let crash1 = logs_dir.join("crash-20261123_100000.log");
        fs::write(&crash1, "crash 1").unwrap();

        // Create newer crash log
        let crash2 = logs_dir.join("crash-20261123_110000.log");
        fs::write(&crash2, "crash 2").unwrap();

        // Should find the latest unacknowledged crash log (crash2)
        let found = find_recent_crash_log(dir.path()).unwrap();
        assert_eq!(found, crash2);

        // Acknowledge crash2
        acknowledge_crash_log(&found).unwrap();
        assert!(!crash2.exists());
        assert!(logs_dir.join("crash-20261123_110000.seen.log").exists());

        // Now find should return crash1
        let found_older = find_recent_crash_log(dir.path()).unwrap();
        assert_eq!(found_older, crash1);

        // Acknowledge crash1
        acknowledge_crash_log(&found_older).unwrap();
        assert!(find_recent_crash_log(dir.path()).is_none());
    }
}
