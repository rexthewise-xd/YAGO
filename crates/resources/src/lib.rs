pub mod assets;
pub mod effects;
pub mod gamebanana;
pub mod github;
pub mod loader;
pub mod proton;
pub mod reshade;

pub use assets::AssetCache;
pub use effects::{
    detect_duplicate_effects, install_or_update_effect, read_texfx_config, write_texfx_config,
    EffectLibraryStatus, SharedEffectLibrary, TexFxConfig,
};
pub use gamebanana::{
    game_id_to_gb_id, get_mod_detail, parse_yago_uri, search_mods, GbModDetail, GbModFile,
    GbModSummary, YagoDeepLink,
};
pub use proton::ProtonSource;

use anyhow::{anyhow, Result};
use futures_util::StreamExt;
use std::path::Path;
use std::time::Duration;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::sync::watch;

#[derive(Debug, Clone, serde::Serialize)]
pub struct DownloadProgress {
    pub current: u64,
    pub total: u64,
    pub percentage: f32,
}

/// Downloads a file with cancellation support, removing partial `.part` files if cancelled.
pub async fn download_file_cancellable<F>(
    url: &str,
    dest: &Path,
    mut cancel_rx: watch::Receiver<bool>,
    mut on_progress: F,
) -> Result<()>
where
    F: FnMut(u64, u64),
{
    let client = reqwest::Client::builder()
        .user_agent("YAGO-Quartermaster")
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(30))
        .build()?;

    if *cancel_rx.borrow() {
        return Err(anyhow!("Download cancelled before starting"));
    }

    let response = client.get(url).send().await?;

    if !response.status().is_success() {
        return Err(anyhow!("Request failed with status: {}", response.status()));
    }

    let total_size = response
        .content_length()
        .ok_or_else(|| anyhow!("Failed to get content length"))?;

    // Ensure parent directory exists
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let part_path = dest.with_extension("part");

    let mut file = File::create(&part_path).await?;
    let mut downloaded: u64 = 0;
    let mut stream = response.bytes_stream();
    let mut cancelled = false;

    loop {
        tokio::select! {
            res = cancel_rx.changed() => {
                if res.is_ok() && *cancel_rx.borrow() {
                    cancelled = true;
                    break;
                }
            }
            item = stream.next() => {
                match item {
                    Some(res) => {
                        match res {
                            Ok(chunk) => {
                                file.write_all(&chunk).await?;
                                downloaded += chunk.len() as u64;
                                on_progress(downloaded, total_size);
                            }
                            Err(e) => {
                                drop(file);
                                let _ = tokio::fs::remove_file(&part_path).await;
                                return Err(e.into());
                            }
                        }
                    }
                    None => break,
                }
            }
        }
    }

    drop(file);

    if cancelled {
        let _ = tokio::fs::remove_file(&part_path).await;
        return Err(anyhow!("Download cancelled"));
    }

    tokio::fs::rename(&part_path, dest).await?;
    Ok(())
}

/// Standard backward-compatible download helper.
pub async fn download_file<F>(url: &str, dest: &Path, on_progress: F) -> Result<()>
where
    F: FnMut(u64, u64),
{
    let (_tx, rx) = watch::channel(false);
    download_file_cancellable(url, dest, rx, on_progress).await
}
