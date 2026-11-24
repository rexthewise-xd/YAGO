use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegistrySource {
    pub id: String,
    pub name: String,
    pub url: String,
    pub enabled: bool,
    pub official: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegistryIndex {
    pub schema_version: u32,
    pub registry_version: String,
    #[serde(default)]
    pub min_yago_version: Option<String>,
    pub updated_at: String,
    pub entries: Vec<RegistryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegistryEntry {
    pub path: String,
    pub category: String, // "game" | "loader" | "runner" | "effect" | "hash" | "fixer" | "script"
    pub sha256: String,
    #[serde(default)]
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct RegistrySyncReport {
    pub registry_id: String,
    pub registry_version: String,
    pub downloaded: usize,
    pub skipped_unchanged: usize,
    pub failed: Vec<String>,
}

/// Computes the lowercase hexadecimal SHA-256 digest of the given bytes.
pub fn compute_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// Validates that a relative path does not attempt path traversal or use absolute formatting.
fn validate_relative_path(rel_path: &str) -> Result<String> {
    let normalized = rel_path.replace('\\', "/");
    let trimmed = normalized.trim();
    if trimmed.is_empty()
        || trimmed.starts_with('/')
        || trimmed.starts_with("./")
        || trimmed.split('/').any(|part| part == "..")
    {
        return Err(anyhow!(
            "Invalid relative path or path traversal detected: {}",
            rel_path
        ));
    }
    Ok(trimmed.to_string())
}

/// Fetches bytes from either an HTTP(S) URL or a local file:// path.
pub async fn fetch_bytes(base_url: &str, rel_path: &str) -> Result<Vec<u8>> {
    let safe_rel = validate_relative_path(rel_path)?;

    if let Some(file_path_str) = base_url.strip_prefix("file://") {
        let base_path = PathBuf::from(file_path_str);
        let target_path = base_path.join(safe_rel);
        std::fs::read(&target_path)
            .map_err(|e| anyhow!("Failed to read local file {:?}: {}", target_path, e))
    } else if base_url.starts_with("http://") || base_url.starts_with("https://") {
        let client = reqwest::Client::builder()
            .user_agent("YAGO-Registry")
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(30))
            .build()?;

        let full_url = format!("{}/{}", base_url.trim_end_matches('/'), safe_rel);
        let res = client.get(&full_url).send().await?;

        if !res.status().is_success() {
            return Err(anyhow!(
                "Failed to fetch {}: HTTP {}",
                full_url,
                res.status()
            ));
        }

        let bytes = res.bytes().await?.to_vec();
        Ok(bytes)
    } else {
        Err(anyhow!(
            "Unsupported registry URL scheme (expected https://, http://, or file://): {}",
            base_url
        ))
    }
}

/// Returns true if version `a` is strictly newer than version `b`.
fn is_version_newer(a: &str, b: &str) -> bool {
    let parse_parts = |v: &str| -> Vec<u64> {
        v.trim_start_matches(['v', 'V'])
            .split('.')
            .filter_map(|s| {
                s.chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect::<String>()
                    .parse::<u64>()
                    .ok()
            })
            .collect()
    };

    let a_parts = parse_parts(a);
    let b_parts = parse_parts(b);

    for (p_a, p_b) in a_parts.iter().zip(b_parts.iter()) {
        if p_a > p_b {
            return true;
        } else if p_a < p_b {
            return false;
        }
    }

    a_parts.len() > b_parts.len()
}

/// Synchronizes a registry source into `cache_root/<source.id>/`, validating SHA-256 hashes
/// and skipping already synchronized, unchanged files.
pub async fn sync_registry(
    source: &RegistrySource,
    cache_root: &Path,
    current_yago_version: &str,
) -> Result<RegistrySyncReport> {
    let index_bytes = fetch_bytes(&source.url, "index.json").await?;
    let index: RegistryIndex = serde_json::from_slice(&index_bytes).map_err(|e| {
        anyhow!(
            "Failed to parse registry index.json from {}: {}",
            source.url,
            e
        )
    })?;

    // Check min_yago_version if specified
    if let Some(min_ver) = &index.min_yago_version {
        if is_version_newer(min_ver, current_yago_version) {
            return Err(anyhow!(
                "Registry {} requires YAGO v{} or newer (current version: v{})",
                source.name,
                min_ver,
                current_yago_version
            ));
        }
    }

    let reg_dir = cache_root.join(&source.id);
    let _ = std::fs::create_dir_all(&reg_dir);

    let mut downloaded = 0;
    let mut skipped_unchanged = 0;
    let mut failed = Vec::new();

    for entry in &index.entries {
        let safe_rel = match validate_relative_path(&entry.path) {
            Ok(p) => p,
            Err(_) => {
                failed.push(entry.path.clone());
                continue;
            }
        };

        let dest_path = reg_dir.join(&safe_rel);

        // Check if existing file matches SHA-256
        if dest_path.exists() {
            if let Ok(existing) = std::fs::read(&dest_path) {
                if compute_sha256(&existing).eq_ignore_ascii_case(&entry.sha256) {
                    skipped_unchanged += 1;
                    continue;
                }
            }
        }

        // Fetch and verify content
        match fetch_bytes(&source.url, &safe_rel).await {
            Ok(bytes) => {
                let computed = compute_sha256(&bytes);
                if !computed.eq_ignore_ascii_case(&entry.sha256) {
                    eprintln!(
                        "SHA-256 mismatch for {}: expected {}, got {}",
                        entry.path, entry.sha256, computed
                    );
                    failed.push(entry.path.clone());
                    continue;
                }

                // Write atomically
                if let Some(parent) = dest_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }

                let tmp_path = dest_path.with_extension(format!(
                    "{}.tmp",
                    dest_path
                        .extension()
                        .and_then(|s| s.to_str())
                        .unwrap_or("tmp")
                ));

                if std::fs::write(&tmp_path, &bytes).is_ok()
                    && std::fs::rename(&tmp_path, &dest_path).is_ok()
                {
                    downloaded += 1;
                } else {
                    let _ = std::fs::remove_file(&tmp_path);
                    failed.push(entry.path.clone());
                }
            }
            Err(e) => {
                eprintln!("Failed to fetch registry entry {}: {}", entry.path, e);
                failed.push(entry.path.clone());
            }
        }
    }

    // Save index.json atomically
    let index_dest = reg_dir.join("index.json");
    let index_tmp = reg_dir.join("index.json.tmp");
    if std::fs::write(&index_tmp, &index_bytes).is_ok() {
        let _ = std::fs::rename(&index_tmp, &index_dest);
    }

    Ok(RegistrySyncReport {
        registry_id: source.id.clone(),
        registry_version: index.registry_version,
        downloaded,
        skipped_unchanged,
        failed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_sync_local_file_registry_and_skip_unchanged() {
        let remote_dir = tempdir().unwrap();
        let cache_dir = tempdir().unwrap();

        // 1. Prepare files in remote_dir
        let file1_rel = "games/genshin.json";
        let file1_path = remote_dir.path().join(file1_rel);
        std::fs::create_dir_all(file1_path.parent().unwrap()).unwrap();
        let file1_content = b"{\"game\": \"genshin\", \"version\": \"5.2\"}";
        std::fs::write(&file1_path, file1_content).unwrap();

        let file2_rel = "effects/orfix.json";
        let file2_path = remote_dir.path().join(file2_rel);
        std::fs::create_dir_all(file2_path.parent().unwrap()).unwrap();
        let file2_content = b"{\"effect\": \"orfix\", \"enabled\": true}";
        std::fs::write(&file2_path, file2_content).unwrap();

        let index = RegistryIndex {
            schema_version: 1,
            registry_version: "1.0.0".to_string(),
            min_yago_version: Some("0.1.0".to_string()),
            updated_at: "2026-11-24T00:00:00Z".to_string(),
            entries: vec![
                RegistryEntry {
                    path: file1_rel.to_string(),
                    category: "game".to_string(),
                    sha256: compute_sha256(file1_content),
                    size_bytes: Some(file1_content.len() as u64),
                },
                RegistryEntry {
                    path: file2_rel.to_string(),
                    category: "effect".to_string(),
                    sha256: compute_sha256(file2_content),
                    size_bytes: Some(file2_content.len() as u64),
                },
            ],
        };

        let index_bytes = serde_json::to_vec_pretty(&index).unwrap();
        std::fs::write(remote_dir.path().join("index.json"), index_bytes).unwrap();

        let source = RegistrySource {
            id: "test-reg".to_string(),
            name: "Test Local Registry".to_string(),
            url: format!("file://{}", remote_dir.path().display()),
            enabled: true,
            official: false,
        };

        // First sync: both files downloaded
        let report1 = sync_registry(&source, cache_dir.path(), "0.1.0")
            .await
            .unwrap();
        assert_eq!(report1.downloaded, 2);
        assert_eq!(report1.skipped_unchanged, 0);
        assert!(report1.failed.is_empty());
        assert!(cache_dir.path().join("test-reg").join(file1_rel).exists());
        assert!(cache_dir.path().join("test-reg").join(file2_rel).exists());
        assert!(cache_dir
            .path()
            .join("test-reg")
            .join("index.json")
            .exists());

        // Second sync: both files skipped unchanged
        let report2 = sync_registry(&source, cache_dir.path(), "0.1.0")
            .await
            .unwrap();
        assert_eq!(report2.downloaded, 0);
        assert_eq!(report2.skipped_unchanged, 2);
        assert!(report2.failed.is_empty());
    }

    #[tokio::test]
    async fn test_sync_rejects_sha256_mismatch() {
        let remote_dir = tempdir().unwrap();
        let cache_dir = tempdir().unwrap();

        let file_rel = "games/corrupt.json";
        let file_path = remote_dir.path().join(file_rel);
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        let actual_content = b"actual real content";
        std::fs::write(&file_path, actual_content).unwrap();

        let fake_sha256 = "0000000000000000000000000000000000000000000000000000000000000000";

        let index = RegistryIndex {
            schema_version: 1,
            registry_version: "1.0.0".to_string(),
            min_yago_version: None,
            updated_at: "2026-11-24T00:00:00Z".to_string(),
            entries: vec![RegistryEntry {
                path: file_rel.to_string(),
                category: "game".to_string(),
                sha256: fake_sha256.to_string(),
                size_bytes: Some(actual_content.len() as u64),
            }],
        };

        let index_bytes = serde_json::to_vec_pretty(&index).unwrap();
        std::fs::write(remote_dir.path().join("index.json"), index_bytes).unwrap();

        let source = RegistrySource {
            id: "mismatch-reg".to_string(),
            name: "Mismatch Registry".to_string(),
            url: format!("file://{}", remote_dir.path().display()),
            enabled: true,
            official: false,
        };

        let report = sync_registry(&source, cache_dir.path(), "0.1.0")
            .await
            .unwrap();
        assert_eq!(report.downloaded, 0);
        assert_eq!(report.skipped_unchanged, 0);
        assert_eq!(report.failed, vec![file_rel.to_string()]);
        assert!(!cache_dir
            .path()
            .join("mismatch-reg")
            .join(file_rel)
            .exists());
    }
}
