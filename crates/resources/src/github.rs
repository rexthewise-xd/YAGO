use anyhow::{anyhow, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct ReleaseAsset {
    pub name: String,
    pub browser_download_url: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GithubRelease {
    pub tag_name: String,
    #[serde(default)]
    pub html_url: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub assets: Vec<ReleaseAsset>,
}

pub async fn get_latest_release(owner: &str, repo: &str) -> Result<GithubRelease> {
    let url = format!(
        "https://api.github.com/repos/{}/{}/releases/latest",
        owner, repo
    );
    let client = reqwest::Client::builder()
        .user_agent("YAGO-Quartermaster")
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let response = client.get(url).send().await?;

    if !response.status().is_success() {
        return Err(anyhow!("GitHub API request failed: {}", response.status()));
    }

    let mut release: GithubRelease = response.json().await?;

    // Filtering Logic
    if repo.to_lowercase().contains("proton") {
        release.assets.retain(|a| {
            let n = a.name.to_lowercase();
            n.ends_with(".tar.gz") || n.ends_with(".tar.xz") || n.ends_with(".tar.zst")
        });
    } else if repo == "WWMI-Package" || repo == "XXMITools" || repo == "GI-Model-Importer" {
        release.assets.retain(|a| a.name.ends_with(".zip"));
    }

    Ok(release)
}
