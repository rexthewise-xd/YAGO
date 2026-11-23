use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use storage::{GlobalSettings, Librarian, SettingsManager, TemplateRegistry};
use tokio::sync::Mutex;

use crate::config::AppConfig;
use crate::events::{AppEvent, EventBus};
use crate::state::AppCore;
use crate::ASSETS_DIR;

#[cfg(target_os = "linux")]
static SHIELD_BYTES: &[u8] = include_bytes!(env!("YAGO_SHIELD_PATH"));

pub fn default_app_data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("yago")
}

pub fn librarian_config(
    settings: &GlobalSettings,
    app_data_dir: &Path,
) -> storage::storage::LibrarianConfig {
    let opt = |p: &Path| {
        if p.as_os_str().is_empty() {
            None
        } else {
            Some(p.to_path_buf())
        }
    };
    storage::storage::LibrarianConfig {
        base_path: if settings.yago_storage_path.as_os_str().is_empty() {
            app_data_dir.to_path_buf()
        } else {
            settings.yago_storage_path.clone()
        },
        games_install_path: opt(&settings.default_games_path),
        mods_path: opt(&settings.mods_path),
        runners_path: opt(&settings.runners_path),
        prefixes_path: opt(&settings.prefixes_path),
        cache_path: opt(&settings.cache_path),
    }
}

pub fn extract_templates(librarian: &Librarian) {
    if let Some(dir) = ASSETS_DIR.get_dir("templates") {
        for file in dir.files() {
            let dest = librarian
                .templates_root
                .join(file.path().file_name().unwrap());
            if let Err(e) = std::fs::write(&dest, file.contents()) {
                eprintln!("Failed to extract {:?}: {}", dest, e);
            }
        }
    }
}

pub async fn init() -> anyhow::Result<AppCore> {
    init_with_dir(default_app_data_dir()).await
}

pub async fn init_with_dir(app_data_dir: PathBuf) -> anyhow::Result<AppCore> {
    std::fs::create_dir_all(&app_data_dir)?;

    let settings_manager = Arc::new(SettingsManager::new(app_data_dir.clone()));

    let settings_path = app_data_dir.join("settings.json");
    let has_settings = settings_path.exists();
    let (settings, settings_recovered) = if has_settings {
        match storage::read_json_with_recovery::<GlobalSettings>(&settings_path) {
            Ok(rec) => (rec.data, rec.recovered_from_backup),
            Err(_) => (GlobalSettings::default(), false),
        }
    } else {
        (GlobalSettings::default(), false)
    };

    let librarian = Librarian::new(librarian_config(&settings, &app_data_dir));
    if has_settings {
        librarian.ensure_core_dirs()?;
    } else {
        std::fs::create_dir_all(&librarian.assets_root)?;
        std::fs::create_dir_all(&librarian.templates_root)?;
    }

    let config_path = app_data_dir.join("app_config.json");
    if !config_path.exists() {
        if let Some(file) = ASSETS_DIR.get_file("app_config.json") {
            let _ = std::fs::write(&config_path, file.contents());
        }
    }

    let hashes_path = librarian.assets_root.join("hashes.json");
    if !hashes_path.exists() {
        if let Some(file) = ASSETS_DIR.get_file("hashes.json") {
            let _ = std::fs::write(&hashes_path, file.contents());
        }
    }
    let hashes_root = librarian.assets_root.join("hashes");
    if !hashes_root.exists() {
        let _ = std::fs::create_dir_all(&hashes_root);
    }
    if let Some(dir) = ASSETS_DIR.get_dir("hashes") {
        for file in dir.files() {
            let dest = hashes_root.join(file.path().file_name().unwrap());
            let _ = std::fs::write(&dest, file.contents());
        }
    }

    let fixers_root = librarian.assets_root.join("fixers");
    if !fixers_root.exists() {
        let _ = std::fs::create_dir_all(&fixers_root);
    }
    if let Some(dir) = ASSETS_DIR.get_dir("fixers") {
        for file in dir.files() {
            let dest = fixers_root.join(file.path().file_name().unwrap());
            let _ = std::fs::write(&dest, file.contents());
        }
    }

    extract_templates(&librarian);

    let libs_dir = app_data_dir.join("libs");
    if !libs_dir.exists() {
        let _ = std::fs::create_dir_all(&libs_dir);
    }

    #[cfg(target_os = "linux")]
    {
        let dest = libs_dir.join("libshield.so");
        let needs_write = std::fs::read(&dest)
            .map(|b| b != SHIELD_BYTES)
            .unwrap_or(true);
        if needs_write {
            if let Err(e) = std::fs::write(&dest, SHIELD_BYTES) {
                eprintln!("Failed to extract libshield.so: {}", e);
            } else {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755));
            }
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let src = dir.join("libs").join("win_helper.exe");
            if src.exists() {
                let dest = libs_dir.join("win_helper.exe");
                if let Err(e) = std::fs::copy(&src, &dest) {
                    eprintln!("Failed to extract win_helper.exe: {}", e);
                }
            }
        }
    }

    let app_config = Arc::new(Mutex::new(AppConfig::load(config_path)));
    let events = EventBus::new();

    let core = AppCore {
        app_data_dir: app_data_dir.clone(),
        librarian: Arc::new(Mutex::new(librarian.clone())),
        game_templates: Arc::new(Mutex::new(HashMap::new())),
        game_dbs: Arc::new(Mutex::new(HashMap::new())),
        running_game_name: Arc::new(Mutex::new(None)),
        is_launching: Arc::new(Mutex::new(false)),
        settings_manager,
        global_settings: Arc::new(Mutex::new(settings)),
        app_config,
        download_controls: Arc::new(Mutex::new(HashMap::new())),
        events,
    };

    let bg = core.clone();
    tokio::spawn(async move {
        if settings_recovered {
            bg.emit(AppEvent::Warning(
                "Restored settings.json from backup (.bak)".to_string(),
            ));
        }

        let registry = TemplateRegistry::new(bg.librarian.lock().await.templates_root.clone());
        if let Ok(templates) = registry.load_all().await {
            *bg.game_templates.lock().await = templates;
        }

        let ids = bg.librarian.lock().await.discover_game_ids().await;
        if let Ok(ids) = ids {
            let mut dbs = bg.game_dbs.lock().await;
            for id in ids {
                if let Ok(db) = bg.librarian.lock().await.load_game_db(&id).await {
                    dbs.insert(id, db);
                }
            }
        }

        let report = crate::reconcile::reconcile_library(&bg).await;
        if report.has_issues() {
            let mut msgs = Vec::new();
            if !report.reset_downloads.is_empty() {
                msgs.push(format!(
                    "Reset {} stuck download(s) to queued",
                    report.reset_downloads.len()
                ));
            }
            if !report.cleaned_session_locks.is_empty() {
                msgs.push(format!(
                    "Cleaned {} dirty session lock(s)",
                    report.cleaned_session_locks.len()
                ));
            }
            if !report.missing_games.is_empty() {
                msgs.push(format!(
                    "{} game executable(s) missing on disk",
                    report.missing_games.len()
                ));
            }
            if !report.orphaned_mods.is_empty() {
                let count: usize = report.orphaned_mods.values().map(|v| v.len()).sum();
                msgs.push(format!("{} orphaned mod(s) detected", count));
            }
            bg.emit(AppEvent::Warning(msgs.join(" • ")));
        }

        bg.emit_library().await;
    });

    Ok(core)
}
