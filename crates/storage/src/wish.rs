use crate::error::{LibrarianError, Result};
use std::path::{Path, PathBuf};

/// Scans the game directory for Chromium web cache `data_2` files and extracts
/// the most recent gacha history URL (Genshin Wishes, Star Rail Warps, ZZZ Signals, WuWa Convenes).
pub fn extract_gacha_url(install_path: &Path) -> Result<String> {
    if !install_path.exists() {
        return Err(LibrarianError::Validation(format!(
            "Install path does not exist: {:?}",
            install_path
        )));
    }

    let candidates = find_data_2_candidates(install_path);
    if candidates.is_empty() {
        return Err(LibrarianError::Validation(
            "No Chromium webCaches data_2 file found. Please open the in-game Wish / Convene History page first."
                .to_string(),
        ));
    }

    for path in &candidates {
        if let Ok(bytes) = std::fs::read(path) {
            if let Some(url) = parse_gacha_urls_from_bytes(&bytes) {
                return Ok(url);
            }
        }
    }

    Err(LibrarianError::Validation(
        "Gacha history URL not found in web cache. Please open the Wish / Signal / Convene history page in-game and try again."
            .to_string(),
    ))
}

fn find_data_2_candidates(install_path: &Path) -> Vec<PathBuf> {
    let mut data_2_files = Vec::new();

    // 1. Look specifically in known structures
    for entry in walkdir::WalkDir::new(install_path)
        .max_depth(7)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() && entry.file_name() == "data_2" {
            let path_str = entry.path().to_string_lossy().replace('\\', "/");
            if path_str.contains("webCaches") || path_str.contains("Cache_Data") {
                let mtime = entry
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                data_2_files.push((mtime, entry.path().to_path_buf()));
            }
        }
    }

    // Sort newest first
    data_2_files.sort_by(|a, b| b.0.cmp(&a.0));
    data_2_files.into_iter().map(|(_, p)| p).collect()
}

pub fn parse_gacha_urls_from_bytes(bytes: &[u8]) -> Option<String> {
    let mut matching_urls = Vec::new();
    let prefix = b"https://";

    let mut idx = 0;
    while idx + prefix.len() <= bytes.len() {
        if &bytes[idx..idx + prefix.len()] == prefix {
            let start = idx;
            let mut end = start + prefix.len();

            while end < bytes.len() {
                let b = bytes[end];
                // Allow valid URL characters
                let is_valid = match b {
                    b'a'..=b'z'
                    | b'A'..=b'Z'
                    | b'0'..=b'9'
                    | b'-'
                    | b'_'
                    | b'.'
                    | b'~'
                    | b'!'
                    | b'*'
                    | b'\''
                    | b'('
                    | b')'
                    | b';'
                    | b':'
                    | b'@'
                    | b'&'
                    | b'='
                    | b'+'
                    | b'$'
                    | b','
                    | b'/'
                    | b'?'
                    | b'%'
                    | b'#'
                    | b'['
                    | b']' => true,
                    _ => false,
                };

                if !is_valid {
                    break;
                }
                end += 1;
            }

            let slice = &bytes[start..end];
            if let Ok(url_str) = std::str::from_utf8(slice) {
                if is_gacha_endpoint(url_str) {
                    matching_urls.push(url_str.trim().to_string());
                }
            }

            idx = end;
        } else {
            idx += 1;
        }
    }

    // Return the last (most recent) matching URL
    matching_urls.pop()
}

fn is_gacha_endpoint(url: &str) -> bool {
    url.contains("e20190909gacha")
        || url.contains("getGachaLog")
        || url.contains("aki-gm-resources.aki-game.com/aki/gacha")
        || url.contains("aki/gacha")
        || (url.contains("authkey=")
            && (url.contains("gacha") || url.contains("history") || url.contains("event")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_extract_gacha_url_mock_blob() {
        let dir = tempdir().unwrap();
        let cache_dir = dir
            .path()
            .join("GenshinImpact_Data")
            .join("webCaches")
            .join("2.25.0.0")
            .join("Cache")
            .join("Cache_Data");
        std::fs::create_dir_all(&cache_dir).unwrap();
        let data_2_path = cache_dir.join("data_2");

        let older_url = "https://public-operation-hk4e.hoyoverse.com/gacha_info/api/getGachaLog?authkey_ver=1&authkey=old_key_12345";
        let newer_url = "https://gs.hoyoverse.com/genshin/event/e20190909gacha/index.html?authkey_ver=1&authkey=new_key_67890#/log";

        let mut blob = Vec::new();
        blob.extend_from_slice(b"random binary header \x00\x01\x02");
        blob.extend_from_slice(older_url.as_bytes());
        blob.extend_from_slice(b"\x00\x00 intermediate binary data \xFF\xFE");
        blob.extend_from_slice(newer_url.as_bytes());
        blob.extend_from_slice(b"\x00\x00 trailing bytes");

        std::fs::write(&data_2_path, blob).unwrap();

        let extracted = extract_gacha_url(dir.path()).unwrap();
        assert_eq!(extracted, newer_url);
    }

    #[test]
    fn test_extract_wuwa_convene_url() {
        let dir = tempdir().unwrap();
        let cache_dir = dir
            .path()
            .join("Client")
            .join("Saved")
            .join("webCaches")
            .join("Cache")
            .join("Cache_Data");
        std::fs::create_dir_all(&cache_dir).unwrap();
        let data_2_path = cache_dir.join("data_2");

        let wuwa_url = "https://aki-gm-resources.aki-game.com/aki/gacha/index.html#/record?record_id=1&server_id=prod";

        let mut blob = Vec::new();
        blob.extend_from_slice(b"\x01\x02");
        blob.extend_from_slice(wuwa_url.as_bytes());
        blob.extend_from_slice(b"\x00 end");

        std::fs::write(&data_2_path, blob).unwrap();

        let extracted = extract_gacha_url(dir.path()).unwrap();
        assert_eq!(extracted, wuwa_url);
    }
}
