use storage::models::InstallStatus;
use sync::orchestrator::ProgressDetailed;
use sync::{ChunkOrchestrator, OrchestratorEvent, SophonClient};
use tokio::sync::mpsc;

use crate::events::AppEvent;
use crate::state::AppCore;

impl AppCore {
    pub async fn start_game_download(
        &self,
        game_id: String,
        selected_category_ids: Vec<String>,
    ) -> Result<(), String> {
        let (install_path, template) = {
            let templates = self.game_templates.lock().await;
            let dbs = self.game_dbs.lock().await;

            let db = dbs
                .get(&game_id)
                .ok_or_else(|| format!("Game {} not found", game_id))?;
            let config = db
                .games
                .get(&game_id)
                .ok_or_else(|| format!("Config for {} missing", game_id))?;

            if config.install_status == InstallStatus::Downloading {
                return Err("Download already in progress".to_string());
            }

            let install_path = config.install_path.clone();
            if install_path.as_os_str().is_empty() {
                return Err("Installation path not set".to_string());
            }

            let template = templates.get(&game_id).ok_or("Template not found")?.clone();
            (install_path, template)
        };

        let client = SophonClient::new();
        let build = client
            .get_build(
                &template.sophon_branch,
                &template.sophon_package_id,
                &template.sophon_password,
                &template.sophon_plat_app,
                &template.sophon_game_biz,
                &template.sophon_launcher_id,
                &template.sophon_channel_id,
                &template.sophon_sub_channel_id,
            )
            .await
            .map_err(|e| e.to_string())?;

        let mut selected_manifests = Vec::new();
        for manifest_meta in build.manifests {
            if manifest_meta.matching_field == "game"
                || selected_category_ids.contains(&manifest_meta.category_id)
            {
                let url = format!("{}/{}", manifest_meta.url_prefix, manifest_meta.manifest_id);
                let manifest = client
                    .fetch_manifest(&url)
                    .await
                    .map_err(|e| e.to_string())?;
                selected_manifests.push(manifest);
            }
        }

        if selected_manifests.is_empty() {
            return Err("No manifests selected for download".to_string());
        }

        let (tx_pause, rx_pause) = tokio::sync::watch::channel(false);
        self.download_controls
            .lock()
            .await
            .insert(game_id.clone(), tx_pause);

        {
            let mut dbs = self.game_dbs.lock().await;
            if let Some(db) = dbs.get_mut(&game_id) {
                if let Some(config) = db.games.get_mut(&game_id) {
                    config.install_status = InstallStatus::Downloading;
                    config.installed_components = selected_category_ids.clone();
                    let _ = self.librarian.lock().await.save_game_db(&game_id, db).await;
                }
            }
        }
        self.emit_library().await;

        let selected_manifests_clone = selected_manifests.clone();

        let orchestrator = ChunkOrchestrator::new(
            game_id.clone(),
            client,
            selected_manifests,
            install_path,
            build.chunk_base_url,
            8,
        );

        let core = self.clone();
        let game_id_clone = game_id.clone();

        tokio::spawn(async move {
            let (tx_events, mut rx_events) = mpsc::channel(100);
            let orchestrator_handle =
                tokio::spawn(async move { orchestrator.run(tx_events, rx_pause).await });

            let mut last_emit = std::time::Instant::now();
            while let Some(event) = rx_events.recv().await {
                match event {
                    OrchestratorEvent::Progress(progress) => {
                        if last_emit.elapsed() >= std::time::Duration::from_millis(100) {
                            core.emit(AppEvent::DownloadProgress(progress));
                            last_emit = std::time::Instant::now();
                        }
                    }
                    OrchestratorEvent::Error { chunk_id, error } => {
                        eprintln!("Download Error for {}: {}", chunk_id, error);
                        core.emit(AppEvent::DownloadError(format!("{}: {}", chunk_id, error)));
                    }
                    OrchestratorEvent::Completed => {
                        println!("Download completed for {}", game_id_clone);

                        let install_path = {
                            let dbs = core.game_dbs.lock().await;
                            dbs.get(&game_id_clone)
                                .and_then(|db| db.games.get(&game_id_clone))
                                .map(|c| c.install_path.clone())
                        };
                        if let Some(install_path) = install_path {
                            if let Err(e) = sync::verifier::Verifier::commit_version(
                                &install_path,
                                &build.version,
                                &selected_manifests_clone,
                            ) {
                                eprintln!("Sophon: Failed to commit version to disk: {}", e);
                            }
                        }

                        let _ = core
                            .update_game_status(
                                &game_id_clone,
                                InstallStatus::Installed,
                                Some(build.version.clone()),
                            )
                            .await;
                        let _ = core.sync_game_assets(game_id_clone.clone()).await;
                        core.emit(AppEvent::DownloadComplete(game_id_clone.clone()));
                    }
                    _ => {}
                }
            }

            let _ = orchestrator_handle.await;
        });

        Ok(())
    }

    pub async fn pause_game_download(&self, game_id: String) -> Result<(), String> {
        let controls = self.download_controls.lock().await;
        if let Some(tx) = controls.get(&game_id) {
            let _ = tx.send(true);
            drop(controls);
            self.update_game_status(&game_id, InstallStatus::Queued, None)
                .await?;
            Ok(())
        } else {
            Err("No active download found for this game".to_string())
        }
    }

    pub async fn resume_game_download(&self, game_id: String) -> Result<(), String> {
        let controls = self.download_controls.lock().await;
        if let Some(tx) = controls.get(&game_id) {
            let _ = tx.send(false);
            drop(controls);
            self.update_game_status(&game_id, InstallStatus::Downloading, None)
                .await?;
            Ok(())
        } else {
            drop(controls);
            let components = {
                let dbs = self.game_dbs.lock().await;
                let db = dbs.get(&game_id).ok_or("Game not found")?;
                let config = db.games.get(&game_id).ok_or("Config missing")?;
                config.installed_components.clone()
            };
            self.start_game_download(game_id, components).await
        }
    }

    pub async fn update_game_status(
        &self,
        game_id: &str,
        status: InstallStatus,
        version: Option<String>,
    ) -> Result<(), String> {
        let mut dbs = self.game_dbs.lock().await;
        if let Some(db) = dbs.get_mut(game_id) {
            if let Some(config) = db.games.get_mut(game_id) {
                config.install_status = status;
                if let Some(v) = version {
                    config.version = v;
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
        Err("Game not found in library".to_string())
    }
}
impl AppCore {
    pub async fn repair_game(&self, game_id: String, deep: Option<bool>) -> Result<(), String> {
        let deep_verification = deep.unwrap_or(true);

        let (install_path, selected_categories, template) = {
            let templates = self.game_templates.lock().await;
            let dbs = self.game_dbs.lock().await;

            let db = dbs.get(&game_id).ok_or("Game not found")?;
            let config = db.games.get(&game_id).ok_or("Config missing")?;

            let install_path = config.install_path.clone();
            let selected_categories = config.installed_components.clone();

            let template = templates
                .get(&game_id)
                .or_else(|| {
                    let base = game_id.trim_end_matches(".exe");
                    templates.get(base).or_else(|| {
                        templates.values().find(|t| {
                            let name_low = t.name.to_lowercase();
                            let short_low = t.short_name.to_lowercase();
                            base.contains(&short_low)
                                || base.contains(&name_low)
                                || short_low.contains(base)
                                || name_low.contains(base)
                        })
                    })
                })
                .ok_or_else(|| format!("Template not found for game ID: {}", game_id))?
                .clone();

            (install_path, selected_categories, template)
        };

        let client = SophonClient::new();
        let build = client
            .get_build(
                &template.sophon_branch,
                &template.sophon_package_id,
                &template.sophon_password,
                &template.sophon_plat_app,
                &template.sophon_game_biz,
                &template.sophon_launcher_id,
                &template.sophon_channel_id,
                &template.sophon_sub_channel_id,
            )
            .await
            .map_err(|e| format!("Sophon getBuild failed: {}", e))?;

        let mut manifests = Vec::new();
        for m_meta in build.manifests {
            let is_core = m_meta.matching_field == "game";
            let is_selected = selected_categories.contains(&m_meta.category_id);
            let is_english = m_meta.matching_field == "en-us";

            if is_core || is_selected || (selected_categories.is_empty() && is_english) {
                let url = format!("{}/{}", m_meta.url_prefix, m_meta.manifest_id);
                let manifest = client
                    .fetch_manifest(&url)
                    .await
                    .map_err(|e| e.to_string())?;
                manifests.push(manifest);
            }
        }

        if manifests.is_empty() {
            return Err(
                "No manifests identified for repair. Ensure game components are valid.".to_string(),
            );
        }

        let (tx_pause, rx_pause) = tokio::sync::watch::channel(false);
        self.download_controls
            .lock()
            .await
            .insert(game_id.clone(), tx_pause);

        self.update_game_status(&game_id, InstallStatus::Updating, None)
            .await?;
        self.emit(AppEvent::DownloadProgress(ProgressDetailed {
            game_id: game_id.clone(),
            percentage: 0.0,
            speed_bps: 0,
            eta_secs: 0,
            downloaded_bytes: 0,
            total_bytes: 100,
        }));

        let manifests_clone = manifests.clone();

        let orchestrator = ChunkOrchestrator::new(
            game_id.clone(),
            client,
            manifests,
            install_path,
            build.chunk_base_url,
            8,
        );

        let core = self.clone();
        let game_id_clone = game_id.clone();
        let target_version_clone = build.version.clone();

        tokio::spawn(async move {
            let (tx_events, mut rx_events) = mpsc::channel(100);
            let orchestrator_handle = tokio::spawn(async move {
                orchestrator
                    .verify_and_repair(tx_events, rx_pause, deep_verification)
                    .await
            });

            let mut last_emit = std::time::Instant::now();
            while let Some(event) = rx_events.recv().await {
                match event {
                    OrchestratorEvent::Progress(progress) => {
                        if last_emit.elapsed() >= std::time::Duration::from_millis(100) {
                            core.emit(AppEvent::DownloadProgress(progress));
                            last_emit = std::time::Instant::now();
                        }
                    }
                    OrchestratorEvent::Error { chunk_id, error } => {
                        eprintln!("Sophon Repair Error [{}]: {}", chunk_id, error);
                        core.emit(AppEvent::DownloadError(format!("{}: {}", chunk_id, error)));
                    }
                    OrchestratorEvent::Completed => {
                        println!("Sophon: Repair completed for {}", game_id_clone);

                        let install_path = {
                            let dbs = core.game_dbs.lock().await;
                            dbs.get(&game_id_clone)
                                .and_then(|db| db.games.get(&game_id_clone))
                                .map(|c| c.install_path.clone())
                        };
                        if let Some(install_path) = install_path {
                            if let Err(e) = sync::verifier::Verifier::commit_version(
                                &install_path,
                                &target_version_clone,
                                &manifests_clone,
                            ) {
                                eprintln!("Sophon: Failed to commit version to disk: {}", e);
                            }
                        }

                        let _ = core
                            .update_game_status(
                                &game_id_clone,
                                InstallStatus::Installed,
                                Some(target_version_clone.clone()),
                            )
                            .await;
                        let _ = core.sync_game_assets(game_id_clone.clone()).await;
                        core.emit(AppEvent::DownloadComplete(game_id_clone.clone()));
                    }
                    _ => {}
                }
            }
            let _ = orchestrator_handle.await;
        });

        Ok(())
    }
}
