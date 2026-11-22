use std::path::{Path, PathBuf};

use storage::models::{ImportCandidate, ModRecord};
use uuid::Uuid;

use crate::commands::library::FileNode;
use crate::state::AppCore;

impl AppCore {
    pub async fn scan_mod_directory(
        &self,
        path: String,
        game_id: String,
    ) -> Result<Vec<ImportCandidate>, String> {
        let path_buf = PathBuf::from(path);
        let librarian = self.librarian.lock().await;
        storage::import::Importer::scan_directory(&path_buf, &librarian.assets_root, &game_id)
            .map_err(|e| e.to_string())
    }

    pub async fn import_mod_with_name(
        &self,
        game_id: String,
        path: String,
        custom_name: Option<String>,
    ) -> Result<ModRecord, String> {
        let path_buf = PathBuf::from(path);
        let record = {
            let librarian = self.librarian.lock().await;
            storage::import::Importer::import_mod_with_name(
                &librarian,
                path_buf,
                game_id.clone(),
                custom_name,
            )
            .await
            .map_err(|e| e.to_string())?
        };

        let new_db = self.librarian.lock().await.load_game_db(&game_id).await;
        if let Ok(new_db) = new_db {
            self.game_dbs.lock().await.insert(game_id.clone(), new_db);
            self.emit_library().await;
        }

        Ok(record)
    }

    pub async fn import_mod(&self, game_id: String, path: String) -> Result<ModRecord, String> {
        self.import_mod_with_name(game_id, path, None).await
    }

    pub async fn add_mod(&self, game_id: String, path: String) -> Result<ModRecord, String> {
        self.import_mod(game_id, path).await
    }

    pub async fn validate_mod(&self, mod_id: String) -> Result<bool, String> {
        let root = self.mod_root(&mod_id).await?;
        if !root.exists() {
            return Err("Mod directory does not exist".to_string());
        }

        let mut ini_found = 0;
        for entry in walkdir::WalkDir::new(&root)
            .max_depth(4)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            let is_ini = path.is_file()
                && (path
                    .extension()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_lowercase())
                    == Some("ini".to_string())
                    || path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .map(|s| s.to_lowercase().ends_with(".ini.yago_source"))
                        .unwrap_or(false));

            if is_ini {
                let file_name = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let bytes = std::fs::read(path)
                    .map_err(|e| format!("Failed to read {}: {}", file_name, e))?;
                let content = String::from_utf8_lossy(&bytes);
                let doc = match ini::parser::parse_ini(&content) {
                    Ok((_, doc)) => doc,
                    Err(e) => {
                        return Err(format!("Syntax error in {}: {}", file_name, e));
                    }
                };
                mod_patches::Validator::validate_logic(&doc)
                    .map_err(|e| format!("Logic error in {}: {}", file_name, e))?;
                ini_found += 1;
            }
        }

        if ini_found == 0 {
            return Err("No INI logic files found in mod directory".to_string());
        }

        Ok(true)
    }

    pub async fn delete_mod(&self, mod_id: String) -> Result<(), String> {
        let mod_uuid = Uuid::parse_str(&mod_id).map_err(|e| e.to_string())?;
        let mut dbs = self.game_dbs.lock().await;

        for (game_id, db) in dbs.iter_mut() {
            if let Some(record) = db.mods.get(&mod_uuid) {
                if record.path.exists() {
                    vfs::Safety::move_to_trash(&record.path).map_err(|e| {
                        format!("Failed to move mod '{}' to trash: {}", record.meta.name, e)
                    })?;
                }

                let _ = db.mods.remove(&mod_uuid);

                for profile in db.profiles.values_mut() {
                    profile.enabled_mod_ids.retain(|id| *id != mod_uuid);
                    profile.load_order.retain(|id| *id != mod_uuid);
                }

                self.librarian
                    .lock()
                    .await
                    .save_game_db(game_id, db)
                    .await
                    .map_err(|e| e.to_string())?;
                drop(dbs);
                self.emit_library().await;
                return Ok(());
            }
        }

        Err("Mod not found".to_string())
    }

    pub async fn toggle_mod(
        &self,
        game_id: String,
        mod_id: String,
        enabled: bool,
    ) -> Result<(), String> {
        let mod_uuid = Uuid::parse_str(&mod_id).map_err(|e| e.to_string())?;
        let mut dbs = self.game_dbs.lock().await;

        if let Some(db) = dbs.get_mut(&game_id) {
            if let Some(game_config) = db.games.get(&game_id) {
                let p_uuid =
                    Uuid::parse_str(&game_config.active_profile_id).map_err(|e| e.to_string())?;
                if let Some(profile) = db.profiles.get_mut(&p_uuid) {
                    if enabled {
                        if !profile.enabled_mod_ids.contains(&mod_uuid) {
                            profile.enabled_mod_ids.push(mod_uuid);
                        }
                    } else {
                        profile.enabled_mod_ids.retain(|id| *id != mod_uuid);
                    }

                    self.librarian
                        .lock()
                        .await
                        .save_game_db(&game_id, db)
                        .await
                        .map_err(|e| e.to_string())?;
                    drop(dbs);
                    self.emit_library().await;
                    return Ok(());
                }
            }
        }

        Err("Game or Profile not found".to_string())
    }

    pub async fn update_mod_tags(
        &self,
        game_id: String,
        mod_id: String,
        tags: Vec<String>,
    ) -> Result<(), String> {
        let mod_uuid = Uuid::parse_str(&mod_id).map_err(|e| e.to_string())?;
        let mut dbs = self.game_dbs.lock().await;

        if let Some(db) = dbs.get_mut(&game_id) {
            if let Some(record) = db.mods.get_mut(&mod_uuid) {
                record.config.tags = tags;
                self.librarian
                    .lock()
                    .await
                    .save_game_db(&game_id, db)
                    .await
                    .map_err(|e| e.to_string())?;
                drop(dbs);
                self.emit_library().await;
                return Ok(());
            }
        }

        Err("Mod not found".to_string())
    }

    async fn mod_root(&self, mod_id: &str) -> Result<PathBuf, String> {
        let mod_uuid = Uuid::parse_str(mod_id).map_err(|e| e.to_string())?;
        let dbs = self.game_dbs.lock().await;
        for db in dbs.values() {
            if let Some(record) = db.mods.get(&mod_uuid) {
                return Ok(record.path.clone());
            }
        }
        Err("Mod not found".to_string())
    }

    pub async fn get_mod_files(&self, mod_id: String) -> Result<Vec<FileNode>, String> {
        let root = self.mod_root(&mod_id).await?;
        if !root.exists() {
            return Ok(vec![]);
        }
        Ok(build_file_tree(&root, &root, 0))
    }

    pub async fn read_mod_file(&self, mod_id: String, file_path: String) -> Result<String, String> {
        let resolved = self.resolve_mod_file(&mod_id, &file_path).await?;
        std::fs::read_to_string(&resolved).map_err(|e| e.to_string())
    }

    pub async fn write_mod_file(
        &self,
        mod_id: String,
        file_path: String,
        content: String,
    ) -> Result<(), String> {
        let resolved = self.resolve_mod_file(&mod_id, &file_path).await?;
        std::fs::write(&resolved, content).map_err(|e| e.to_string())
    }

    async fn resolve_mod_file(&self, mod_id: &str, file_path: &str) -> Result<PathBuf, String> {
        let root = self.mod_root(mod_id).await?;
        let root = root.canonicalize().map_err(|e| e.to_string())?;
        let joined = root.join(file_path);
        let resolved = joined.canonicalize().map_err(|e| e.to_string())?;
        if !resolved.starts_with(&root) {
            return Err("Path escapes mod directory".to_string());
        }
        Ok(resolved)
    }

    fn load_fix_rules(&self, librarian_assets: &Path, game_id: &str) -> mod_patches::FixRuleSet {
        let fixers_dir = librarian_assets.join("fixers");

        // 1. Exact match e.g. "genshin.json"
        let exact_path = fixers_dir.join(format!("{}.json", game_id));
        if exact_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&exact_path) {
                if let Ok(rules) = serde_json::from_str::<mod_patches::FixRuleSet>(&content) {
                    return rules;
                }
            }
        }

        // 2. Normalized stem e.g. "GenshinImpact.exe" -> "genshin"
        let norm = game_id.to_lowercase().replace(".exe", "");
        for candidate in &["genshin", "zzz", "wuwa", "endfield", "starrail"] {
            if norm.contains(candidate) {
                let p = fixers_dir.join(format!("{}.json", candidate));
                if p.exists() {
                    if let Ok(content) = std::fs::read_to_string(&p) {
                        if let Ok(rules) = serde_json::from_str::<mod_patches::FixRuleSet>(&content)
                        {
                            return rules;
                        }
                    }
                }
            }
        }

        mod_patches::FixRuleSet {
            game_id: game_id.to_string(),
            ..Default::default()
        }
    }

    pub async fn preview_mod_fix(
        &self,
        game_id: String,
        mod_id: String,
    ) -> Result<mod_patches::FixPreview, String> {
        let root = self.mod_root(&mod_id).await?;
        let librarian = self.librarian.lock().await;
        let rules = self.load_fix_rules(&librarian.assets_root, &game_id);
        drop(librarian);

        mod_patches::preview_mod_fix(&root, &rules).map_err(|e| e.to_string())
    }

    pub async fn apply_mod_fix(
        &self,
        game_id: String,
        mod_id: String,
    ) -> Result<mod_patches::FixApplyReport, String> {
        let root = self.mod_root(&mod_id).await?;
        let librarian = self.librarian.lock().await;
        let rules = self.load_fix_rules(&librarian.assets_root, &game_id);
        drop(librarian);

        let report = mod_patches::apply_mod_fix(&root, &rules).map_err(|e| e.to_string())?;
        self.emit_library().await;
        Ok(report)
    }

    pub async fn rollback_mod_fix(&self, mod_id: String) -> Result<usize, String> {
        let root = self.mod_root(&mod_id).await?;
        let count = mod_patches::rollback_mod_fix(&root).map_err(|e| e.to_string())?;
        self.emit_library().await;
        Ok(count)
    }

    pub async fn has_mod_fix_backup(&self, mod_id: String) -> Result<bool, String> {
        let root = self.mod_root(&mod_id).await?;
        Ok(mod_patches::has_fix_backup(&root))
    }
}

fn build_file_tree(dir: &Path, root: &Path, depth: usize) -> Vec<FileNode> {
    if depth > 8 {
        return vec![];
    }
    let mut nodes = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return nodes;
    };
    let mut entries: Vec<_> = entries.filter_map(|e| e.ok()).collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();
        if path.is_dir() {
            nodes.push(FileNode {
                id: rel,
                name,
                kind: "folder".to_string(),
                size: None,
                children: Some(build_file_tree(&path, root, depth + 1)),
            });
        } else {
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            nodes.push(FileNode {
                id: rel,
                name,
                kind: "file".to_string(),
                size: Some(format_size(size)),
                children: None,
            });
        }
    }
    nodes
}

fn format_size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / 1024.0 / 1024.0)
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init::init_with_dir;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_delete_mod_moves_to_trash_and_updates_db() {
        let dir = tempdir().unwrap();
        let core = init_with_dir(dir.path().to_path_buf()).await.unwrap();

        let game_id = "genshin".to_string();
        let mod_uuid = Uuid::new_v4();
        let mod_dir = dir.path().join("test_mod");
        std::fs::create_dir_all(&mod_dir).unwrap();
        std::fs::write(mod_dir.join("mod.ini"), "test").unwrap();

        let record = ModRecord {
            id: mod_uuid,
            owner_game_id: game_id.clone(),
            path: mod_dir.clone(),
            size: "0".to_string(),
            meta: storage::models::ModMetadata {
                name: "Test Mod".to_string(),
                version: "1.0".to_string(),
                author: "Author".to_string(),
                url: None,
                preview_image: None,
                description: None,
            },
            compatibility: storage::models::ModCompatibility {
                game: "genshin".to_string(),
                character: "Keqing".to_string(),
                hashes: vec![],
                fingerprint: "".to_string(),
            },
            config: storage::models::ModConfig {
                mod_type: Some("character".to_string()),
                tags: vec![],
                keybinds: std::collections::HashMap::new(),
            },
            enabled: true,
            added_at: chrono::Utc::now(),
        };

        {
            let mut dbs = core.game_dbs.lock().await;
            let mut db = storage::LibraryDatabase::default();
            db.mods.insert(mod_uuid, record);
            dbs.insert(game_id.clone(), db);
        }

        let res = core.delete_mod(mod_uuid.to_string()).await;
        if res.is_ok() {
            let dbs = core.game_dbs.lock().await;
            assert!(!dbs.get(&game_id).unwrap().mods.contains_key(&mod_uuid));
            assert!(!mod_dir.exists(), "Mod directory should be moved to trash");
        } else {
            let dbs = core.game_dbs.lock().await;
            assert!(
                dbs.get(&game_id).unwrap().mods.contains_key(&mod_uuid),
                "Mod should stay in db if trash fails"
            );
        }
    }

    #[tokio::test]
    async fn test_mod_fix_lifecycle() {
        let dir = tempdir().unwrap();
        let core = init_with_dir(dir.path().to_path_buf()).await.unwrap();

        let game_id = "genshin".to_string();
        let mod_uuid = Uuid::new_v4();
        let mod_dir = dir.path().join("fix_test_mod");
        std::fs::create_dir_all(&mod_dir).unwrap();
        let original_ini = "[TextureOverrideBody]\nhash = deadbeef\nps-t0 = ResourceDiffuse\nps-t1 = ResourceNormalMap\n";
        std::fs::write(mod_dir.join("mod.ini"), original_ini).unwrap();

        let record = ModRecord {
            id: mod_uuid,
            owner_game_id: game_id.clone(),
            path: mod_dir.clone(),
            size: "0".to_string(),
            meta: storage::models::ModMetadata {
                name: "Fix Test".to_string(),
                version: "1.0".to_string(),
                author: "Author".to_string(),
                url: None,
                preview_image: None,
                description: None,
            },
            compatibility: storage::models::ModCompatibility {
                game: "genshin".to_string(),
                character: "Keqing".to_string(),
                hashes: vec![],
                fingerprint: "".to_string(),
            },
            config: storage::models::ModConfig {
                mod_type: Some("character".to_string()),
                tags: vec![],
                keybinds: std::collections::HashMap::new(),
            },
            enabled: true,
            added_at: chrono::Utc::now(),
        };

        {
            let mut dbs = core.game_dbs.lock().await;
            let mut db = storage::LibraryDatabase::default();
            db.mods.insert(mod_uuid, record);
            dbs.insert(game_id.clone(), db);
        }

        // Initially no backup
        assert!(!core.has_mod_fix_backup(mod_uuid.to_string()).await.unwrap());

        // Preview fix
        let preview = core
            .preview_mod_fix(game_id.clone(), mod_uuid.to_string())
            .await
            .unwrap();
        assert!(preview.total_orfix_fixes > 0);

        // Apply fix
        let report = core
            .apply_mod_fix(game_id.clone(), mod_uuid.to_string())
            .await
            .unwrap();
        assert_eq!(report.files_modified, 1);
        assert!(core.has_mod_fix_backup(mod_uuid.to_string()).await.unwrap());

        // Rollback fix
        let restored = core.rollback_mod_fix(mod_uuid.to_string()).await.unwrap();
        assert_eq!(restored, 1);
        assert!(!core.has_mod_fix_backup(mod_uuid.to_string()).await.unwrap());

        let restored_content = std::fs::read_to_string(mod_dir.join("mod.ini")).unwrap();
        assert_eq!(restored_content, original_ini);
    }
}
