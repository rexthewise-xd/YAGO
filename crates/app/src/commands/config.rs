use storage::settings::GlobalSettings;

use crate::config::AppConfig;
use crate::events::AppEvent;
use crate::init::{extract_templates, librarian_config};
use crate::state::AppCore;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct AppUpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub has_update: bool,
    pub release_url: Option<String>,
    pub notes: Option<String>,
}

fn is_version_newer(latest: &str, current: &str) -> bool {
    let parse_parts = |v: &str| -> Vec<u64> {
        v.split('.')
            .filter_map(|s| {
                s.chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect::<String>()
                    .parse::<u64>()
                    .ok()
            })
            .collect()
    };

    let latest_parts = parse_parts(latest);
    let current_parts = parse_parts(current);

    for (l, c) in latest_parts.iter().zip(current_parts.iter()) {
        if l > c {
            return true;
        } else if l < c {
            return false;
        }
    }

    latest_parts.len() > current_parts.len()
}

impl AppCore {
    pub async fn get_app_config(&self) -> Result<AppConfig, String> {
        Ok(self.app_config.lock().await.clone())
    }

    pub async fn update_app_config(&self, config: AppConfig) -> Result<(), String> {
        let config_path = self.app_data_dir.join("app_config.json");
        storage::write_json_atomic(&config_path, &config).map_err(|e| e.to_string())?;

        *self.app_config.lock().await = config.clone();
        self.emit(AppEvent::AppConfigUpdated(config));
        Ok(())
    }

    pub async fn get_settings(&self) -> Result<GlobalSettings, String> {
        Ok(self.global_settings.lock().await.clone())
    }

    pub async fn update_settings(&self, settings: GlobalSettings) -> Result<(), String> {
        let mut current_settings = self.global_settings.lock().await;

        self.settings_manager
            .save(&settings)
            .await
            .map_err(|e| e.to_string())?;

        let base = if settings.yago_storage_path.as_os_str().is_empty() {
            self.app_data_dir.clone()
        } else {
            settings.yago_storage_path.clone()
        };
        let path_changed = base != current_settings.yago_storage_path;

        let mut librarian = self.librarian.lock().await;
        librarian.update_roots(librarian_config(&settings, &self.app_data_dir));
        librarian.ensure_core_dirs().map_err(|e| e.to_string())?;

        if path_changed {
            extract_templates(&librarian);
        }

        *current_settings = settings.clone();
        self.emit(AppEvent::SettingsUpdated(settings));
        Ok(())
    }

    pub async fn check_app_update(&self) -> Result<AppUpdateInfo, String> {
        let current_version = env!("CARGO_PKG_VERSION").to_string();
        let release = resources::github::get_latest_release("rexthewise-xd", "YAGO")
            .await
            .map_err(|e| e.to_string())?;

        let clean_current = current_version.trim_start_matches(['v', 'V']);
        let clean_latest = release.tag_name.trim_start_matches(['v', 'V']);

        let has_update = is_version_newer(clean_latest, clean_current);

        let release_url = release.html_url.or_else(|| {
            Some(format!(
                "https://github.com/rexthewise-xd/YAGO/releases/tag/{}",
                release.tag_name
            ))
        });

        Ok(AppUpdateInfo {
            current_version,
            latest_version: release.tag_name,
            has_update,
            release_url,
            notes: release.body,
        })
    }

    pub fn find_recent_crash(&self) -> Option<std::path::PathBuf> {
        crate::logging::find_recent_crash_log(&self.app_data_dir)
    }

    pub fn acknowledge_crash(&self, path: &std::path::Path) -> Result<(), String> {
        crate::logging::acknowledge_crash_log(path).map_err(|e| e.to_string())
    }
}
