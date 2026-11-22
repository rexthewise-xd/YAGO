use crate::download_file;
use crate::github;
use anyhow::{anyhow, Result};
use std::path::Path;
use vfs::archive::extract_targz;

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
