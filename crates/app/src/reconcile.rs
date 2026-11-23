use std::collections::HashMap;
use std::path::PathBuf;
use storage::models::InstallStatus;
use uuid::Uuid;

use crate::events::AppEvent;
use crate::state::AppCore;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default, PartialEq, Eq)]
pub struct ReconcileReport {
    pub reset_downloads: Vec<String>,
    pub missing_games: Vec<String>,
    pub orphaned_mods: HashMap<String, Vec<Uuid>>,
    pub cleaned_session_locks: Vec<String>,
}

impl ReconcileReport {
    pub fn has_issues(&self) -> bool {
        !self.reset_downloads.is_empty()
            || !self.missing_games.is_empty()
            || !self.orphaned_mods.is_empty()
            || !self.cleaned_session_locks.is_empty()
    }
}

/// Reconciles library states, resetting stuck downloads, detecting missing executables,
/// discovering orphaned mod folders, and cleaning up crashed Proxy session locks.
pub async fn reconcile_library(core: &AppCore) -> ReconcileReport {
    let mut report = ReconcileReport::default();
    let mut dbs = core.game_dbs.lock().await;

    let active_downloads: std::collections::HashSet<String> = {
        let controls = core.download_controls.lock().await;
        controls.keys().cloned().collect()
    };

    let mut games_to_save = Vec::new();

    for (game_id, db) in dbs.iter_mut() {
        if let Some(config) = db.games.get_mut(game_id) {
            // 1. Stuck Download Recovery: reset unmonitored Downloading / Updating states to Queued
            if (config.install_status == InstallStatus::Downloading
                || config.install_status == InstallStatus::Updating)
                && !active_downloads.contains(game_id)
            {
                config.install_status = InstallStatus::Queued;
                report.reset_downloads.push(game_id.clone());
                games_to_save.push(game_id.clone());
            }

            // 2. Missing Game Check
            if config.install_status == InstallStatus::Installed && !config.exe_path.exists() {
                report.missing_games.push(game_id.clone());
            }

            // 3. Dirty Proxy Exit Recovery: detect .yago_session_lock when process is not running
            let lock_path = config.install_path.join(".yago_session_lock");
            let is_running = launcher::Monitor::is_running(&config.exe_name);
            if lock_path.exists() && !is_running {
                if !config.unsafe_mode {
                    let _ = loader::LoaderContext::uninstall_loader(
                        &config.install_path,
                        config.prefix_path.as_deref(),
                        false,
                        config.unsafe_mode,
                    )
                    .await;
                }
                let _ = std::fs::remove_file(&lock_path);
                report.cleaned_session_locks.push(game_id.clone());
            }
        }

        // 4. Orphaned Mod Check: check if mod directories were externally deleted
        let mut missing_mod_ids = Vec::new();
        for (mod_id, record) in &db.mods {
            if !record.path.exists() {
                missing_mod_ids.push(*mod_id);
            }
        }
        if !missing_mod_ids.is_empty() {
            report
                .orphaned_mods
                .insert(game_id.clone(), missing_mod_ids);
        }
    }

    // Save DBs where install_status was reset
    for gid in games_to_save {
        if let Some(db) = dbs.get(&gid) {
            let _ = core.librarian.lock().await.save_game_db(&gid, db).await;
        }
    }

    drop(dbs);

    report
}

impl AppCore {
    /// Triggers an immediate library reconciliation pass.
    pub async fn reconcile_now(&self) -> Result<ReconcileReport, String> {
        let report = reconcile_library(self).await;
        if report.has_issues() {
            self.emit_library().await;
        }
        Ok(report)
    }

    /// Relocates a game's install and executable paths after manual moves on disk.
    pub async fn relocate_game(&self, game_id: String, new_path: String) -> Result<(), String> {
        let target = PathBuf::from(&new_path);
        if !target.exists() {
            return Err(format!("Specified path does not exist: {new_path}"));
        }

        let mut dbs = self.game_dbs.lock().await;
        let db = dbs.get_mut(&game_id).ok_or("Game not found in database")?;
        let (install_dir, exe_file) = if target.is_file() {
            let parent = target
                .parent()
                .ok_or("Invalid file path parent")?
                .to_path_buf();
            (parent, target)
        } else {
            let exe_name = db
                .games
                .get(&game_id)
                .ok_or("Game configuration missing")?
                .exe_name
                .clone();

            let direct_exe = target.join(&exe_name);
            if direct_exe.exists() {
                (target.clone(), direct_exe)
            } else {
                let mut found_exe = None;
                if let Ok(entries) = std::fs::read_dir(&target) {
                    for entry in entries.filter_map(|e| e.ok()) {
                        let p = entry.path();
                        if p.is_file()
                            && p.extension()
                                .and_then(|s| s.to_str())
                                .map(|s| s.to_lowercase())
                                == Some("exe".to_string())
                        {
                            found_exe = Some(p);
                            break;
                        }
                    }
                }
                let exe = found_exe.ok_or_else(|| {
                    format!("Could not locate {} in folder {:?}", exe_name, target)
                })?;
                (target, exe)
            }
        };

        let saved_exe = {
            let config = db
                .games
                .get_mut(&game_id)
                .ok_or("Game configuration missing")?;
            config.install_path = install_dir;
            config.exe_path = exe_file.clone();
            if let Some(name) = exe_file.file_name() {
                config.exe_name = name.to_string_lossy().to_string();
            }
            config.exe_path.clone()
        };

        self.librarian
            .lock()
            .await
            .save_game_db(&game_id, db)
            .await
            .map_err(|e| e.to_string())?;

        drop(dbs);
        self.emit_library().await;
        self.emit(AppEvent::TaskCompleted(format!(
            "Relocated game to {:?}",
            saved_exe
        )));

        Ok(())
    }

    /// Prunes orphaned mod database records whose directories no longer exist on disk.
    pub async fn prune_orphaned_mods(&self, game_id: String) -> Result<usize, String> {
        let mut dbs = self.game_dbs.lock().await;
        let db = dbs.get_mut(&game_id).ok_or("Game not found")?;

        let mut to_remove = Vec::new();
        for (id, record) in &db.mods {
            if !record.path.exists() {
                to_remove.push(*id);
            }
        }

        if to_remove.is_empty() {
            return Ok(0);
        }

        let pruned_count = to_remove.len();

        for id in &to_remove {
            db.mods.remove(id);
        }

        for profile in db.profiles.values_mut() {
            profile.enabled_mod_ids.retain(|id| !to_remove.contains(id));
            profile.load_order.retain(|id| !to_remove.contains(id));
        }

        self.librarian
            .lock()
            .await
            .save_game_db(&game_id, db)
            .await
            .map_err(|e| e.to_string())?;

        drop(dbs);
        self.emit_library().await;

        Ok(pruned_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init::init_with_dir;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_stuck_download_resets_to_queued() {
        let dir = tempdir().unwrap();
        let core = init_with_dir(dir.path().to_path_buf()).await.unwrap();

        let game_id = "test_game".to_string();
        let mut db = storage::LibraryDatabase::default();
        let config = storage::models::GameConfig {
            id: game_id.clone(),
            install_status: InstallStatus::Downloading,
            ..Default::default()
        };
        db.games.insert(game_id.clone(), config);

        {
            let mut dbs = core.game_dbs.lock().await;
            dbs.insert(game_id.clone(), db);
        }

        let report = reconcile_library(&core).await;
        assert_eq!(report.reset_downloads, vec![game_id.clone()]);

        {
            let dbs = core.game_dbs.lock().await;
            let db = dbs.get(&game_id).unwrap();
            let game = db.games.get(&game_id).unwrap();
            assert_eq!(game.install_status, InstallStatus::Queued);
        }
    }

    #[tokio::test]
    async fn test_missing_game_exe_reported() {
        let dir = tempdir().unwrap();
        let core = init_with_dir(dir.path().to_path_buf()).await.unwrap();

        let game_id = "genshin".to_string();
        let mut db = storage::LibraryDatabase::default();
        let config = storage::models::GameConfig {
            id: game_id.clone(),
            exe_path: dir.path().join("non_existent.exe"),
            install_status: InstallStatus::Installed,
            ..Default::default()
        };
        db.games.insert(game_id.clone(), config);

        {
            let mut dbs = core.game_dbs.lock().await;
            dbs.insert(game_id.clone(), db);
        }

        let report = reconcile_library(&core).await;
        assert_eq!(report.missing_games, vec![game_id]);
    }

    #[tokio::test]
    async fn test_orphaned_mods_detected_and_pruned() {
        let dir = tempdir().unwrap();
        let core = init_with_dir(dir.path().to_path_buf()).await.unwrap();

        let game_id = "genshin".to_string();
        let mod_id = Uuid::new_v4();
        let mut db = storage::LibraryDatabase::default();

        let record = storage::models::ModRecord {
            id: mod_id,
            owner_game_id: game_id.clone(),
            path: dir.path().join("deleted_mod_folder"),
            size: "0".to_string(),
            meta: storage::models::ModMetadata {
                name: "Deleted Mod".to_string(),
                version: "1.0".to_string(),
                author: "Author".to_string(),
                url: None,
                preview_image: None,
                description: None,
                gb_mod_id: None,
                gb_date_updated: None,
            },
            compatibility: storage::models::ModCompatibility {
                game: game_id.clone(),
                character: "Raiden".to_string(),
                hashes: vec![],
                fingerprint: "".to_string(),
            },
            config: storage::models::ModConfig::default(),
            enabled: true,
            added_at: chrono::Utc::now(),
        };

        db.mods.insert(mod_id, record);

        let p_id = Uuid::new_v4();
        let profile = storage::models::Profile {
            id: p_id,
            name: "Default".to_string(),
            enabled_mod_ids: vec![mod_id],
            load_order: vec![mod_id],
            ..Default::default()
        };
        db.profiles.insert(p_id, profile);
        db.games
            .insert(game_id.clone(), storage::models::GameConfig::default());

        {
            let mut dbs = core.game_dbs.lock().await;
            dbs.insert(game_id.clone(), db);
        }

        // Reconcile detects orphaned mod
        let report = reconcile_library(&core).await;
        assert_eq!(report.orphaned_mods.get(&game_id), Some(&vec![mod_id]));

        // Pruning removes it from mods and profile
        let pruned = core.prune_orphaned_mods(game_id.clone()).await.unwrap();
        assert_eq!(pruned, 1);

        {
            let dbs = core.game_dbs.lock().await;
            let db = dbs.get(&game_id).unwrap();
            assert!(!db.mods.contains_key(&mod_id));
            let prof = db.profiles.get(&p_id).unwrap();
            assert!(!prof.enabled_mod_ids.contains(&mod_id));
            assert!(!prof.load_order.contains(&mod_id));
        }
    }

    #[tokio::test]
    async fn test_session_lock_cleaned_on_reconcile() {
        let dir = tempdir().unwrap();
        let core = init_with_dir(dir.path().to_path_buf()).await.unwrap();

        let game_id = "genshin".to_string();
        let game_dir = dir.path().join("GenshinGame");
        std::fs::create_dir_all(&game_dir).unwrap();

        let lock_file = game_dir.join(".yago_session_lock");
        std::fs::write(&lock_file, "crashed_session").unwrap();
        assert!(lock_file.exists());

        let mut db = storage::LibraryDatabase::default();
        let config = storage::models::GameConfig {
            id: game_id.clone(),
            install_path: game_dir.clone(),
            exe_name: "GenshinImpact_fake_test_123.exe".to_string(),
            unsafe_mode: true, // Don't call actual DLL removal in test
            ..Default::default()
        };
        db.games.insert(game_id.clone(), config);

        {
            let mut dbs = core.game_dbs.lock().await;
            dbs.insert(game_id.clone(), db);
        }

        let report = reconcile_library(&core).await;
        assert_eq!(report.cleaned_session_locks, vec![game_id]);
        assert!(!lock_file.exists(), "Session lock file must be removed");
    }
}
