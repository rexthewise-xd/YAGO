use crate::{download_file, github};
use anyhow::{anyhow, Result};
use ini::ast::{IniDocument, IniItem, Section};
use ini::IniCompiler;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EffectRegistrySpec {
    pub id: String,
    pub name: String,
    pub description: String,
    pub repo: String,
    #[serde(default)]
    pub applicable_games: Vec<String>,
}

impl EffectRegistrySpec {
    pub fn defaults() -> Vec<Self> {
        vec![
            Self {
                id: "orfix".to_string(),
                name: "ORFix".to_string(),
                description: "Object Rendering Fix for Genshin Impact (LeoTools)".to_string(),
                repo: "leotorrez/LeoTools".to_string(),
                applicable_games: vec!["genshin".to_string()],
            },
            Self {
                id: "texfx".to_string(),
                name: "TexFx".to_string(),
                description: "Advanced texture effects, uncensor, and photosensitivity protection"
                    .to_string(),
                repo: "SinsOfSeven/TexFx".to_string(),
                applicable_games: vec![
                    "genshin".to_string(),
                    "hsr".to_string(),
                    "zzz".to_string(),
                    "wuwa".to_string(),
                ],
            },
            Self {
                id: "rabbitfx".to_string(),
                name: "RabbitFX".to_string(),
                description:
                    "Universal visual shader enhancement library for ZZZ, WuWa, and Endfield"
                        .to_string(),
                repo: "SilentNightSound/RabbitFX".to_string(),
                applicable_games: vec![
                    "zzz".to_string(),
                    "wuwa".to_string(),
                    "endfield".to_string(),
                ],
            },
        ]
    }

    /// Loads effect specifications from registry folders, falling back to built-in defaults.
    pub fn load_from_registries(registry_root: &Path) -> Vec<Self> {
        let mut specs = HashMap::new();

        // 1. Default fallback specs
        for spec in Self::defaults() {
            specs.insert(spec.id.clone(), spec);
        }

        // 2. Overlay any synced effects/effects.json
        if registry_root.exists() {
            if let Ok(entries) = std::fs::read_dir(registry_root) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let path = entry.path().join("effects").join("effects.json");
                    if path.exists() {
                        if let Ok(content) = std::fs::read_to_string(&path) {
                            if let Ok(list) =
                                serde_json::from_str::<Vec<EffectRegistrySpec>>(&content)
                            {
                                for spec in list {
                                    specs.insert(spec.id.clone(), spec);
                                }
                            }
                        }
                    }
                }
            }
        }

        let mut result: Vec<_> = specs.into_values().collect();
        result.sort_by(|a, b| a.id.cmp(&b.id));
        result
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SharedEffectLibrary {
    ORFix,
    TexFx,
    RabbitFX,
}

impl SharedEffectLibrary {
    pub fn all() -> &'static [SharedEffectLibrary] {
        &[
            SharedEffectLibrary::ORFix,
            SharedEffectLibrary::TexFx,
            SharedEffectLibrary::RabbitFX,
        ]
    }

    pub fn id(&self) -> &'static str {
        match self {
            Self::ORFix => "orfix",
            Self::TexFx => "texfx",
            Self::RabbitFX => "rabbitfx",
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::ORFix => "ORFix",
            Self::TexFx => "TexFx",
            Self::RabbitFX => "RabbitFX",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::ORFix => "Object Rendering Fix for Genshin Impact (LeoTools)",
            Self::TexFx => "Advanced texture effects, uncensor, and photosensitivity protection",
            Self::RabbitFX => {
                "Universal visual shader enhancement library for ZZZ, WuWa, and Endfield"
            }
        }
    }

    pub fn default_repo(&self) -> &'static str {
        match self {
            Self::ORFix => "leotorrez/LeoTools",
            Self::TexFx => "SinsOfSeven/TexFx",
            Self::RabbitFX => "SilentNightSound/RabbitFX",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id.to_lowercase().as_str() {
            "orfix" => Some(Self::ORFix),
            "texfx" => Some(Self::TexFx),
            "rabbitfx" => Some(Self::RabbitFX),
            _ => None,
        }
    }

    pub fn applies_to_game(&self, game_id: &str) -> bool {
        let gid = game_id.to_lowercase();
        match self {
            Self::ORFix | Self::TexFx => gid.contains("genshin"),
            Self::RabbitFX => {
                gid.contains("zzz")
                    || gid.contains("zenless")
                    || gid.contains("wuwa")
                    || gid.contains("wuthering")
                    || gid.contains("endfield")
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TexFxConfig {
    pub uncensor: u8, // 0 = off, 1 = on, 2 = camera-proximity
    pub seizure: u8,  // 0 = off, 1 = on (photosensitivity protection)
}

impl Default for TexFxConfig {
    fn default() -> Self {
        Self {
            uncensor: 0,
            seizure: 0,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct EffectLibraryStatus {
    pub id: String,
    pub name: String,
    pub description: String,
    pub installed_version: Option<String>,
    pub enabled: bool,
    pub duplicate_warnings: Vec<String>,
    pub texfx_config: Option<TexFxConfig>,
}

/// Installs or updates a shared effect library.
/// Skips download if .yago_version matches the latest release tag.
pub async fn install_or_update_effect<F>(
    lib: SharedEffectLibrary,
    dest_dir: &Path,
    mut on_progress: F,
) -> Result<String>
where
    F: FnMut(u64, u64),
{
    let repo_slug = lib.default_repo();
    let parts: Vec<&str> = repo_slug.split('/').collect();
    if parts.len() != 2 {
        return Err(anyhow!("Invalid repository slug: {}", repo_slug));
    }
    let (owner, repo) = (parts[0], parts[1]);

    let release = github::get_latest_release(owner, repo).await?;
    let tag = release.tag_name.clone();

    // 1. Check version
    let version_file = dest_dir.join(".yago_version");
    if version_file.exists() {
        if let Ok(current_tag) = std::fs::read_to_string(&version_file) {
            if current_tag.trim() == tag.trim() {
                println!(
                    "Effect library {} is already at latest version ({}). Skipping download.",
                    lib.name(),
                    tag
                );
                on_progress(100, 100);
                return Ok(tag);
            }
        }
    }

    // 2. Locate release zip asset or fall back to zipball
    let download_url = if let Some(asset) = release
        .assets
        .iter()
        .find(|a| a.name.to_lowercase().ends_with(".zip"))
    {
        asset.browser_download_url.clone()
    } else {
        format!(
            "https://api.github.com/repos/{}/{}/zipball/{}",
            owner, repo, tag
        )
    };

    println!(
        "Downloading Effect Library {} ({}): {}...",
        lib.name(),
        tag,
        download_url
    );

    let temp_dir = tempfile::tempdir()?;
    let archive_path = temp_dir.path().join("effect.zip");

    download_file(&download_url, &archive_path, on_progress).await?;

    println!("Extracting {} into {:?}...", lib.name(), dest_dir);
    vfs::archive::extract_and_sanitize(&archive_path, dest_dir)
        .map_err(|e| anyhow!("Extraction failed: {}", e))?;

    // 3. Save version tag
    if let Some(parent) = version_file.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(&version_file, &tag)?;

    Ok(tag)
}

/// Scans a game's user mods directory (including BufferValues/ subfolders)
/// for duplicate copies of ORFix.ini, TexFx.ini, or RabbitFX.ini that would conflict.
pub fn detect_duplicate_effects(mods_root: &Path) -> Vec<PathBuf> {
    let mut duplicates = Vec::new();
    if !mods_root.exists() {
        return duplicates;
    }

    let target_names = ["orfix.ini", "texfx.ini", "rabbitfx.ini"];

    for entry in WalkDir::new(mods_root).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            if let Some(name) = entry.file_name().to_str() {
                let name_lower = name.to_lowercase();
                if target_names.contains(&name_lower.as_str()) {
                    duplicates.push(entry.path().to_path_buf());
                }
            }
        }
    }

    duplicates
}

fn find_texfx_config_file(effect_dir: &Path) -> Option<PathBuf> {
    if !effect_dir.exists() {
        return None;
    }
    // Check direct Config.ini or config.ini
    let candidate = effect_dir.join("Config.ini");
    if candidate.exists() {
        return Some(candidate);
    }
    let candidate_lower = effect_dir.join("config.ini");
    if candidate_lower.exists() {
        return Some(candidate_lower);
    }
    // Search recursively
    for entry in WalkDir::new(effect_dir)
        .max_depth(3)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            if let Some(name) = entry.file_name().to_str() {
                if name.eq_ignore_ascii_case("config.ini") {
                    return Some(entry.path().to_path_buf());
                }
            }
        }
    }
    None
}

/// Reads $uncensor and $seizure variables from TexFx's Config.ini.
pub fn read_texfx_config(effect_dir: &Path) -> TexFxConfig {
    let mut cfg = TexFxConfig::default();
    let Some(config_path) = find_texfx_config_file(effect_dir) else {
        return cfg;
    };

    let Ok(content) = std::fs::read_to_string(&config_path) else {
        return cfg;
    };

    let Ok((_, doc)) = ini::parser::parse_ini(&content) else {
        return cfg;
    };

    for section in &doc.sections {
        for item in &section.items {
            if let IniItem::Pair { key, value } = item {
                let key_clean = key.trim().to_lowercase();
                if key_clean.contains("uncensor") {
                    if let Ok(val) = value.trim().parse::<u8>() {
                        cfg.uncensor = val;
                    }
                } else if key_clean.contains("seizure") {
                    if let Ok(val) = value.trim().parse::<u8>() {
                        cfg.seizure = val;
                    }
                }
            }
        }
    }

    cfg
}

/// Updates $uncensor and $seizure variables in TexFx's Config.ini.
pub fn write_texfx_config(effect_dir: &Path, cfg: &TexFxConfig) -> Result<()> {
    if !effect_dir.exists() {
        std::fs::create_dir_all(effect_dir)?;
    }

    let config_path =
        find_texfx_config_file(effect_dir).unwrap_or_else(|| effect_dir.join("Config.ini"));

    let mut doc = if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)?;
        let (_, parsed) = ini::parser::parse_ini(&content)
            .map_err(|e| anyhow!("Failed to parse {}: {:?}", config_path.display(), e))?;
        parsed
    } else {
        IniDocument {
            sections: vec![Section {
                name: "Constants".to_string(),
                items: Vec::new(),
            }],
        }
    };

    let mut uncensor_updated = false;
    let mut seizure_updated = false;

    for section in &mut doc.sections {
        for item in &mut section.items {
            if let IniItem::Pair { key, value } = item {
                let key_clean = key.trim().to_lowercase();
                if key_clean.contains("uncensor") {
                    *value = cfg.uncensor.to_string();
                    uncensor_updated = true;
                } else if key_clean.contains("seizure") {
                    *value = cfg.seizure.to_string();
                    seizure_updated = true;
                }
            }
        }
    }

    if !uncensor_updated || !seizure_updated {
        let constants = if let Some(sec) = doc
            .sections
            .iter_mut()
            .find(|s| s.name.eq_ignore_ascii_case("Constants"))
        {
            sec
        } else {
            doc.sections.insert(
                0,
                Section {
                    name: "Constants".to_string(),
                    items: Vec::new(),
                },
            );
            &mut doc.sections[0]
        };

        if !uncensor_updated {
            constants.items.push(IniItem::Pair {
                key: "global persist $uncensor".to_string(),
                value: cfg.uncensor.to_string(),
            });
        }
        if !seizure_updated {
            constants.items.push(IniItem::Pair {
                key: "global persist $seizure".to_string(),
                value: cfg.seizure.to_string(),
            });
        }
    }

    let compiled = IniCompiler::default().serialize(&doc);
    std::fs::write(&config_path, compiled)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_duplicate_effects() {
        let dir = tempfile::tempdir().unwrap();
        let mods = dir.path().join("Mods");
        let nested = mods.join("CharacterA").join("BufferValues");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("ORFix.ini"), "; orfix duplicate").unwrap();
        std::fs::write(nested.join("TexFx.ini"), "; texfx duplicate").unwrap();
        std::fs::write(nested.join("normal.ini"), "; normal mod").unwrap();

        let dupes = detect_duplicate_effects(&mods);
        assert_eq!(dupes.len(), 2);
        let names: Vec<String> = dupes
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert!(names.contains(&"ORFix.ini".to_string()));
        assert!(names.contains(&"TexFx.ini".to_string()));
    }

    #[test]
    fn test_texfx_config_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let initial = TexFxConfig {
            uncensor: 2,
            seizure: 1,
        };
        write_texfx_config(dir.path(), &initial).unwrap();
        let loaded = read_texfx_config(dir.path());
        assert_eq!(loaded, initial);

        let updated = TexFxConfig {
            uncensor: 0,
            seizure: 0,
        };
        write_texfx_config(dir.path(), &updated).unwrap();
        let reloaded = read_texfx_config(dir.path());
        assert_eq!(reloaded, updated);
    }
}
