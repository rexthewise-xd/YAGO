use crate::error::{LibrarianError, Result};
use crate::models::{LibraryDatabase, Profile, VariantPreset};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;
use uuid::Uuid;
use zip::write::SimpleFileOptions;

pub fn export_profile_for_sharing(profile: &Profile) -> String {
    format!(
        "YAGO_PROFILE_V1|{}|{}|MODS:{}",
        profile.name.to_uppercase(),
        profile.id,
        profile.enabled_mod_ids.len()
    )
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BundleModEntry {
    pub name: String,
    pub character: String,
    #[serde(default)]
    pub gb_mod_id: Option<u64>,
    #[serde(default)]
    pub active_preset: Option<VariantPreset>,
    pub enabled: bool,
    pub load_order_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LoadoutBundle {
    pub version: u32,
    pub game_id: String,
    pub profile_name: String,
    pub description: String,
    pub mods: Vec<BundleModEntry>,
}

/// Serializes a profile and its associated mods into a shareable JSON loadout bundle.
pub fn export_loadout_bundle(
    db: &LibraryDatabase,
    game_id: &str,
    profile_id: Uuid,
) -> Result<String> {
    let profile = db
        .profiles
        .get(&profile_id)
        .ok_or_else(|| LibrarianError::ProfileError(format!("Profile {} not found", profile_id)))?;

    let mut bundle_mods = Vec::new();

    // 1. Mods in load order
    for (idx, mid) in profile.load_order.iter().enumerate() {
        if let Some(record) = db.mods.get(mid) {
            let active_preset = record.config.active_preset_id.as_ref().and_then(|pid| {
                record
                    .config
                    .variant_presets
                    .iter()
                    .find(|p| &p.id == pid)
                    .cloned()
            });

            bundle_mods.push(BundleModEntry {
                name: record.meta.name.clone(),
                character: record.compatibility.character.clone(),
                gb_mod_id: record.meta.gb_mod_id,
                active_preset,
                enabled: profile.enabled_mod_ids.contains(mid),
                load_order_index: idx,
            });
        }
    }

    // 2. Any enabled mods not explicitly in load order
    let mut next_idx = bundle_mods.len();
    for mid in &profile.enabled_mod_ids {
        if !profile.load_order.contains(mid) {
            if let Some(record) = db.mods.get(mid) {
                let active_preset = record.config.active_preset_id.as_ref().and_then(|pid| {
                    record
                        .config
                        .variant_presets
                        .iter()
                        .find(|p| &p.id == pid)
                        .cloned()
                });

                bundle_mods.push(BundleModEntry {
                    name: record.meta.name.clone(),
                    character: record.compatibility.character.clone(),
                    gb_mod_id: record.meta.gb_mod_id,
                    active_preset,
                    enabled: true,
                    load_order_index: next_idx,
                });
                next_idx += 1;
            }
        }
    }

    let bundle = LoadoutBundle {
        version: 1,
        game_id: game_id.to_string(),
        profile_name: profile.name.clone(),
        description: profile.description.clone(),
        mods: bundle_mods,
    };

    serde_json::to_string_pretty(&bundle).map_err(LibrarianError::Json)
}

/// Parses a JSON loadout bundle.
pub fn parse_loadout_bundle(json: &str) -> Result<LoadoutBundle> {
    serde_json::from_str(json).map_err(LibrarianError::Json)
}

/// Zips the mod directory, stripping `.yago_backups/` and uncloaking any `*.ini.yago_source`
/// files back to `*.ini` so the exported `.zip` is immediately usable in vanilla 3DMigoto/XXMI.
pub fn export_clean_mod_zip(mod_root: &Path, dest_zip: &Path) -> Result<usize> {
    if !mod_root.exists() {
        return Err(LibrarianError::Validation(format!(
            "Mod root path does not exist: {:?}",
            mod_root
        )));
    }

    if let Some(parent) = dest_zip.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Map: target_rel_path -> (source_path, is_priority_clean_source)
    let mut files_to_zip: HashMap<String, (std::path::PathBuf, bool)> = HashMap::new();

    for entry in walkdir::WalkDir::new(mod_root)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            let rel = match entry.path().strip_prefix(mod_root) {
                Ok(r) => r,
                Err(_) => continue,
            };
            let rel_str = rel.to_string_lossy().replace('\\', "/");

            // Skip internal backup artifacts
            if rel_str.contains(".yago_backups") {
                continue;
            }

            if rel_str.ends_with(".ini.yago_source") {
                let clean_rel = rel_str.strip_suffix(".yago_source").unwrap().to_string();
                files_to_zip.insert(clean_rel, (entry.path().to_path_buf(), true));
            } else if !files_to_zip
                .get(&rel_str)
                .map(|(_, prio)| *prio)
                .unwrap_or(false)
            {
                files_to_zip.insert(rel_str, (entry.path().to_path_buf(), false));
            }
        }
    }

    let file = std::fs::File::create(dest_zip)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let mut written_count = 0usize;
    for (rel_path, (src_path, _)) in files_to_zip {
        let mut f = std::fs::File::open(&src_path)?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf)?;

        zip.start_file(rel_path, options)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        zip.write_all(&buf)?;
        written_count += 1;
    }

    zip.finish()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

    Ok(written_count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{GameConfig, ModConfig, ModMetadata, ModRecord};
    use chrono::Utc;
    use insta::assert_yaml_snapshot;
    use tempfile::tempdir;
    use uuid::Uuid;

    #[test]
    fn test_profile_export_snapshot() {
        let profile = Profile {
            id: Uuid::nil(),
            name: "Raiden's Eternal Storm".to_string(),
            description: "A high-perf mod loadout for the Sovereign.".to_string(),
            enabled_mod_ids: vec![Uuid::nil()],
            load_order: vec![Uuid::nil()],
            added_at: Utc::now(),
            ..Default::default()
        };

        let exported = export_profile_for_sharing(&profile);
        // We snapshot the raw string as YAML for better readability
        assert_yaml_snapshot!("profile_export_output", exported);
    }

    #[test]
    fn test_loadout_bundle_export_and_parse_roundtrip() {
        let mut db = LibraryDatabase::default();
        let game_id = "genshin";
        let profile_id = Uuid::new_v4();
        let mod_id = Uuid::new_v4();

        let profile = Profile {
            id: profile_id,
            name: "Abyss Raiden & Keqing".to_string(),
            description: "Custom community loadout bundle".to_string(),
            enabled_mod_ids: vec![mod_id],
            load_order: vec![mod_id],
            added_at: Utc::now(),
            ..Default::default()
        };

        let record = ModRecord {
            id: mod_id,
            owner_game_id: game_id.to_string(),
            path: std::path::PathBuf::from("/mods/raiden_shogun"),
            size: "10 MB".to_string(),
            meta: ModMetadata {
                name: "Raiden Shogun Boss Armor".to_string(),
                version: "1.2".to_string(),
                author: "Modeler".to_string(),
                url: None,
                preview_image: None,
                description: None,
                gb_mod_id: Some(54321),
                gb_date_updated: None,
            },
            compatibility: crate::models::ModCompatibility {
                game: game_id.to_string(),
                character: "Raiden".to_string(),
                hashes: vec![],
                fingerprint: "".to_string(),
            },
            config: ModConfig {
                mod_type: Some("character".to_string()),
                ..Default::default()
            },
            enabled: true,
            added_at: Utc::now(),
        };

        db.games.insert(game_id.to_string(), GameConfig::default());
        db.profiles.insert(profile_id, profile);
        db.mods.insert(mod_id, record);

        let json = export_loadout_bundle(&db, game_id, profile_id).unwrap();
        assert!(json.contains("Abyss Raiden & Keqing"));
        assert!(json.contains("Raiden Shogun Boss Armor"));
        assert!(json.contains("54321"));

        let parsed = parse_loadout_bundle(&json).unwrap();
        assert_eq!(parsed.game_id, game_id);
        assert_eq!(parsed.profile_name, "Abyss Raiden & Keqing");
        assert_eq!(parsed.mods.len(), 1);
        assert_eq!(parsed.mods[0].name, "Raiden Shogun Boss Armor");
        assert_eq!(parsed.mods[0].gb_mod_id, Some(54321));
        assert!(parsed.mods[0].enabled);
    }

    #[test]
    fn test_export_clean_mod_zip_uncloaking_and_backup_stripping() {
        let dir = tempdir().unwrap();
        let mod_root = dir.path().join("KeqingMod");
        let backups_dir = mod_root.join(".yago_backups");
        std::fs::create_dir_all(&backups_dir).unwrap();

        // 1. Modified mod.ini (patched with keybinds)
        std::fs::write(mod_root.join("mod.ini"), "[KeyToggle]\nkey = k\n").unwrap();

        // 2. Clean original source .yago_source (should uncloak to mod.ini)
        std::fs::write(
            mod_root.join("mod.ini.yago_source"),
            "[KeyToggle]\nkey = f\n; clean source\n",
        )
        .unwrap();

        // 3. Backup file in .yago_backups (should be skipped)
        std::fs::write(backups_dir.join("backup.ini"), "backup").unwrap();

        // 4. Texture asset
        std::fs::write(mod_root.join("diffuse.dds"), b"DDS_TEXTURE_DATA").unwrap();

        let dest_zip = dir.path().join("export").join("KeqingMod_Clean.zip");
        let count = export_clean_mod_zip(&mod_root, &dest_zip).unwrap();

        assert_eq!(
            count, 2,
            "Should write exactly 2 files (mod.ini and diffuse.dds)"
        );

        // Read back zip to inspect
        let zip_file = std::fs::File::open(&dest_zip).unwrap();
        let mut archive = zip::ZipArchive::new(zip_file).unwrap();

        let mut file_names: Vec<String> = archive.file_names().map(|s| s.to_string()).collect();
        file_names.sort();
        assert_eq!(file_names, vec!["diffuse.dds", "mod.ini"]);

        // Check content of mod.ini inside zip
        let mut ini_file = archive.by_name("mod.ini").unwrap();
        let mut content = String::new();
        ini_file.read_to_string(&mut content).unwrap();
        assert!(
            content.contains("; clean source"),
            "Exported mod.ini must contain clean uncloaked source"
        );
    }
}
