use storage::settings::GlobalSettings;

use crate::config::AppConfig;
use crate::events::AppEvent;
use crate::init::{extract_templates, librarian_config};
use crate::state::AppCore;

impl AppCore {
    pub async fn get_app_config(&self) -> Result<AppConfig, String> {
        Ok(self.app_config.lock().await.clone())
    }

    pub async fn update_app_config(&self, config: AppConfig) -> Result<(), String> {
        let config_path = self.app_data_dir.join("app_config.json");
        let content = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
        std::fs::write(config_path, content).map_err(|e| e.to_string())?;

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
}