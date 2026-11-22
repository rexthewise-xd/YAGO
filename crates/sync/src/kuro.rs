use crate::error::{Result, SophonError};
use md5::{Digest, Md5};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

// Cache entry: path -> (cached_size, cached_mtime, cached_md5)
static HASH_CACHE: LazyLock<Mutex<HashMap<PathBuf, (u64, u64, String)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KuroFileEntry {
    pub dest: String,
    pub md5: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct KuroResourceManifest {
    #[serde(default)]
    pub resource: Vec<KuroFileEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct KuroCdnEntry {
    #[serde(default)]
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct KuroChannelInfo {
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub resources: String,
    #[serde(rename = "resourcesBasePath", default)]
    pub resources_base_path: String,
    #[serde(rename = "cdnList", default)]
    pub cdn_list: Vec<KuroCdnEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct KuroIndexResponse {
    pub default: KuroChannelInfo,
    #[serde(default)]
    pub predownload: Option<KuroChannelInfo>,
}

impl KuroIndexResponse {
    pub fn latest_version(&self) -> &str {
        &self.default.version
    }

    pub fn primary_cdn_url(&self) -> &str {
        self.default
            .cdn_list
            .first()
            .map(|c| c.url.as_str())
            .unwrap_or("")
    }

    pub fn resource_manifest_url(&self) -> String {
        let base = self.primary_cdn_url().trim_end_matches('/');
        let path = self.default.resources.trim_start_matches('/');
        if base.is_empty() {
            path.to_string()
        } else {
            format!("{}/{}", base, path)
        }
    }

    pub fn resources_base_path(&self) -> String {
        let base = self.primary_cdn_url().trim_end_matches('/');
        let path = self.default.resources_base_path.trim_start_matches('/');
        if base.is_empty() {
            path.to_string()
        } else {
            format!("{}/{}", base, path)
        }
    }
}

pub struct KuroClient {
    http: Client,
}

impl Default for KuroClient {
    fn default() -> Self {
        Self::new()
    }
}

impl KuroClient {
    pub fn new() -> Self {
        Self {
            http: Client::builder()
                .user_agent("YAGO-Quartermaster")
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
        }
    }

    pub async fn fetch_index(&self, index_url: &str) -> Result<KuroIndexResponse> {
        let resp = self
            .http
            .get(index_url)
            .send()
            .await
            .map_err(SophonError::Network)?;

        if !resp.status().is_success() {
            return Err(SophonError::Api(format!(
                "Kuro index request failed with status: {}",
                resp.status()
            )));
        }

        let body: KuroIndexResponse = resp.json().await.map_err(SophonError::Network)?;
        Ok(body)
    }

    pub async fn fetch_resources(&self, url: &str) -> Result<KuroResourceManifest> {
        let resp = self
            .http
            .get(url)
            .send()
            .await
            .map_err(SophonError::Network)?;

        if !resp.status().is_success() {
            return Err(SophonError::Api(format!(
                "Kuro resources request failed with status: {}",
                resp.status()
            )));
        }

        let body: KuroResourceManifest = resp.json().await.map_err(SophonError::Network)?;
        Ok(body)
    }

    /// Reads file in 1 MiB streaming chunks to prevent memory spikes on massive .pak files.
    /// Utilizes a fast (size, mtime) cache to avoid hashing unchanged files on every launch.
    pub fn verify_file_streaming_md5(
        path: &Path,
        expected_md5: &str,
        expected_size: u64,
    ) -> Result<bool> {
        if !path.exists() {
            return Ok(false);
        }

        let metadata = std::fs::metadata(path)?;
        if metadata.len() != expected_size {
            return Ok(false);
        }

        let mtime = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Check in-memory cache first
        let cached = {
            let cache = HASH_CACHE.lock().unwrap();
            cache.get(path).cloned()
        };

        if let Some((cached_size, cached_mtime, cached_hash)) = cached {
            if cached_size == metadata.len() && cached_mtime == mtime {
                return Ok(cached_hash.eq_ignore_ascii_case(expected_md5.trim()));
            }
        }

        // Stream and calculate MD5 in 1 MiB chunks
        let mut file = std::fs::File::open(path)?;
        let mut hasher = Md5::new();
        let mut buffer = vec![0u8; 1024 * 1024];

        loop {
            let count = file.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            hasher.update(&buffer[..count]);
        }

        let calculated = hex::encode(hasher.finalize());

        // Update cache
        {
            let mut cache = HASH_CACHE.lock().unwrap();
            cache.insert(
                path.to_path_buf(),
                (metadata.len(), mtime, calculated.clone()),
            );
        }

        Ok(calculated.eq_ignore_ascii_case(expected_md5.trim()))
    }

    /// Returns true for user save/config paths (e.g. Client/Saved/) to ensure
    /// integrity sweeps and cleanup routines never wipe user saves or Engine.ini.
    pub fn is_protected_user_path(rel_path: &str) -> bool {
        let normalized = rel_path.replace('\\', "/");
        let trimmed = normalized.trim_start_matches('/');
        let lower = trimmed.to_lowercase();

        lower.starts_with("client/saved/")
            || lower == "client/saved"
            || lower.contains("/client/saved/")
    }
}

pub fn is_protected_user_path(rel_path: &str) -> bool {
    KuroClient::is_protected_user_path(rel_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_deserialize_kuro_index_and_resource() {
        let index_json = r#"{
            "default": {
                "version": "2.3.1",
                "resources": "launcher/game/G153/2.3.1/resource.json",
                "resourcesBasePath": "launcher/game/G153/2.3.1/zip",
                "cdnList": [
                    { "url": "https://hw-pcdownload-qcloud.aki-game.net" },
                    { "url": "https://hw-pcdownload-aws.aki-game.net" }
                ]
            }
        }"#;

        let index: KuroIndexResponse = serde_json::from_str(index_json).unwrap();
        assert_eq!(index.latest_version(), "2.3.1");
        assert_eq!(
            index.resource_manifest_url(),
            "https://hw-pcdownload-qcloud.aki-game.net/launcher/game/G153/2.3.1/resource.json"
        );
        assert_eq!(
            index.resources_base_path(),
            "https://hw-pcdownload-qcloud.aki-game.net/launcher/game/G153/2.3.1/zip"
        );

        let resource_json = r#"{
            "resource": [
                {
                    "dest": "Client/Binaries/Win64/Client-Win64-Shipping.exe",
                    "md5": "abcdef1234567890abcdef1234567890",
                    "size": 104857600
                },
                {
                    "dest": "Client/Content/Paks/pakchunk1-WindowsNoEditor.pak",
                    "md5": "0987654321fedcba0987654321fedcba",
                    "size": 2147483648
                }
            ]
        }"#;

        let manifest: KuroResourceManifest = serde_json::from_str(resource_json).unwrap();
        assert_eq!(manifest.resource.len(), 2);
        assert_eq!(manifest.resource[0].size, 104857600);
        assert_eq!(
            manifest.resource[1].dest,
            "Client/Content/Paks/pakchunk1-WindowsNoEditor.pak"
        );
    }

    #[test]
    fn test_verify_file_streaming_md5() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test_file.bin");
        let content = b"YAGO Kuro Stream Verification Test Data";
        std::fs::write(&file_path, content).unwrap();

        let mut hasher = Md5::new();
        hasher.update(content);
        let expected_md5 = hex::encode(hasher.finalize());
        let expected_size = content.len() as u64;

        // 1. Valid match
        let pass = KuroClient::verify_file_streaming_md5(&file_path, &expected_md5, expected_size)
            .unwrap();
        assert!(pass, "File MD5 should match");

        // 2. Cached match hit
        let pass_cached =
            KuroClient::verify_file_streaming_md5(&file_path, &expected_md5, expected_size)
                .unwrap();
        assert!(pass_cached, "Cached MD5 should match");

        // 3. Mismatched MD5
        let fail = KuroClient::verify_file_streaming_md5(
            &file_path,
            "00000000000000000000000000000000",
            expected_size,
        )
        .unwrap();
        assert!(!fail, "Mismatched MD5 should return false");

        // 4. Mismatched size
        let fail_size =
            KuroClient::verify_file_streaming_md5(&file_path, &expected_md5, expected_size + 10)
                .unwrap();
        assert!(!fail_size, "Mismatched size should return false");
    }

    #[test]
    fn test_is_protected_user_path() {
        assert!(KuroClient::is_protected_user_path(
            "Client/Saved/Config/WindowsNoEditor/Engine.ini"
        ));
        assert!(KuroClient::is_protected_user_path(
            "Client\\Saved\\SaveGames\\User.sav"
        ));
        assert!(KuroClient::is_protected_user_path(
            "Client/Saved/Logs/UE4.log"
        ));
        assert!(!KuroClient::is_protected_user_path(
            "Client/Content/Paks/pakchunk1.pak"
        ));
        assert!(!KuroClient::is_protected_user_path("Wuthering Waves.exe"));
    }
}
