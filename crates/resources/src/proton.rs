use crate::download_file;
use crate::github;
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::path::Path;
use vfs::archive::extract_targz;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RunnerSourceSpec {
    pub id: String,
    pub name: String,
    pub owner: String,
    pub repo: String,
    #[serde(default)]
    pub archive_exts: Vec<String>,
}

impl RunnerSourceSpec {
    pub fn defaults() -> Vec<Self> {
        vec![
            Self {
                id: "ge-proton".to_string(),
                name: "GE-Proton".to_string(),
                owner: "GloriousEggroll".to_string(),
                repo: "proton-ge-custom".to_string(),
                archive_exts: vec![
                    ".tar.gz".to_string(),
                    ".tar.xz".to_string(),
                    ".tar.zst".to_string(),
                ],
            },
            Self {
                id: "proton-cachyos".to_string(),
                name: "Proton-CachyOS".to_string(),
                owner: "CachyOS".to_string(),
                repo: "proton-cachyos".to_string(),
                archive_exts: vec![
                    ".tar.gz".to_string(),
                    ".tar.xz".to_string(),
                    ".tar.zst".to_string(),
                ],
            },
            Self {
                id: "dw-proton".to_string(),
                name: "DW-Proton".to_string(),
                owner: "Dawn-Wings".to_string(),
                repo: "DW-Proton".to_string(),
                archive_exts: vec![
                    ".tar.gz".to_string(),
                    ".tar.xz".to_string(),
                    ".tar.zst".to_string(),
                ],
            },
        ]
    }

    /// Loads runner source specifications from registry folders, falling back to built-in defaults.
    pub fn load_from_registries(registry_root: &Path) -> Vec<Self> {
        let mut specs = HashMap::new();

        for spec in Self::defaults() {
            specs.insert(spec.id.clone(), spec);
        }

        if registry_root.exists() {
            if let Ok(entries) = std::fs::read_dir(registry_root) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let path = entry.path().join("runners").join("proton-sources.json");
                    if path.exists() {
                        if let Ok(content) = std::fs::read_to_string(&path) {
                            if let Ok(list) =
                                serde_json::from_str::<Vec<RunnerSourceSpec>>(&content)
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
pub enum ProtonSource {
    GeProton,
    ProtonCachyOs,
    DwProton,
}

impl ProtonSource {
    pub fn all() -> &'static [ProtonSource] {
        &[
            ProtonSource::GeProton,
            ProtonSource::ProtonCachyOs,
            ProtonSource::DwProton,
        ]
    }

    pub fn name(&self) -> &'static str {
        match self {
            ProtonSource::GeProton => "GE-Proton",
            ProtonSource::ProtonCachyOs => "Proton-CachyOS",
            ProtonSource::DwProton => "DW-Proton",
        }
    }

    pub fn repo(&self) -> (&'static str, &'static str) {
        match self {
            ProtonSource::GeProton => ("GloriousEggroll", "proton-ge-custom"),
            ProtonSource::ProtonCachyOs => ("CachyOS", "proton-cachyos"),
            ProtonSource::DwProton => ("Dawn-Wings", "DW-Proton"),
        }
    }
}

pub async fn update_ge_proton<F>(
    owner: &str,
    repo_name: &str,
    dest: &Path,
    mut on_progress: F,
) -> Result<()>
where
    F: FnMut(u64, u64),
{
    let release = github::get_latest_release(owner, repo_name).await?;

    // 1. Check if already installed
    // GE-Proton releases usually have a folder inside named after the tag
    let tag = &release.tag_name;
    let expected_dir = dest.join(tag);
    if expected_dir.exists() && expected_dir.is_dir() {
        println!("Proton {} is already installed. Skipping download.", tag);
        // Signal 100% progress
        on_progress(100, 100);
        return Ok(());
    }

    let asset = release
        .assets
        .iter()
        .find(|a| a.name.ends_with(".tar.gz"))
        .ok_or_else(|| anyhow!("No .tar.gz asset found in latest {} release", repo_name))?;

    println!("Downloading Proton: {}...", asset.name);

    let temp_dir = tempfile::tempdir()?;
    let archive_path = temp_dir.path().join(&asset.name);

    download_file(&asset.browser_download_url, &archive_path, on_progress).await?;

    println!("Extracting Proton...");
    extract_targz(&archive_path, dest).map_err(|e| anyhow!("Extraction failed: {}", e))?;

    Ok(())
}
