use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalSettings {
    pub steam_compat_tools_path: PathBuf,
    pub wine_prefix_path: PathBuf,
    pub yago_storage_path: PathBuf,
    pub default_games_path: PathBuf,
    pub mods_path: PathBuf,
    pub runners_path: PathBuf,
    pub prefixes_path: PathBuf,
    pub cache_path: PathBuf,
    pub default_runner_id: Option<String>,
    pub language: String,
    pub stream_safe: bool,
    pub nsfw_behavior: String, // "blur" | "hide"
    pub close_on_launch: bool,
    #[serde(default)]
    pub suppress_creditinfo_global: bool,
}

impl Default for GlobalSettings {
    fn default() -> Self {
        Self {
            steam_compat_tools_path: PathBuf::from(""),
            wine_prefix_path: PathBuf::from(""),
            yago_storage_path: PathBuf::from(""),
            default_games_path: PathBuf::from(""),
            mods_path: PathBuf::from(""),
            runners_path: PathBuf::from(""),
            prefixes_path: PathBuf::from(""),
            cache_path: PathBuf::from(""),
            default_runner_id: None,
            language: "en-US".to_string(),
            stream_safe: true,
            nsfw_behavior: "blur".to_string(),
            close_on_launch: false,
            suppress_creditinfo_global: false,
        }
    }
}

pub struct SettingsManager {
    pub config_path: PathBuf,
}

impl SettingsManager {
    pub fn new(app_data_dir: PathBuf) -> Self {
        Self {
            config_path: app_data_dir.join("settings.json"),
        }
    }

    pub async fn load(&self) -> Result<GlobalSettings> {
        if !self.config_path.exists() {
            let default = GlobalSettings::default();
            self.save(&default).await?;
            return Ok(default);
        }

        let recovery = crate::atomic::read_json_with_recovery::<GlobalSettings>(&self.config_path)?;
        Ok(recovery.data)
    }

    pub async fn save(&self, settings: &GlobalSettings) -> Result<()> {
        crate::atomic::write_json_atomic(&self.config_path, settings)
    }
}
