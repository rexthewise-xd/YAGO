use crate::state::AppCore;
use resources::effects::{
    detect_duplicate_effects, install_or_update_effect, read_texfx_config, write_texfx_config,
    EffectLibraryStatus, SharedEffectLibrary, TexFxConfig,
};
use std::path::PathBuf;

impl AppCore {
    pub fn effect_dir(&self, assets_root: &std::path::Path, effect_id: &str) -> PathBuf {
        assets_root.join("effects").join(effect_id)
    }

    pub async fn list_effect_libraries(
        &self,
        game_id: String,
    ) -> Result<Vec<EffectLibraryStatus>, String> {
        let (enabled_effects, install_path, short_name, name) = {
            let dbs = self.game_dbs.lock().await;
            let db = dbs.get(&game_id).ok_or("Game database not found")?;
            let config = db
                .games
                .get(&game_id)
                .ok_or("Game configuration not found")?;
            (
                config.enabled_effects.clone(),
                config.install_path.clone(),
                config.short_name.clone(),
                config.name.clone(),
            )
        };

        let librarian = self.librarian.lock().await;
        let assets_root = librarian.assets_root.clone();
        let game_paths = librarian.game_paths(&game_id);
        drop(librarian);

        let mut dupe_warnings = Vec::new();
        for m_path in &[game_paths.mods, install_path.join("Mods")] {
            if m_path.exists() {
                for dupe in detect_duplicate_effects(m_path) {
                    let s = dupe.to_string_lossy().to_string();
                    if !dupe_warnings.contains(&s) {
                        dupe_warnings.push(s);
                    }
                }
            }
        }

        let mut statuses = Vec::new();
        for lib in SharedEffectLibrary::all() {
            if !lib.applies_to_game(&game_id)
                && !lib.applies_to_game(&short_name)
                && !lib.applies_to_game(&name)
            {
                continue;
            }

            let eff_id = lib.id();
            let dest_dir = self.effect_dir(&assets_root, eff_id);
            let version_file = dest_dir.join(".yago_version");
            let installed_version = if version_file.exists() {
                std::fs::read_to_string(version_file)
                    .ok()
                    .map(|s| s.trim().to_string())
            } else {
                None
            };

            let enabled = enabled_effects
                .iter()
                .any(|e| e.eq_ignore_ascii_case(eff_id));

            let texfx_config = if *lib == SharedEffectLibrary::TexFx && installed_version.is_some()
            {
                Some(read_texfx_config(&dest_dir))
            } else {
                None
            };

            let lib_ini = format!("{}.ini", lib.name().to_lowercase());
            let relevant_dupes: Vec<String> = dupe_warnings
                .iter()
                .filter(|p| p.to_lowercase().ends_with(&lib_ini))
                .cloned()
                .collect();

            statuses.push(EffectLibraryStatus {
                id: eff_id.to_string(),
                name: lib.name().to_string(),
                description: lib.description().to_string(),
                installed_version,
                enabled,
                duplicate_warnings: relevant_dupes,
                texfx_config,
            });
        }

        Ok(statuses)
    }

    pub async fn install_effect_library(
        &self,
        game_id: String,
        effect_id: String,
    ) -> Result<(), String> {
        let lib = SharedEffectLibrary::from_id(&effect_id)
            .ok_or_else(|| format!("Unknown effect library: {}", effect_id))?;

        let librarian = self.librarian.lock().await;
        let dest_dir = self.effect_dir(&librarian.assets_root, lib.id());
        drop(librarian);

        install_or_update_effect(lib, &dest_dir, |_, _| {})
            .await
            .map_err(|e| format!("Failed to install {}: {}", lib.name(), e))?;

        // Auto-enable after installation
        self.toggle_effect_library(game_id, effect_id, true).await?;

        Ok(())
    }

    pub async fn toggle_effect_library(
        &self,
        game_id: String,
        effect_id: String,
        enabled: bool,
    ) -> Result<(), String> {
        let eff_id_lower = effect_id.to_lowercase();
        let mut dbs = self.game_dbs.lock().await;
        let db = dbs.get_mut(&game_id).ok_or("Game database not found")?;
        let config = db
            .games
            .get_mut(&game_id)
            .ok_or("Game configuration not found")?;

        if enabled {
            if !config
                .enabled_effects
                .iter()
                .any(|e| e.eq_ignore_ascii_case(&eff_id_lower))
            {
                config.enabled_effects.push(eff_id_lower);
            }
        } else {
            config
                .enabled_effects
                .retain(|e| !e.eq_ignore_ascii_case(&eff_id_lower));
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

    pub async fn update_texfx_config(
        &self,
        _game_id: String,
        config: TexFxConfig,
    ) -> Result<(), String> {
        let librarian = self.librarian.lock().await;
        let dest_dir = self.effect_dir(&librarian.assets_root, "texfx");
        drop(librarian);

        write_texfx_config(&dest_dir, &config)
            .map_err(|e| format!("Failed to write TexFx config: {}", e))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::init::init_with_dir;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_effects_list_and_toggle() {
        let dir = tempdir().unwrap();
        let core = init_with_dir(dir.path().to_path_buf()).await.unwrap();

        let game_id = "genshin".to_string();
        let game_config = storage::models::GameConfig {
            id: game_id.clone(),
            name: "Genshin Impact".to_string(),
            short_name: "Genshin".to_string(),
            enabled_effects: vec![],
            ..Default::default()
        };
        {
            let mut dbs = core.game_dbs.lock().await;
            let mut db = storage::LibraryDatabase::default();
            db.games.insert(game_id.clone(), game_config);
            dbs.insert(game_id.clone(), db);
        }

        let effects = core.list_effect_libraries(game_id.clone()).await.unwrap();
        assert!(
            !effects.is_empty(),
            "Should list applicable effects for Genshin"
        );
        assert!(effects.iter().any(|e| e.id == "orfix"));
        assert!(effects.iter().any(|e| e.id == "texfx"));

        // Toggle effect on
        core.toggle_effect_library(game_id.clone(), "orfix".to_string(), true)
            .await
            .unwrap();
        let effects_after = core.list_effect_libraries(game_id.clone()).await.unwrap();
        let orfix = effects_after.iter().find(|e| e.id == "orfix").unwrap();
        assert!(orfix.enabled, "ORFix should be enabled");

        // Toggle effect off
        core.toggle_effect_library(game_id.clone(), "orfix".to_string(), false)
            .await
            .unwrap();
        let effects_off = core.list_effect_libraries(game_id.clone()).await.unwrap();
        let orfix_off = effects_off.iter().find(|e| e.id == "orfix").unwrap();
        assert!(!orfix_off.enabled, "ORFix should be disabled");
    }
}
