use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

pub fn game_id_to_gb_id(game_id: &str) -> Option<u64> {
    let lower = game_id.to_lowercase();
    if lower.contains("genshin") || lower == "8552" {
        Some(8552)
    } else if lower.contains("starrail")
        || lower.contains("star_rail")
        || lower.contains("star rail")
        || lower == "18366"
    {
        Some(18366)
    } else if lower.contains("zzz") || lower.contains("zenless") || lower == "19567" {
        Some(19567)
    } else if lower.contains("wuwa") || lower.contains("wuthering") || lower == "20357" {
        Some(20357)
    } else if lower.contains("hi3")
        || lower.contains("honkai3rd")
        || lower.contains("honkai impact")
        || lower == "10349"
    {
        Some(10349)
    } else {
        None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GbModSummary {
    pub id: u64,
    pub name: String,
    pub author: String,
    pub thumbnail_url: Option<String>,
    pub like_count: u64,
    pub view_count: u64,
    pub date_updated: u64,
    pub is_nsfw: bool,
    pub profile_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GbModFile {
    pub id: u64,
    pub filename: String,
    pub download_url: String,
    pub filesize: u64,
    pub description: String,
    pub date_added: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GbModDetail {
    pub summary: GbModSummary,
    pub description: String,
    pub files: Vec<GbModFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct YagoDeepLink {
    pub mod_id: u64,
    pub download_url: Option<String>,
    pub file_id: Option<u64>,
}

pub fn parse_yago_uri(uri: &str) -> Result<YagoDeepLink> {
    let trimmed = uri.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("Empty URI"));
    }

    let decoded = urlencoding::decode(trimmed)
        .map(|c| c.into_owned())
        .unwrap_or_else(|_| trimmed.to_string());
    let s = decoded.trim();

    // 1. Web URL: https://gamebanana.com/mods/<id>
    if s.starts_with("http://") || s.starts_with("https://") {
        if let Some(idx) = s.find("/mods/") {
            let after = &s[idx + 6..];
            let id_str: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(mod_id) = id_str.parse::<u64>() {
                return Ok(YagoDeepLink {
                    mod_id,
                    download_url: None,
                    file_id: None,
                });
            }
        }
    }

    // 2. yago:// custom scheme
    if s.to_lowercase().starts_with("yago://") {
        let remainder = &s[7..];

        // Format: yago://<download_url>,Mod,<mod_id>
        let lower_rem = remainder.to_lowercase();
        if let Some(pos) = lower_rem.find(",mod,") {
            let dl_part = remainder[..pos].trim();
            let id_part = remainder[pos + 5..].trim();

            let id_digits: String = id_part.chars().take_while(|c| c.is_ascii_digit()).collect();
            let mod_id = id_digits
                .parse::<u64>()
                .map_err(|e| anyhow!("Invalid mod ID in deep link: {}", e))?;

            let file_id = if let Some(dl_pos) = dl_part.rfind("/dl/") {
                let after = &dl_part[dl_pos + 4..];
                let f_digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
                f_digits.parse::<u64>().ok()
            } else {
                None
            };

            return Ok(YagoDeepLink {
                mod_id,
                download_url: if dl_part.is_empty() {
                    None
                } else {
                    Some(dl_part.to_string())
                },
                file_id,
            });
        }

        // Format: yago://install/Mod/<mod_id> or yago://Mod/<mod_id>
        let prefixes = ["install/mod/", "install/Mod/", "mod/", "Mod/"];
        for p in prefixes {
            if remainder.starts_with(p) {
                let id_str = &remainder[p.len()..];
                let id_digits: String = id_str.chars().take_while(|c| c.is_ascii_digit()).collect();
                if let Ok(mod_id) = id_digits.parse::<u64>() {
                    return Ok(YagoDeepLink {
                        mod_id,
                        download_url: None,
                        file_id: None,
                    });
                }
            }
        }

        // Bare number after yago://
        let id_digits: String = remainder
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        if let Ok(mod_id) = id_digits.parse::<u64>() {
            return Ok(YagoDeepLink {
                mod_id,
                download_url: None,
                file_id: None,
            });
        }
    }

    // 3. Fallback: bare number string
    if s.chars().all(|c| c.is_ascii_digit()) {
        if let Ok(mod_id) = s.parse::<u64>() {
            return Ok(YagoDeepLink {
                mod_id,
                download_url: None,
                file_id: None,
            });
        }
    }

    Err(anyhow!("Could not parse GameBanana mod URI: {}", uri))
}

pub async fn search_mods(
    gb_game_id: u64,
    query: &str,
    page: u32,
    include_nsfw: bool,
) -> Result<Vec<GbModSummary>> {
    let q = query.trim();
    let url = if q.is_empty() {
        format!(
            "https://gamebanana.com/apiv11/Game/{}/Subfeed?_nPage={}&_nPerpage=20",
            gb_game_id, page
        )
    } else {
        format!(
            "https://gamebanana.com/apiv11/Util/Search/Results?_sSearchString={}&_idGameRow={}&_nPage={}&_nPerpage=20",
            urlencoding::encode(q),
            gb_game_id,
            page
        )
    };

    let client = reqwest::Client::builder()
        .user_agent("YAGO-Organizer")
        .timeout(std::time::Duration::from_secs(15))
        .build()?;

    let res = client.get(&url).send().await?;
    if !res.status().is_success() {
        return Err(anyhow!("GameBanana API returned status {}", res.status()));
    }

    let json_val: serde_json::Value = res.json().await?;
    parse_search_records(&json_val, include_nsfw)
}

pub fn parse_search_records(
    val: &serde_json::Value,
    include_nsfw: bool,
) -> Result<Vec<GbModSummary>> {
    let mut results = Vec::new();

    let records = val
        .get("_aRecords")
        .and_then(|r| r.as_array())
        .ok_or_else(|| anyhow!("Missing _aRecords in response"))?;

    for r in records {
        let model = r.get("_sModelName").and_then(|m| m.as_str()).unwrap_or("");
        if model == "Question" || model == "Request" {
            continue;
        }

        let id = match r.get("_idRow").and_then(|id| id.as_u64()) {
            Some(id) => id,
            None => continue,
        };

        let name = r
            .get("_sName")
            .and_then(|n| n.as_str())
            .unwrap_or("Untitled")
            .to_string();

        let author = r
            .get("_aSubmitter")
            .and_then(|sub| sub.get("_sName"))
            .and_then(|n| n.as_str())
            .unwrap_or("Unknown")
            .to_string();

        let like_count = r.get("_nLikeCount").and_then(|c| c.as_u64()).unwrap_or(0);
        let view_count = r.get("_nViewCount").and_then(|c| c.as_u64()).unwrap_or(0);

        let date_updated = r
            .get("_tsDateUpdated")
            .and_then(|d| d.as_u64())
            .or_else(|| r.get("_tsDateModified").and_then(|d| d.as_u64()))
            .or_else(|| r.get("_tsDateAdded").and_then(|d| d.as_u64()))
            .unwrap_or(0);

        let is_nsfw = r
            .get("_bHasContentRatings")
            .and_then(|b| b.as_bool())
            .unwrap_or(false);

        if !include_nsfw && is_nsfw {
            continue;
        }

        let profile_url = r
            .get("_sProfileUrl")
            .and_then(|u| u.as_str())
            .map(|u| u.to_string())
            .unwrap_or_else(|| format!("https://gamebanana.com/mods/{}", id));

        let thumbnail_url = r
            .get("_aPreviewMedia")
            .and_then(|pm| pm.get("_aImages"))
            .and_then(|imgs| imgs.as_array())
            .and_then(|arr| arr.first())
            .and_then(|img| {
                let base = img.get("_sBaseUrl").and_then(|b| b.as_str())?;
                let file = img
                    .get("_sFile220")
                    .or_else(|| img.get("_sFile"))
                    .and_then(|f| f.as_str())?;
                Some(format!("{}/{}", base, file))
            });

        results.push(GbModSummary {
            id,
            name,
            author,
            thumbnail_url,
            like_count,
            view_count,
            date_updated,
            is_nsfw,
            profile_url,
        });
    }

    Ok(results)
}

pub async fn get_mod_detail(mod_id: u64) -> Result<GbModDetail> {
    let url = format!("https://gamebanana.com/apiv11/Mod/{}/ProfilePage", mod_id);

    let client = reqwest::Client::builder()
        .user_agent("YAGO-Organizer")
        .timeout(std::time::Duration::from_secs(15))
        .build()?;

    let res = client.get(&url).send().await?;
    if !res.status().is_success() {
        return Err(anyhow!(
            "GameBanana API returned status {} for mod {}",
            res.status(),
            mod_id
        ));
    }

    let val: serde_json::Value = res.json().await?;
    parse_mod_detail(mod_id, &val)
}

pub fn parse_mod_detail(mod_id: u64, val: &serde_json::Value) -> Result<GbModDetail> {
    let id = val.get("_idRow").and_then(|v| v.as_u64()).unwrap_or(mod_id);
    let name = val
        .get("_sName")
        .and_then(|n| n.as_str())
        .unwrap_or("Untitled")
        .to_string();

    let author = val
        .get("_aSubmitter")
        .and_then(|sub| sub.get("_sName"))
        .and_then(|n| n.as_str())
        .unwrap_or("Unknown")
        .to_string();

    let description = val
        .get("_sText")
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string();

    let like_count = val.get("_nLikeCount").and_then(|c| c.as_u64()).unwrap_or(0);
    let view_count = val.get("_nViewCount").and_then(|c| c.as_u64()).unwrap_or(0);

    let date_updated = val
        .get("_tsDateUpdated")
        .and_then(|d| d.as_u64())
        .or_else(|| val.get("_tsDateModified").and_then(|d| d.as_u64()))
        .or_else(|| val.get("_tsDateAdded").and_then(|d| d.as_u64()))
        .unwrap_or(0);

    let is_nsfw = val
        .get("_bHasContentRatings")
        .and_then(|b| b.as_bool())
        .unwrap_or(false);

    let profile_url = val
        .get("_sProfileUrl")
        .and_then(|u| u.as_str())
        .map(|u| u.to_string())
        .unwrap_or_else(|| format!("https://gamebanana.com/mods/{}", id));

    let thumbnail_url = val
        .get("_aPreviewMedia")
        .and_then(|pm| pm.get("_aImages"))
        .and_then(|imgs| imgs.as_array())
        .and_then(|arr| arr.first())
        .and_then(|img| {
            let base = img.get("_sBaseUrl").and_then(|b| b.as_str())?;
            let file = img
                .get("_sFile220")
                .or_else(|| img.get("_sFile"))
                .and_then(|f| f.as_str())?;
            Some(format!("{}/{}", base, file))
        });

    let mut files = Vec::new();
    if let Some(files_arr) = val.get("_aFiles").and_then(|f| f.as_array()) {
        for f in files_arr {
            let fid = f.get("_idRow").and_then(|id| id.as_u64()).unwrap_or(0);
            let filename = f
                .get("_sFile")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string();
            let download_url = f
                .get("_sDownloadUrl")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string();
            let filesize = f.get("_nFilesize").and_then(|s| s.as_u64()).unwrap_or(0);
            let desc = f
                .get("_sDescription")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string();
            let date_added = f.get("_tsDateAdded").and_then(|s| s.as_u64()).unwrap_or(0);

            if !download_url.is_empty() {
                files.push(GbModFile {
                    id: fid,
                    filename,
                    download_url,
                    filesize,
                    description: desc,
                    date_added,
                });
            }
        }
    }

    Ok(GbModDetail {
        summary: GbModSummary {
            id,
            name,
            author,
            thumbnail_url,
            like_count,
            view_count,
            date_updated,
            is_nsfw,
            profile_url,
        },
        description,
        files,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_yago_uri_formats() {
        // 1. yago://install/Mod/<mod_id>
        let l1 = parse_yago_uri("yago://install/Mod/705202").unwrap();
        assert_eq!(l1.mod_id, 705202);
        assert_eq!(l1.download_url, None);
        assert_eq!(l1.file_id, None);

        // 2. yago://<download_url>,Mod,<mod_id>
        let l2 = parse_yago_uri("yago://https://gamebanana.com/dl/1898604,Mod,705202").unwrap();
        assert_eq!(l2.mod_id, 705202);
        assert_eq!(
            l2.download_url.as_deref(),
            Some("https://gamebanana.com/dl/1898604")
        );
        assert_eq!(l2.file_id, Some(1898604));

        // 3. Web URL: https://gamebanana.com/mods/705202
        let l3 = parse_yago_uri("https://gamebanana.com/mods/705202").unwrap();
        assert_eq!(l3.mod_id, 705202);
        assert_eq!(l3.download_url, None);

        // 4. URL-encoded format
        let l4 =
            parse_yago_uri("yago://https%3A%2F%2Fgamebanana.com%2Fdl%2F1898604%2CMod%2C705202")
                .unwrap();
        assert_eq!(l4.mod_id, 705202);
        assert_eq!(l4.file_id, Some(1898604));

        // 5. Bare ID
        let l5 = parse_yago_uri("705202").unwrap();
        assert_eq!(l5.mod_id, 705202);
    }

    #[test]
    fn test_mock_json_deserialization() {
        let json_text = r#"{
            "_aMetadata": { "_nRecordCount": 1 },
            "_aRecords": [
                {
                    "_idRow": 705202,
                    "_sModelName": "Mod",
                    "_sName": "Odette Tweaks",
                    "_sProfileUrl": "https://gamebanana.com/mods/705202",
                    "_tsDateUpdated": 1795281431,
                    "_nLikeCount": 42,
                    "_nViewCount": 1200,
                    "_bHasContentRatings": false,
                    "_aSubmitter": { "_sName": "LeoTorrez" },
                    "_aPreviewMedia": {
                        "_aImages": [
                            {
                                "_sBaseUrl": "https://images.gamebanana.com/img/ss/mods",
                                "_sFile": "preview.jpg",
                                "_sFile220": "220-preview.jpg"
                            }
                        ]
                    }
                }
            ]
        }"#;

        let val: serde_json::Value = serde_json::from_str(json_text).unwrap();
        let summaries = parse_search_records(&val, false).unwrap();
        assert_eq!(summaries.len(), 1);
        let s = &summaries[0];
        assert_eq!(s.id, 705202);
        assert_eq!(s.name, "Odette Tweaks");
        assert_eq!(s.author, "LeoTorrez");
        assert_eq!(s.like_count, 42);
        assert_eq!(s.view_count, 1200);
        assert_eq!(
            s.thumbnail_url.as_deref(),
            Some("https://images.gamebanana.com/img/ss/mods/220-preview.jpg")
        );
        assert!(!s.is_nsfw);

        let detail_json = r#"{
            "_idRow": 705202,
            "_sName": "Odette Tweaks",
            "_sText": "Detailed description of the mod",
            "_nLikeCount": 42,
            "_nViewCount": 1200,
            "_tsDateUpdated": 1795281431,
            "_bHasContentRatings": false,
            "_sProfileUrl": "https://gamebanana.com/mods/705202",
            "_aSubmitter": { "_sName": "LeoTorrez" },
            "_aPreviewMedia": { "_aImages": [] },
            "_aFiles": [
                {
                    "_idRow": 1898604,
                    "_sFile": "odettetweaks.zip",
                    "_nFilesize": 8547795,
                    "_tsDateAdded": 1795281357,
                    "_sDownloadUrl": "https://gamebanana.com/dl/1898604",
                    "_sDescription": "Main zip"
                }
            ]
        }"#;

        let d_val: serde_json::Value = serde_json::from_str(detail_json).unwrap();
        let detail = parse_mod_detail(705202, &d_val).unwrap();
        assert_eq!(detail.summary.id, 705202);
        assert_eq!(detail.description, "Detailed description of the mod");
        assert_eq!(detail.files.len(), 1);
        assert_eq!(detail.files[0].id, 1898604);
        assert_eq!(detail.files[0].filename, "odettetweaks.zip");
    }
}
