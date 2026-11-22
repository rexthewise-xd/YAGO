use std::path::{Path, PathBuf};

use sync::SophonClient;

use crate::commands::library::{LoaderProgress, ProtonProgress, SetupStatus};
use crate::events::AppEvent;
use crate::state::AppCore;

impl AppCore {
    pub(crate) async fn base_storage(&self) -> PathBuf {
        let settings = self.global_settings.lock().await;
        if settings.yago_storage_path.as_os_str().is_empty() {
            self.app_data_dir.clone()
        } else {
            settings.yago_storage_path.clone()
        }
    }

    pub async fn check_setup(&self) -> Result<bool, String> {
        let settings_path = self.app_data_dir.join("settings.json");
        if !settings_path.exists() {
            return Ok(false);
        }

        let _ = self.pick_default_runner().await;

        let base_storage = self.base_storage().await;
        let common_loaders = base_storage.join("loaders").join("common");
        if !common_loaders.exists()
            || !common_loaders.join("d3d11.dll").exists()
            || !common_loaders.join("ReShade.dll").exists()
        {
            return Ok(false);
        }

        Ok(true)
    }

    async fn pick_default_runner(&self) -> Result<(), String> {
        {
            let settings = self.global_settings.lock().await;
            if settings
                .default_runner_id
                .as_ref()
                .is_some_and(|id| !id.is_empty())
            {
                return Ok(());
            }
        }

        let runners = self.list_runners().await.unwrap_or_default();
        if runners.is_empty() {
            return Ok(());
        }

        let preferred = runners
            .iter()
            .find(|r| r.contains("GE-Proton"))
            .or_else(|| runners.iter().find(|r| r.contains("cachyos")))
            .or_else(|| runners.iter().find(|r| r.contains("Proton")))
            .or_else(|| runners.first());

        if let Some(runner_id) = preferred {
            let mut settings = self.global_settings.lock().await;
            if settings
                .default_runner_id
                .as_ref()
                .is_none_or(|id| id.is_empty())
            {
                settings.default_runner_id = Some(runner_id.clone());
                let sm = self.settings_manager.clone();
                let settings_clone = settings.clone();
                tokio::spawn(async move {
                    let _ = sm.save(&settings_clone).await;
                });
            }
        }

        Ok(())
    }

    pub async fn get_setup_status(&self) -> Result<SetupStatus, String> {
        let base_storage = self.base_storage().await;

        let runners_dir = {
            let settings = self.global_settings.lock().await;
            if !settings.runners_path.as_os_str().is_empty() {
                settings.runners_path.clone()
            } else {
                base_storage.join("runners")
            }
        };
        let has_runners = runners_dir.exists()
            && std::fs::read_dir(runners_dir)
                .map(|e| e.count() > 0)
                .unwrap_or(false);

        let common_loaders = base_storage.join("loaders").join("common");
        let has_common_loaders = common_loaders.exists()
            && common_loaders.join("d3d11.dll").exists()
            && common_loaders.join("ReShade.dll").exists();

        let detected_steam = crate::commands::library::detect_steam_proton_path_internal()
            .await
            .unwrap_or(None);

        Ok(SetupStatus {
            has_runners,
            has_common_loaders,
            detected_steam_path: detected_steam,
        })
    }

    pub async fn fetch_manifest(&self, url: String) -> Result<sync::SophonManifest, String> {
        let client = SophonClient::new();
        client.fetch_manifest(&url).await.map_err(|e| e.to_string())
    }

    pub async fn download_game(
        &self,
        _game_id: String,
        _install_path: String,
    ) -> Result<(), String> {
        Ok(())
    }

    pub async fn install_common_libs(&self) -> Result<(), String> {
        let base_storage = self.base_storage().await;
        let common_path = base_storage.join("loaders").join("common");
        if !common_path.exists() {
            std::fs::create_dir_all(&common_path).map_err(|e| e.to_string())?;
        }

        self.install_xxmi_libs_internal(&common_path).await?;
        self.download_reshade_internal(&common_path).await?;

        self.emit(AppEvent::LoaderProgress(LoaderProgress {
            game_id: "common".to_string(),
            status: "Done".to_string(),
            progress: 1.0,
        }));

        Ok(())
    }

    async fn install_xxmi_libs_internal(&self, common_path: &Path) -> Result<(), String> {
        let repo_full = self.app_config.lock().await.common_loader_repo.clone();

        let parts: Vec<&str> = repo_full.split('/').collect();
        if parts.len() != 2 {
            return Err("Invalid common_loader_repo format".to_string());
        }
        let owner = parts[0];
        let repo = parts[1];

        let release = resources::github::get_latest_release(owner, repo)
            .await
            .map_err(|e| e.to_string())?;

        let asset = release
            .assets
            .iter()
            .find(|a| a.name.ends_with(".zip"))
            .ok_or_else(|| "No zip asset found in latest release".to_string())?;

        let temp_zip = self.app_data_dir.join("cache").join("temp_common.zip");
        if let Some(parent) = temp_zip.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let events = self.events.clone();
        resources::download_file(
            &asset.browser_download_url,
            &temp_zip,
            move |current, total| {
                let progress = (current as f64 / total as f64) * 0.5;
                events.emit(AppEvent::LoaderProgress(LoaderProgress {
                    game_id: "common".to_string(),
                    status: "Downloading Loaders...".to_string(),
                    progress,
                }));
            },
        )
        .await
        .map_err(|e| e.to_string())?;

        let extract_dir = common_path.join("_tmp_extract");
        if extract_dir.exists() {
            let _ = std::fs::remove_dir_all(&extract_dir);
        }
        std::fs::create_dir_all(&extract_dir).map_err(|e| e.to_string())?;

        vfs::extract_and_sanitize(&temp_zip, &extract_dir).map_err(|e| e.to_string())?;

        let walker = walkdir::WalkDir::new(&extract_dir);
        let mut files_found = 0;
        for entry in walker.into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_file() {
                let fname = entry.file_name().to_string_lossy().to_lowercase();
                if fname == "d3d11.dll"
                    || fname == "3dmloader.dll"
                    || fname == "dxgi.dll"
                    || fname == "d3dcompiler_47.dll"
                {
                    let dest = common_path.join(entry.file_name());
                    if let Err(e) = std::fs::copy(entry.path(), dest) {
                        eprintln!("Setup: Failed to copy {}: {}", fname, e);
                    } else {
                        files_found += 1;
                    }
                }
            }
        }

        if files_found == 0 {
            return Err("No required DLLs found in the downloaded archive".to_string());
        }

        let _ = std::fs::remove_dir_all(&extract_dir);
        let _ = std::fs::remove_file(&temp_zip);

        Ok(())
    }

    async fn download_reshade_internal(&self, common_path: &Path) -> Result<(), String> {
        let url = self.app_config.lock().await.reshade_url.clone();

        let events = self.events.clone();
        resources::reshade::download_reshade_dll(&url, common_path, move |current, total| {
            let progress = 0.5 + (current as f64 / total as f64) * 0.5;
            events.emit(AppEvent::LoaderProgress(LoaderProgress {
                game_id: "common".to_string(),
                status: "Downloading ReShade...".to_string(),
                progress,
            }));
        })
        .await
        .map_err(|e| e.to_string())?;

        Ok(())
    }
}
impl AppCore {
    pub async fn install_reshade(&self, game_id: String) -> Result<(), String> {
        let base_storage = self.base_storage().await;
        let common_path = base_storage.join("loaders").join("common");
        if !common_path.exists() {
            std::fs::create_dir_all(&common_path).map_err(|e| e.to_string())?;
        }

        self.download_reshade_internal(&common_path).await?;

        self.emit(AppEvent::LoaderProgress(LoaderProgress {
            game_id: game_id.clone(),
            status: "Done".to_string(),
            progress: 1.0,
        }));

        Ok(())
    }

    pub async fn download_loader(&self, game_id: String) -> Result<(), String> {
        let base_storage = self.base_storage().await;
        let path = base_storage.join("loaders").join(&game_id);
        let common_path = base_storage.join("loaders").join("common");

        let loader_repo = self
            .game_templates
            .lock()
            .await
            .get(&game_id)
            .and_then(|t| t.loader_repo.clone());

        if let Some(repo_full) = loader_repo {
            if !path.exists() {
                std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;

                let parts: Vec<&str> = repo_full.split('/').collect();
                if parts.len() == 2 {
                    let owner = parts[0];
                    let repo = parts[1];

                    if let Ok(release) = resources::github::get_latest_release(owner, repo).await {
                        if let Some(asset) =
                            release.assets.iter().find(|a| a.name.ends_with(".zip"))
                        {
                            let temp_zip = self
                                .app_data_dir
                                .join("cache")
                                .join(format!("temp_{}.zip", game_id));

                            let events = self.events.clone();
                            let gid_clone = game_id.clone();
                            let _ = resources::download_file(
                                &asset.browser_download_url,
                                &temp_zip,
                                move |current, total| {
                                    let progress = current as f64 / total as f64;
                                    events.emit(AppEvent::LoaderProgress(LoaderProgress {
                                        game_id: gid_clone.clone(),
                                        status: "Downloading...".to_string(),
                                        progress,
                                    }));
                                },
                            )
                            .await;

                            if temp_zip.exists() {
                                let _ = vfs::extract_and_sanitize(&temp_zip, &path);
                                let _ = std::fs::remove_file(temp_zip);
                            }
                        }
                    }
                }
            }
        } else if !path.exists() {
            std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
        }

        if common_path.exists() {
            for file in [
                "d3d11.dll",
                "3dmloader.dll",
                "d3dcompiler_47.dll",
                "nvapi64.dll",
            ] {
                let src = common_path.join(file);
                if src.exists() {
                    let dest = path.join(file);
                    if !dest.exists() {
                        let _ = std::fs::copy(src, dest);
                    }
                }
            }
        }

        self.emit(AppEvent::LoaderProgress(LoaderProgress {
            game_id: game_id.clone(),
            status: "Done".to_string(),
            progress: 1.0,
        }));

        Ok(())
    }

    pub async fn ensure_game_resources(&self, game_id: String) -> Result<(), String> {
        let base_storage = self.base_storage().await;
        let common_path = base_storage.join("loaders").join("common");
        let common_exists = common_path.exists() && common_path.join("d3d11.dll").exists();

        if !common_exists {
            self.install_common_libs().await?;
        }

        self.download_loader(game_id).await?;

        Ok(())
    }

    pub async fn download_proton(&self) -> Result<(), String> {
        let base_storage = self.base_storage().await;
        let repo_full = self.app_config.lock().await.proton_repo.clone();

        let parts: Vec<&str> = repo_full.split('/').collect();
        if parts.len() != 2 {
            return Err("Invalid proton_repo format".to_string());
        }
        let owner = parts[0];
        let repo = parts[1];

        let runners_dir = {
            let settings = self.global_settings.lock().await;
            if !settings.runners_path.as_os_str().is_empty() {
                settings.runners_path.clone()
            } else {
                base_storage.join("runners")
            }
        };
        if !runners_dir.exists() {
            std::fs::create_dir_all(&runners_dir).map_err(|e| e.to_string())?;
        }

        let release: resources::github::GithubRelease =
            resources::github::get_latest_release(owner, repo)
                .await
                .map_err(|e| e.to_string())?;

        let asset = release
            .assets
            .iter()
            .find(|a| a.name.ends_with(".tar.gz"))
            .ok_or_else(|| "No tar.gz asset found in latest release".to_string())?;

        let temp_tar = self.app_data_dir.join("cache").join("temp_proton.tar.gz");
        if let Some(parent) = temp_tar.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let events = self.events.clone();
        resources::download_file(
            &asset.browser_download_url,
            &temp_tar,
            move |current, total| {
                let progress = current as f64 / total as f64;
                events.emit(AppEvent::ProtonProgress(ProtonProgress {
                    version: "Latest".to_string(),
                    status: "Downloading...".to_string(),
                    progress,
                }));
            },
        )
        .await
        .map_err(|e| e.to_string())?;

        vfs::extract_targz(&temp_tar, &runners_dir).map_err(|e| e.to_string())?;

        let _ = std::fs::remove_file(&temp_tar);

        self.emit(AppEvent::ProtonProgress(ProtonProgress {
            version: "Latest".to_string(),
            status: "Done".to_string(),
            progress: 1.0,
        }));

        Ok(())
    }
}
