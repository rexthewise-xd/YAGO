use storage::models::Profile;
use uuid::Uuid;

use crate::commands::library::ProfileUpdate;
use crate::state::AppCore;

impl AppCore {
    pub async fn switch_profile(&self, game_id: String, profile_id: String) -> Result<(), String> {
        let mut dbs = self.game_dbs.lock().await;
        if let Some(db) = dbs.get_mut(&game_id) {
            if let Some(config) = db.games.get_mut(&game_id) {
                let p_uuid = Uuid::parse_str(&profile_id).map_err(|e| e.to_string())?;
                if db.profiles.contains_key(&p_uuid) {
                    config.active_profile_id = profile_id;
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
        Err("Profile or Game not found".to_string())
    }

    pub async fn create_profile(&self, game_id: String, name: String) -> Result<Profile, String> {
        let profile = self
            .librarian
            .lock()
            .await
            .create_profile(&game_id, name)
            .await
            .map_err(|e| e.to_string())?;
        let mut dbs = self.game_dbs.lock().await;
        if let Some(db) = dbs.get_mut(&game_id) {
            db.profiles.insert(profile.id, profile.clone());
        }
        drop(dbs);
        self.emit_library().await;
        Ok(profile)
    }

    pub async fn duplicate_profile(
        &self,
        game_id: String,
        profile_id: Uuid,
        name: String,
    ) -> Result<Profile, String> {
        let profile = self
            .librarian
            .lock()
            .await
            .duplicate_profile(&game_id, profile_id, name)
            .await
            .map_err(|e| e.to_string())?;
        let mut dbs = self.game_dbs.lock().await;
        if let Some(db) = dbs.get_mut(&game_id) {
            db.profiles.insert(profile.id, profile.clone());
        }
        drop(dbs);
        self.emit_library().await;
        Ok(profile)
    }

    pub async fn update_profile(
        &self,
        game_id: String,
        profile_id: String,
        update: ProfileUpdate,
    ) -> Result<(), String> {
        let mut dbs = self.game_dbs.lock().await;
        if let Some(db) = dbs.get_mut(&game_id) {
            let p_uuid = Uuid::parse_str(&profile_id).map_err(|e| e.to_string())?;
            if let Some(profile) = db.profiles.get_mut(&p_uuid) {
                if let Some(n) = update.name {
                    profile.name = n;
                }
                if let Some(d) = update.description {
                    profile.description = d;
                }
                if let Some(gs) = update.use_gamescope {
                    profile.use_gamescope = gs;
                }
                if let Some(gm) = update.use_gamemode {
                    profile.use_gamemode = gm;
                }
                if let Some(mh) = update.use_mangohud {
                    profile.use_mangohud = mh;
                }
                if let Some(rs) = update.use_reshade {
                    profile.use_reshade = rs;
                }
                if let Some(res) = update.resolution {
                    profile.resolution = Some(res);
                }
                if let Some(args) = update.launch_args {
                    profile.launch_args = args;
                }
                if let Some(ga) = update.gamescope_args {
                    profile.gamescope_args = ga;
                }
                if let Some(path) = update.save_data_path {
                    profile.save_data_path = Some(path);
                }
                if let Some(mods) = update.enabled_mod_ids {
                    profile.enabled_mod_ids = mods;
                }
                if let Some(order) = update.load_order {
                    profile.load_order = order;
                }
                if let Some(pw) = update.proton_wayland {
                    profile.proton_wayland = pw;
                }
                if let Some(ll) = update.dxvk_low_latency {
                    profile.dxvk_low_latency = ll;
                }
                if let Some(vk) = update.obs_vkcapture {
                    profile.obs_vkcapture = vk;
                }
                if let Some(ls) = update.lsfg_vk {
                    profile.lsfg_vk = ls;
                }
                if let Some(sfw) = update.enable_sfw_toggle {
                    profile.enable_sfw_toggle = sfw;
                }
                if let Some(key) = update.sfw_toggle_key {
                    profile.sfw_toggle_key = key;
                }
                if let Some(shuf) = update.shuffle_on_launch {
                    profile.shuffle_on_launch = shuf;
                }
                if let Some(ctx) = update.enable_context_swaps {
                    profile.enable_context_swaps = ctx;
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
        Err("Game or Profile not found".to_string())
    }

    pub async fn delete_profile(&self, game_id: String, profile_id: String) -> Result<(), String> {
        let mut dbs = self.game_dbs.lock().await;
        if let Some(db) = dbs.get_mut(&game_id) {
            let p_uuid = Uuid::parse_str(&profile_id).map_err(|e| e.to_string())?;
            if db.profiles.len() <= 1 {
                return Err("Cannot delete the last remaining profile.".to_string());
            }
            if let Some(config) = db.games.get(&game_id) {
                if config.active_profile_id == profile_id {
                    return Err("Cannot delete active profile".into());
                }
            }
            if db.profiles.remove(&p_uuid).is_some() {
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
        Err("Game or Profile not found".to_string())
    }

    pub async fn rename_profile(
        &self,
        game_id: String,
        profile_id: String,
        new_name: String,
    ) -> Result<(), String> {
        let mut dbs = self.game_dbs.lock().await;
        if let Some(db) = dbs.get_mut(&game_id) {
            let p_uuid = Uuid::parse_str(&profile_id).map_err(|e| e.to_string())?;
            if let Some(profile) = db.profiles.get_mut(&p_uuid) {
                profile.name = new_name;
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
        Err("Game or Profile not found".to_string())
    }

    pub async fn set_load_order(&self, game_id: String, order: Vec<Uuid>) -> Result<(), String> {
        let mut dbs = self.game_dbs.lock().await;
        if let Some(db) = dbs.get_mut(&game_id) {
            if let Some(config) = db.games.get_mut(&game_id) {
                let p_uuid =
                    Uuid::parse_str(&config.active_profile_id).map_err(|e| e.to_string())?;
                if let Some(profile) = db.profiles.get_mut(&p_uuid) {
                    profile.load_order = order;
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
        Err("Profile or Game not found".to_string())
    }

    pub async fn export_profile_bundle(
        &self,
        game_id: String,
        profile_id: String,
    ) -> Result<String, String> {
        let p_uuid = Uuid::parse_str(&profile_id).map_err(|e| e.to_string())?;
        let dbs = self.game_dbs.lock().await;
        let db = dbs.get(&game_id).ok_or("Game not found")?;
        storage::export_loadout_bundle(db, &game_id, p_uuid).map_err(|e| e.to_string())
    }

    pub async fn import_profile_bundle(
        &self,
        game_id: String,
        json: String,
    ) -> Result<BundleImportReport, String> {
        let bundle = storage::parse_loadout_bundle(&json).map_err(|e| e.to_string())?;

        let mut dbs = self.game_dbs.lock().await;
        let db = dbs.get_mut(&game_id).ok_or("Game not found")?;

        let new_pid = Uuid::new_v4();
        let mut enabled_ids = Vec::new();
        let mut load_order = Vec::new();
        let mut matched_count = 0usize;
        let mut missing_mods = Vec::new();

        // Sort bundle mods by load_order_index
        let mut sorted_mods = bundle.mods.clone();
        sorted_mods.sort_by_key(|m| m.load_order_index);

        for b_mod in sorted_mods {
            // Find local mod by gb_mod_id or normalized name
            let found = db.mods.values().find(|local| {
                if let (Some(b_gb), Some(l_gb)) = (b_mod.gb_mod_id, local.meta.gb_mod_id) {
                    if b_gb == l_gb {
                        return true;
                    }
                }
                local
                    .meta
                    .name
                    .trim()
                    .eq_ignore_ascii_case(b_mod.name.trim())
            });

            if let Some(local_mod) = found {
                let mid = local_mod.id;
                load_order.push(mid);
                if b_mod.enabled {
                    enabled_ids.push(mid);
                }
                matched_count += 1;
            } else {
                missing_mods.push(b_mod.name.clone());
            }
        }

        let new_profile = Profile {
            id: new_pid,
            name: bundle.profile_name,
            description: bundle.description,
            enabled_mod_ids: enabled_ids,
            load_order,
            added_at: chrono::Utc::now(),
            ..Default::default()
        };

        db.profiles.insert(new_pid, new_profile);
        self.librarian
            .lock()
            .await
            .save_game_db(&game_id, db)
            .await
            .map_err(|e| e.to_string())?;

        drop(dbs);
        self.emit_library().await;

        Ok(BundleImportReport {
            profile_id: new_pid.to_string(),
            matched_count,
            missing_mods,
        })
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct BundleImportReport {
    pub profile_id: String,
    pub matched_count: usize,
    pub missing_mods: Vec<String>,
}
