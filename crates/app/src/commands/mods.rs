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

    pub async fn list_mod_presets(
        &self,
        game_id: String,
        mod_id: String,
    ) -> Result<Vec<storage::models::VariantPreset>, String> {
        let root = self.mod_root(&mod_id).await?;
        let default_preset = mod_patches::extract_default_preset(&root);

        let dbs = self.game_dbs.lock().await;
        let db = dbs.get(&game_id).ok_or("Game not found")?;
        let mod_uuid = Uuid::parse_str(&mod_id).map_err(|e| e.to_string())?;
        let record = db.mods.get(&mod_uuid).ok_or("Mod not found")?;

        let mut list = vec![default_preset];
        for p in &record.config.variant_presets {
            if !p.is_default {
                list.push(p.clone());
            }
        }
        Ok(list)
    }

    pub async fn save_mod_preset(
        &self,
        game_id: String,
        mod_id: String,
        name: String,
        variables: Option<std::collections::HashMap<String, String>>,
    ) -> Result<storage::models::VariantPreset, String> {
        let root = self.mod_root(&mod_id).await?;
        let default_preset = mod_patches::extract_default_preset(&root);

        let vars = if let Some(v) = variables {
            v
        } else {
            let librarian = self.librarian.lock().await;
            let paths = librarian.game_paths(&game_id);
            let candidates = [
                paths.profile_mods.join("00_Configs").join("d3dx_user.ini"),
                paths.profile_mods.join("d3dx_user.ini"),
                paths.mods.join("d3dx_user.ini"),
            ];
            drop(librarian);

            let d3dx_path = candidates.into_iter().find(|p| p.exists());
            let known_vars: Vec<String> = default_preset.variables.keys().cloned().collect();

            let captured = if let Some(path) = d3dx_path {
                mod_patches::capture_from_d3dx_user(&path, &mod_id, &known_vars).unwrap_or_default()
            } else {
                std::collections::HashMap::new()
            };

            if captured.is_empty() {
                default_preset.variables
            } else {
                captured
            }
        };

        let new_preset = storage::models::VariantPreset {
            id: Uuid::new_v4().to_string(),
            name,
            is_default: false,
            variables: vars,
        };

        let mod_uuid = Uuid::parse_str(&mod_id).map_err(|e| e.to_string())?;
        let mut dbs = self.game_dbs.lock().await;
        let db = dbs.get_mut(&game_id).ok_or("Game not found")?;
        let record = db.mods.get_mut(&mod_uuid).ok_or("Mod not found")?;

        record.config.variant_presets.push(new_preset.clone());

        self.librarian
            .lock()
            .await
            .save_game_db(&game_id, db)
            .await
            .map_err(|e| e.to_string())?;

        drop(dbs);
        self.emit_library().await;
        Ok(new_preset)
    }

    pub async fn activate_mod_preset(
        &self,
        game_id: String,
        mod_id: String,
        preset_id: Option<String>,
    ) -> Result<(), String> {
        let mod_uuid = Uuid::parse_str(&mod_id).map_err(|e| e.to_string())?;
        let mut dbs = self.game_dbs.lock().await;
        let db = dbs.get_mut(&game_id).ok_or("Game not found")?;
        let record = db.mods.get_mut(&mod_uuid).ok_or("Mod not found")?;

        record.config.active_preset_id = preset_id;

        self.librarian
            .lock()
            .await
            .save_game_db(&game_id, db)
            .await
            .map_err(|e| e.to_string())?;

        drop(dbs);
        self.emit_library().await;
        Ok(())
    }

    pub async fn delete_mod_preset(
        &self,
        game_id: String,
        mod_id: String,
        preset_id: String,
    ) -> Result<(), String> {
        if preset_id == "default" {
            return Err("Cannot delete the factory default preset".to_string());
        }

        let mod_uuid = Uuid::parse_str(&mod_id).map_err(|e| e.to_string())?;
        let mut dbs = self.game_dbs.lock().await;
        let db = dbs.get_mut(&game_id).ok_or("Game not found")?;
        let record = db.mods.get_mut(&mod_uuid).ok_or("Mod not found")?;

        let is_default = record
            .config
            .variant_presets
            .iter()
            .find(|p| p.id == preset_id)
            .map(|p| p.is_default)
            .unwrap_or(false);

        if is_default {
            return Err("Cannot delete the factory default preset".to_string());
        }

        record.config.variant_presets.retain(|p| p.id != preset_id);
        if record.config.active_preset_id.as_deref() == Some(&preset_id) {
            record.config.active_preset_id = None;
        }

        self.librarian
            .lock()
            .await
            .save_game_db(&game_id, db)
            .await
            .map_err(|e| e.to_string())?;

        drop(dbs);
        self.emit_library().await;
        Ok(())
    }

    pub async fn update_mod_keybind(
        &self,
        game_id: String,
        mod_id: String,
        label: String,
        new_key: String,
    ) -> Result<(), String> {
        let mod_uuid = Uuid::parse_str(&mod_id).map_err(|e| e.to_string())?;
        let root = self.mod_root(&mod_id).await?;

        // 1. Update in-memory database and find target variable
        let mut target_variable = None;
        {
            let mut dbs = self.game_dbs.lock().await;
            let db = dbs.get_mut(&game_id).ok_or("Game not found")?;
            let record = db.mods.get_mut(&mod_uuid).ok_or("Mod not found")?;

            let mut found = false;
            for (kname, kb) in &mut record.config.keybinds {
                if kb.label == label || kname == &label || kb.variable == label {
                    kb.key = new_key.clone();
                    target_variable = Some(kb.variable.clone());
                    found = true;
                    break;
                }
            }

            if !found {
                let var_name = format!("${}", label.trim_start_matches('$'));
                record.config.keybinds.insert(
                    label.clone(),
                    storage::models::Keybind {
                        label: label.clone(),
                        key: new_key.clone(),
                        variable: var_name.clone(),
                        values: vec![],
                        condition: None,
                    },
                );
                target_variable = Some(var_name);
            }

            self.librarian
                .lock()
                .await
                .save_game_db(&game_id, db)
                .await
                .map_err(|e| e.to_string())?;
        }

        // 2. Rewrite key in .ini / .ini.yago_source
        let target_var_clean = target_variable
            .as_deref()
            .unwrap_or(&label)
            .trim_start_matches('$')
            .to_lowercase();

        for entry in walkdir::WalkDir::new(&root)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let name_lower = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();

            if name_lower.ends_with(".ini") || name_lower.ends_with(".ini.yago_source") {
                if let Ok(content) = std::fs::read_to_string(path) {
                    if let Ok((_, mut doc)) = ini::parser::parse_ini(&content) {
                        let mut file_changed = false;
                        for section in &mut doc.sections {
                            let sec_lower = section.name.to_lowercase();
                            if sec_lower.contains("key") {
                                let controls_var = section.items.iter().any(|item| {
                                    if let ini::ast::IniItem::Pair { key, .. } = item {
                                        let k_clean =
                                            key.trim().trim_start_matches('$').to_lowercase();
                                        k_clean == target_var_clean
                                    } else {
                                        false
                                    }
                                }) || sec_lower.contains(&target_var_clean);

                                if controls_var {
                                    for item in &mut section.items {
                                        if let ini::ast::IniItem::Pair { key, value } = item {
                                            if key.trim().eq_ignore_ascii_case("key") {
                                                *value = new_key.clone();
                                                file_changed = true;
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        if file_changed {
                            let compiled = ini::IniCompiler::default().serialize(&doc);
                            let _ = std::fs::write(path, compiled);
                        }
                    }
                }
            }
        }

        self.emit_library().await;
        Ok(())
    }

    pub async fn update_mod_anchors(
        &self,
        game_id: String,
        mod_id: String,
        anchors: Vec<storage::models::IniAnchor>,
        suppress_creditinfo: bool,
    ) -> Result<(), String> {
        let mod_uuid = Uuid::parse_str(&mod_id).map_err(|e| e.to_string())?;
        let mut dbs = self.game_dbs.lock().await;
        let db = dbs.get_mut(&game_id).ok_or("Game not found")?;
        let record = db.mods.get_mut(&mod_uuid).ok_or("Mod not found")?;

        record.config.ini_anchors = anchors;
        record.config.suppress_creditinfo = suppress_creditinfo;

        self.librarian
            .lock()
            .await
            .save_game_db(&game_id, db)
            .await
            .map_err(|e| e.to_string())?;

        drop(dbs);
        self.emit_library().await;
        Ok(())
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
                gb_mod_id: None,
                gb_date_updated: None,
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
                ..Default::default()
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
                gb_mod_id: None,
                gb_date_updated: None,
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
                ..Default::default()
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

    #[tokio::test]
    async fn test_update_mod_keybind_rewrites_ini() {
        let dir = tempdir().unwrap();
        let core = init_with_dir(dir.path().to_path_buf()).await.unwrap();

        let game_id = "genshin".to_string();
        let mod_uuid = Uuid::new_v4();
        let mod_dir = dir.path().join("keybind_test_mod");
        std::fs::create_dir_all(&mod_dir).unwrap();

        let ini_content = "[KeyToggleHat]\nkey = h\ntype = toggle\n$hat = 0, 1\n";
        std::fs::write(mod_dir.join("mod.ini.yago_source"), ini_content).unwrap();

        let mut keybinds = std::collections::HashMap::new();
        keybinds.insert(
            "hat".to_string(),
            storage::models::Keybind {
                label: "hat".to_string(),
                key: "h".to_string(),
                variable: "$hat".to_string(),
                values: vec!["0".to_string(), "1".to_string()],
                condition: None,
            },
        );

        let record = ModRecord {
            id: mod_uuid,
            owner_game_id: game_id.clone(),
            path: mod_dir.clone(),
            size: "0".to_string(),
            meta: storage::models::ModMetadata {
                name: "Keybind Test".to_string(),
                version: "1.0".to_string(),
                author: "Author".to_string(),
                url: None,
                preview_image: None,
                description: None,
                gb_mod_id: None,
                gb_date_updated: None,
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
                keybinds,
                ..Default::default()
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

        // Update keybind
        core.update_mod_keybind(
            game_id.clone(),
            mod_uuid.to_string(),
            "hat".to_string(),
            "k".to_string(),
        )
        .await
        .unwrap();

        // 1. Verify in-memory DB updated
        {
            let dbs = core.game_dbs.lock().await;
            let db = dbs.get(&game_id).unwrap();
            let rec = db.mods.get(&mod_uuid).unwrap();
            assert_eq!(rec.config.keybinds.get("hat").unwrap().key, "k");
        }

        // 2. Verify file on disk updated
        let on_disk = std::fs::read_to_string(mod_dir.join("mod.ini.yago_source")).unwrap();
        assert!(
            on_disk.contains("key = k"),
            "INI file must contain 'key = k', got: {}",
            on_disk
        );
    }
}
