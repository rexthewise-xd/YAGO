use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    pub common_loader_repo: String,
    pub reshade_url: String,
    pub proton_repo: String,
    pub default_cover_image: String,
    pub default_icon_image: String,
    pub community_backgrounds_repo: String,
    pub community_backgrounds_base_url: String,
    pub preset_covers: Vec<String>,
    pub yago_update_url: String,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_registries")]
    pub registries: Vec<resources::RegistrySource>,
}

fn default_theme() -> String {
    "linear".to_string()
}

pub fn default_registries() -> Vec<resources::RegistrySource> {
    vec![resources::RegistrySource {
        id: "official".to_string(),
        name: "YAGO Official Registry".to_string(),
        url: "https://raw.githubusercontent.com/rexthewise-xd/yago-registry/main".to_string(),
        enabled: true,
        official: true,
    }]
}

impl AppConfig {
    pub fn load(path: std::path::PathBuf) -> Self {
        if let Ok(recovery) = storage::read_json_with_recovery::<AppConfig>(&path) {
            return recovery.data;
        }
        Self::default()
    }
}

#[allow(clippy::derivable_impls)]
impl Default for AppConfig {
    fn default() -> Self {
        Self {
            common_loader_repo: "SpectrumQT/XXMI-Libs-Package".to_string(),
            reshade_url: "https://reshade.me/downloads/ReShade_Setup_6.6.2.exe".to_string(),
            proton_repo: "GloriousEggroll/proton-ge-custom".to_string(),
            default_cover_image: "https://picsum.photos/seed/yago/1920/1080".to_string(),
            default_icon_image: "https://picsum.photos/seed/yago-icon/200/200".to_string(),
            community_backgrounds_repo: "UIGF-org/HoYoPlay-Launcher-Background".to_string(),
            community_backgrounds_base_url: "https://raw.githubusercontent.com/UIGF-org/HoYoPlay-Launcher-Background/main/output".to_string(),
            preset_covers: vec![
                "https://picsum.photos/seed/genshin-landscape/1920/1080".to_string(),
                "https://picsum.photos/seed/honkai-landscape/1920/1080".to_string(),
                "https://picsum.photos/seed/starrail-landscape/1920/1080".to_string(),
                "https://picsum.photos/seed/zenless-landscape/1920/1080".to_string(),
                "https://picsum.photos/seed/wuwa-landscape/1920/1080".to_string(),
                "https://picsum.photos/seed/fantasy-landscape/1920/1080".to_string(),
            ],
            yago_update_url: "https://api.github.com/repos/rexthewise-xd/YAGO/releases/latest".to_string(),
            theme: "linear".to_string(),
            registries: default_registries(),
        }
    }
}
