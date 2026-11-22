use crate::error::Result;
use crate::models::{FpsConfig, InjectionMethod, StartMethod};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct GameTemplate {
    #[serde(default)]
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub short_name: String,
    #[serde(default)]
    pub developer: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub color: String,
    #[serde(default)]
    pub accent_color: String,
    #[serde(default)]
    pub cover_image: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub logo_initial: String,
    #[serde(default)]
    pub executables: Vec<String>,
    #[serde(default)]
    pub logic_type: String,

    // External Resources
    #[serde(default)]
    pub loader_repo: Option<String>, // e.g., "SpectrumQT/WWMI-Package"

    #[serde(default, alias = "hash_repo")]
    pub hash_db_url: Option<String>, // Raw GitHub URL or repo for character hash map

    // Logic/Patching Config
    #[serde(default)]
    pub patch_logic: Option<HashMap<String, String>>, // e.g., {"Constants/global_persist_$orfix": "1"}

    // Configuration Presets
    #[serde(default)]
    pub injection_method_linux: Option<InjectionMethod>,
    #[serde(default)]
    pub injection_method_windows: Option<InjectionMethod>,
    #[serde(default)]
    pub modloader_enabled: Option<bool>,
    #[serde(default)]
    pub supported_injection_methods: Option<Vec<InjectionMethod>>,
    #[serde(default)]
    pub launch_args: Option<Vec<String>>,
    #[serde(default)]
    pub fps_config: Option<FpsConfig>,
    #[serde(default)]
    pub auto_update: Option<bool>,
    #[serde(default)]
    pub community_folder: Option<String>,
    #[serde(default)]
    pub start_method: Option<StartMethod>,
    #[serde(default)]
    pub unsafe_mode: Option<bool>,
    #[serde(default)]
    pub umu_game_id: Option<String>,
    #[serde(default)]
    pub use_umu: Option<bool>,
    #[serde(default)]
    pub pre_launch_cmd: Option<String>,
    #[serde(default)]
    pub post_exit_cmd: Option<String>,
    #[serde(default)]
    pub use_rabbit_wrapper: Option<bool>,

    // Sophon Discovery Fields
    #[serde(default)]
    pub sophon_branch: String, // e.g., "main"
    #[serde(default)]
    pub sophon_package_id: String, // e.g., "genshin_impact_sophon"
    #[serde(default)]
    pub sophon_password: String,
    #[serde(default)]
    pub sophon_plat_app: String, // e.g., "hk4e_global" (The ID)
    #[serde(default)]
    pub sophon_game_biz: String, // e.g., "hk4e_global" (The BIZ name)
    #[serde(default)]
    pub sophon_launcher_id: String,
    #[serde(default)]
    pub sophon_channel_id: String,
    #[serde(default)]
    pub sophon_sub_channel_id: String,
}

pub fn load_templates(dir: &Path) -> Result<Vec<GameTemplate>> {
    let mut templates = Vec::new();
    if !dir.exists() {
        return Ok(templates);
    }

    for entry in walkdir::WalkDir::new(dir).min_depth(1).max_depth(1) {
        let entry = entry.map_err(|e| std::io::Error::other(e.to_string()))?; // Basic mapping
        if entry.path().extension().and_then(|s| s.to_str()) == Some("json") {
            let content = std::fs::read_to_string(entry.path())?;
            let mut template: GameTemplate = serde_json::from_str(&content)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

            // Auto-fill ID from filename if missing
            if template.id.is_empty() {
                if let Some(stem) = entry.path().file_stem().and_then(|s| s.to_str()) {
                    template.id = stem.to_string();
                }
            }

            templates.push(template);
        }
    }

    Ok(templates)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_game_template_hash_repo_alias() {
        let json = r#"{
            "name": "Genshin Impact",
            "hash_repo": "leotorrez/GI-Model-Importer-Assets"
        }"#;
        let template: GameTemplate = serde_json::from_str(json).unwrap();
        assert_eq!(
            template.hash_db_url.as_deref(),
            Some("leotorrez/GI-Model-Importer-Assets")
        );
    }
}
