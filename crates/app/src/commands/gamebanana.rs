use crate::state::AppCore;
use resources::gamebanana::{self, GbModDetail, GbModSummary};
use storage::models::ModRecord;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct ModUpdateInfo {
    pub mod_uuid: String,
    pub gb_mod_id: u64,
    pub name: String,
    pub local_updated: u64,
    pub remote_updated: u64,
}

impl AppCore {
    fn resolve_gb_game_id(&self, game_id: &str) -> Result<u64, String> {
        // 1. Check template.gamebanana_id first
        if let Ok(templates) = self.game_templates.try_lock() {
            if let Some(t) = templates
                .get(game_id)
                .or_else(|| templates.get(game_id.trim_end_matches(".exe")))
            {
                if let Some(id) = t.gamebanana_id {
                    return Ok(id);
                }
            }
        }

        // 2. Hardcoded fallback mapping
        if let Some(id) = gamebanana::game_id_to_gb_id(game_id) {
            return Ok(id);
        }

        // 3. Try looking up template short_name or name
        if let Ok(templates) = self.game_templates.try_lock() {
            if let Some(t) = templates
                .get(game_id)
                .or_else(|| templates.get(game_id.trim_end_matches(".exe")))
            {
                if let Some(id) = gamebanana::game_id_to_gb_id(&t.name)
                    .or_else(|| gamebanana::game_id_to_gb_id(&t.short_name))
                {
                    return Ok(id);
                }
            }
        }

        Err(format!(
            "No GameBanana game ID mapping found for '{}'",
            game_id
        ))
    }

    pub async fn browse_gamebanana(
        &self,
        game_id: String,
        query: String,
        page: u32,
        include_nsfw: bool,
    ) -> Result<Vec<GbModSummary>, String> {
        let gb_id = self.resolve_gb_game_id(&game_id)?;
        gamebanana::search_mods(gb_id, &query, page, include_nsfw)
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn get_gamebanana_mod_detail(&self, mod_id: u64) -> Result<GbModDetail, String> {
        gamebanana::get_mod_detail(mod_id)
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn install_from_gamebanana(
        &self,
        game_id: String,
        mod_id: u64,
        file_id: Option<u64>,
    ) -> Result<ModRecord, String> {
        let detail = gamebanana::get_mod_detail(mod_id)
            .await
            .map_err(|e| e.to_string())?;

        let target_file = if let Some(fid) = file_id {
            detail.files.iter().find(|f| f.id == fid)
        } else {
            detail
                .files
                .iter()
                .find(|f| {
                    let fn_lower = f.filename.to_lowercase();
                    fn_lower.ends_with(".zip")
                        || fn_lower.ends_with(".7z")
                        || fn_lower.ends_with(".rar")
                })
                .or_else(|| detail.files.first())
        }
        .ok_or_else(|| "No downloadable files found for this mod on GameBanana".to_string())?;

        let librarian = self.librarian.lock().await;
        let cache_dir = librarian.assets_root.join("cache");
        let _ = std::fs::create_dir_all(&cache_dir);
        let safe_filename = target_file.filename.replace(['/', '\\'], "_");
        let cache_file = cache_dir.join(format!("gb_{}_{}", mod_id, safe_filename));

        println!(
            "Downloading GameBanana mod {} (file {}): {}",
            mod_id, target_file.id, target_file.download_url
        );

        resources::download_file(&target_file.download_url, &cache_file, |_, _| {})
            .await
            .map_err(|e| format!("Download failed: {}", e))?;

        let mut record = storage::Importer::import_mod_with_name(
            &librarian,
            cache_file,
            game_id.clone(),
            Some(detail.summary.name.clone()),
        )
        .await
        .map_err(|e| format!("Import failed: {}", e))?;

        drop(librarian);

        // Stamp GameBanana metadata
        record.meta.author = detail.summary.author.clone();
        record.meta.url = Some(detail.summary.profile_url.clone());
        record.meta.gb_mod_id = Some(mod_id);
        record.meta.gb_date_updated = Some(detail.summary.date_updated);
        if record.meta.description.is_none() || record.meta.description.as_deref() == Some("") {
            record.meta.description = Some(detail.description.clone());
        }

        // Save in DB
        let mut dbs = self.game_dbs.lock().await;
        if let Some(db) = dbs.get_mut(&game_id) {
            db.mods.insert(record.id, record.clone());
            self.librarian
                .lock()
                .await
                .save_game_db(&game_id, db)
                .await
                .map_err(|e| e.to_string())?;
        }
        drop(dbs);

        self.emit_library().await;
        Ok(record)
    }

    pub async fn check_mod_updates(&self, game_id: String) -> Result<Vec<ModUpdateInfo>, String> {
        let candidates: Vec<(Uuid, u64, String, u64)> = {
            let dbs = self.game_dbs.lock().await;
            let Some(db) = dbs.get(&game_id) else {
                return Ok(Vec::new());
            };
            db.mods
                .values()
                .filter_map(|m| {
                    m.meta.gb_mod_id.map(|gb_id| {
                        (
                            m.id,
                            gb_id,
                            m.meta.name.clone(),
                            m.meta.gb_date_updated.unwrap_or(0),
                        )
                    })
                })
                .collect()
        };

        let mut updates = Vec::new();
        for (uuid, gb_id, name, local_updated) in candidates {
            if let Ok(detail) = gamebanana::get_mod_detail(gb_id).await {
                if detail.summary.date_updated > local_updated {
                    updates.push(ModUpdateInfo {
                        mod_uuid: uuid.to_string(),
                        gb_mod_id: gb_id,
                        name,
                        local_updated,
                        remote_updated: detail.summary.date_updated,
                    });
                }
            }
        }

        Ok(updates)
    }

    pub async fn update_mod_from_gamebanana(
        &self,
        game_id: String,
        mod_uuid: String,
    ) -> Result<ModRecord, String> {
        let uuid = Uuid::parse_str(&mod_uuid).map_err(|e| e.to_string())?;

        // 1. Read existing mod record and preserve attributes
        let (gb_id, name, tags, presets, active_preset, anchors, suppress, enabled, old_path) = {
            let dbs = self.game_dbs.lock().await;
            let db = dbs.get(&game_id).ok_or("Game database not found")?;
            let record = db.mods.get(&uuid).ok_or("Mod record not found")?;
            let gb_id = record
                .meta
                .gb_mod_id
                .ok_or("Mod is not associated with GameBanana")?;
            (
                gb_id,
                record.meta.name.clone(),
                record.config.tags.clone(),
                record.config.variant_presets.clone(),
                record.config.active_preset_id.clone(),
                record.config.ini_anchors.clone(),
                record.config.suppress_creditinfo,
                record.enabled,
                record.path.clone(),
            )
        };

        // 2. Fetch latest detail from GameBanana
        let detail = gamebanana::get_mod_detail(gb_id)
            .await
            .map_err(|e| e.to_string())?;

        let target_file = detail
            .files
            .iter()
            .find(|f| {
                let fn_lower = f.filename.to_lowercase();
                fn_lower.ends_with(".zip")
                    || fn_lower.ends_with(".7z")
                    || fn_lower.ends_with(".rar")
            })
            .or_else(|| detail.files.first())
            .ok_or_else(|| "No downloadable files found for mod update".to_string())?;

        // 3. Download update archive
        let librarian = self.librarian.lock().await;
        let cache_dir = librarian.assets_root.join("cache");
        let _ = std::fs::create_dir_all(&cache_dir);
        let safe_filename = target_file.filename.replace(['/', '\\'], "_");
        let cache_file = cache_dir.join(format!("gb_{}_{}", gb_id, safe_filename));

        resources::download_file(&target_file.download_url, &cache_file, |_, _| {})
            .await
            .map_err(|e| format!("Download failed: {}", e))?;

        // 4. Move old mod files to trash non-destructively
        if old_path.exists() {
            let _ = vfs::Safety::move_to_trash(&old_path);
        }

        // 5. Import new mod archive preserving name
        let mut new_record = storage::Importer::import_mod_with_name(
            &librarian,
            cache_file,
            game_id.clone(),
            Some(name),
        )
        .await
        .map_err(|e| format!("Import failed: {}", e))?;

        drop(librarian);

        // 6. Restore preserved attributes
        new_record.config.tags = tags;
        new_record.config.variant_presets = presets;
        new_record.config.active_preset_id = active_preset;
        new_record.config.ini_anchors = anchors;
        new_record.config.suppress_creditinfo = suppress;
        new_record.enabled = enabled;
        new_record.meta.gb_mod_id = Some(gb_id);
        new_record.meta.gb_date_updated = Some(detail.summary.date_updated);
        new_record.meta.author = detail.summary.author;
        new_record.meta.url = Some(detail.summary.profile_url);

        // 7. Update database: substitute UUID in profiles so active profile stays intact
        let new_uuid = new_record.id;
        let mut dbs = self.game_dbs.lock().await;
        if let Some(db) = dbs.get_mut(&game_id) {
            db.mods.remove(&uuid);
            db.mods.insert(new_uuid, new_record.clone());

            for profile in db.profiles.values_mut() {
                for id in &mut profile.enabled_mod_ids {
                    if *id == uuid {
                        *id = new_uuid;
                    }
                }
                for id in &mut profile.load_order {
                    if *id == uuid {
                        *id = new_uuid;
                    }
                }
            }

            self.librarian
                .lock()
                .await
                .save_game_db(&game_id, db)
                .await
                .map_err(|e| e.to_string())?;
        }
        drop(dbs);

        self.emit_library().await;
        Ok(new_record)
    }

    pub async fn handle_deep_link(
        &self,
        game_id: String,
        uri: String,
    ) -> Result<ModRecord, String> {
        let link = gamebanana::parse_yago_uri(&uri).map_err(|e| e.to_string())?;
        self.install_from_gamebanana(game_id, link.mod_id, link.file_id)
            .await
    }
}
