use std::collections::HashMap;
use std::path::PathBuf;

use launcher::InjectionMethod;
use storage::catalog::RemoteCatalogEntry;
use storage::scanner::DiscoveredGame;
use storage::{CatalogManager, Discovery, FpsConfig, LibraryDatabase};
use sync::SophonClient;
use uuid::Uuid;
use vfs::{ExeInspector, Safety};

use crate::state::AppCore;

#[derive(serde::Serialize, Clone, PartialEq)]
pub struct IdentifiedGame {
    pub id: String,
    pub name: String,
    pub short_name: String,
    pub developer: String,
    pub description: String,
    pub version: String,
    pub size: String,
    pub color: String,
    pub accent_color: String,
    pub cover_image: String,
    pub icon: String,
    pub logo_initial: String,
    pub install_path: String,
    pub exe_name: String,
    pub supported_injection_methods: Vec<InjectionMethod>,
    pub injection_method: InjectionMethod,
    pub modloader_enabled: bool,
}

#[derive(serde::Serialize, Clone, Debug, PartialEq)]
pub struct FileNode {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub size: Option<String>,
    pub children: Option<Vec<FileNode>>,
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct LoaderProgress {
    pub game_id: String,
    pub status: String,
    pub progress: f64,
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct ProtonProgress {
    pub version: String,
    pub status: String,
    pub progress: f64,
}

#[derive(serde::Serialize, Clone, PartialEq)]
pub struct SetupStatus {
    pub has_runners: bool,
    pub has_common_loaders: bool,
    pub detected_steam_path: Option<String>,
}

#[derive(serde::Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct GameConfigUpdate {
    pub name: Option<String>,
    pub cover_image: Option<String>,
    pub icon: Option<String>,
    pub developer: Option<String>,
    pub description: Option<String>,
    pub install_path: Option<String>,
    pub exe_name: Option<String>,
    pub launch_args: Option<Vec<String>>,
    pub gamescope_args: Option<Vec<String>>,
    pub fps_config: Option<FpsConfig>,
    pub short_name: Option<String>,
    pub color: Option<String>,
    pub accent_color: Option<String>,
    pub logo_initial: Option<String>,
    pub injection_method: Option<InjectionMethod>,
    pub modloader_enabled: Option<bool>,
    pub auto_update: Option<bool>,
    pub active_profile_id: Option<String>,
    pub active_runner_id: Option<Option<String>>,
    pub prefix_path: Option<Option<String>>,
    pub enable_linux_shield: Option<bool>,
    pub start_method: Option<storage::models::StartMethod>,
    pub unsafe_mode: Option<bool>,
    pub umu_game_id: Option<Option<String>>,
    pub use_umu: Option<bool>,
    pub pre_launch_cmd: Option<Option<String>>,
    pub post_exit_cmd: Option<Option<String>>,
    pub use_rabbit_wrapper: Option<bool>,
    pub hunting_mode: Option<bool>,
    pub gui_collect_ini: Option<Option<PathBuf>>,
}

#[derive(serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProfileUpdate {
    pub name: Option<String>,
    pub description: Option<String>,
    pub use_gamescope: Option<bool>,
    pub use_gamemode: Option<bool>,
    pub use_mangohud: Option<bool>,
    pub use_reshade: Option<bool>,
    pub resolution: Option<(u32, u32)>,
    pub launch_args: Option<Vec<String>>,
    pub gamescope_args: Option<Vec<String>>,
    pub save_data_path: Option<PathBuf>,
    pub enabled_mod_ids: Option<Vec<Uuid>>,
    pub load_order: Option<Vec<Uuid>>,
    pub proton_wayland: Option<bool>,
    pub dxvk_low_latency: Option<bool>,
    pub obs_vkcapture: Option<bool>,
    pub lsfg_vk: Option<bool>,
    pub enable_sfw_toggle: Option<bool>,
    pub sfw_toggle_key: Option<Option<String>>,
    pub shuffle_on_launch: Option<bool>,
    pub enable_context_swaps: Option<bool>,
}

pub(crate) fn map_injection(m: storage::InjectionMethod) -> InjectionMethod {
    match m {
        storage::InjectionMethod::None => InjectionMethod::None,
        storage::InjectionMethod::Proxy => InjectionMethod::Proxy,
        storage::InjectionMethod::Loader => InjectionMethod::Loader,
        storage::InjectionMethod::RemoteThread => InjectionMethod::RemoteThread,
        storage::InjectionMethod::ManualMap => InjectionMethod::ManualMap,
    }
}

pub(crate) fn map_start_method(m: storage::StartMethod) -> launcher::StartMethod {
    match m {
        storage::StartMethod::Native => launcher::StartMethod::Native,
        storage::StartMethod::Shell => launcher::StartMethod::Shell,
        storage::StartMethod::Steam => launcher::StartMethod::Steam,
        storage::StartMethod::Manual => launcher::StartMethod::Manual,
    }
}

impl AppCore {
    pub async fn get_remote_catalog(&self) -> Result<Vec<RemoteCatalogEntry>, String> {
        let templates = self.game_templates.lock().await;
        let dbs = self.game_dbs.lock().await;
        let installed_ids: Vec<String> = dbs.keys().cloned().collect();

        let mut catalog = CatalogManager::get_remote_catalog(&templates, &installed_ids)
            .await
            .map_err(|e| e.to_string())?;

        let librarian = self.librarian.lock().await;
        for entry in &mut catalog {
            entry.template.cover_image =
                librarian.resolve_template_asset(&entry.template.cover_image);
            entry.template.icon = librarian.resolve_template_asset(&entry.template.icon);
        }

        Ok(catalog)
    }

    pub async fn initialize_remote_game(&self, template_id: String) -> Result<String, String> {
        let game_id = template_id.clone();

        let (exists, template) = {
            let templates = self.game_templates.lock().await;
            let t = templates
                .get(&template_id)
                .ok_or_else(|| format!("Template {} not found", template_id))?
                .clone();
            let dbs = self.game_dbs.lock().await;
            (dbs.contains_key(&game_id), t)
        };

        if exists {
            return Ok(game_id);
        }

        let default_profile = storage::models::Profile::default();
        let p_id = default_profile.id;

        let librarian = self.librarian.lock().await;

        let game_dir = librarian.games_root.join(&game_id);
        if !game_dir.exists() {
            std::fs::create_dir_all(&game_dir).map_err(|e| e.to_string())?;
        }

        let config = storage::models::GameConfig {
            id: game_id.clone(),
            name: template.name.clone(),
            short_name: template.short_name.clone(),
            developer: template.developer.clone(),
            description: template.description.clone(),
            install_path: PathBuf::new(),
            exe_path: PathBuf::new(),
            exe_name: template.executables.first().cloned().unwrap_or_default(),
            version: "Pending".to_string(),
            remote_version: None,
            installed_components: vec![],
            size: "Unknown".to_string(),
            color: template.color.clone(),
            accent_color: template.accent_color.clone(),
            cover_image: librarian.resolve_template_asset(&template.cover_image),
            icon: librarian.resolve_template_asset(&template.icon),
            logo_initial: template.logo_initial.clone(),
            enabled: true,
            added_at: chrono::Utc::now(),
            launch_args: template.launch_args.clone().unwrap_or_default(),
            gamescope_args: vec![],
            active_profile_id: p_id.to_string(),
            fps_config: template.fps_config.clone(),
            injection_method: if cfg!(target_os = "windows") {
                template.injection_method_windows
            } else {
                template.injection_method_linux
            }
            .unwrap_or(storage::models::InjectionMethod::None),
            install_status: storage::models::InstallStatus::Remote,
            auto_update: template.auto_update.unwrap_or(true),
            active_runner_id: None,
            prefix_path: None,
            modloader_enabled: template.modloader_enabled.unwrap_or(true),
            sandbox: storage::models::SandboxConfig {
                registry_keys: vec![],
                files: vec!["d3dx_user.ini".to_string()],
            },
            loader_repo: template.loader_repo.clone(),
            hash_db_url: template.hash_db_url.clone(),
            patch_logic: template.patch_logic.clone(),
            enable_linux_shield: true,
            supported_injection_methods: template
                .supported_injection_methods
                .clone()
                .unwrap_or_default(),
            remote_info: None,
            enabled_effects: vec![],
            start_method: template.start_method.unwrap_or_else(|| {
                if game_id.contains("genshin") {
                    storage::models::StartMethod::Shell
                } else {
                    storage::models::StartMethod::Native
                }
            }),
            unsafe_mode: template.unsafe_mode.unwrap_or(false),
            umu_game_id: template.umu_game_id.clone(),
            use_umu: template.use_umu.unwrap_or(false),
            pre_launch_cmd: template.pre_launch_cmd.clone(),
            post_exit_cmd: template.post_exit_cmd.clone(),
            use_rabbit_wrapper: template.use_rabbit_wrapper.unwrap_or(false),
            total_playtime_secs: 0,
            last_played: None,
            hunting_mode: false,
            gui_collect_ini: None,
        };

        let mut db = LibraryDatabase::default();
        db.games.insert(game_id.clone(), config);
        db.profiles.insert(p_id, default_profile);

        librarian
            .save_game_db(&game_id, &db)
            .await
            .map_err(|e| e.to_string())?;
        drop(librarian);

        let mut dbs = self.game_dbs.lock().await;
        dbs.insert(game_id.clone(), db);
        drop(dbs);
        self.emit_library().await;

        Ok(game_id)
    }
}
impl AppCore {
    pub async fn get_install_options(
        &self,
        game_id: String,
    ) -> Result<Vec<sync::protocol::ManifestCategory>, String> {
        let dbs = self.game_dbs.lock().await;
        let db = dbs
            .get(&game_id)
            .ok_or_else(|| format!("Game {} not found", game_id))?;
        let _config = db
            .games
            .get(&game_id)
            .ok_or_else(|| format!("Config for {} missing", game_id))?;

        let templates = self.game_templates.lock().await;
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
            .ok_or_else(|| format!("Template not found for game ID: {}", game_id))?;

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
                true,
            )
            .await
            .map_err(|e| e.to_string())?;

        Ok(build
            .manifests
            .into_iter()
            .map(|m| {
                let name = match m.matching_field.as_str() {
                    "game" => "Game Core".to_string(),
                    "en-us" => "English Audio".to_string(),
                    "zh-cn" => "Chinese Audio".to_string(),
                    "ja-jp" => "Japanese Audio".to_string(),
                    "ko-kr" => "Korean Audio".to_string(),
                    "zh-tw" => "Traditional Chinese Audio".to_string(),
                    _ => m.category_name.clone(),
                };

                sync::protocol::ManifestCategory {
                    id: m.category_id,
                    name,
                    size: m.uncompressed_size,
                    is_required: m.matching_field == "game",
                }
            })
            .collect())
    }

    pub async fn force_reset_state(&self) -> Result<(), String> {
        *self.running_game_name.lock().await = None;
        *self.is_launching.lock().await = false;
        Ok(())
    }

    pub async fn extract_wish_url(&self, game_id: String) -> Result<String, String> {
        let dbs = self.game_dbs.lock().await;
        let db = dbs.get(&game_id).ok_or("Game not found")?;
        let config = db.games.get(&game_id).ok_or("Game config not found")?;

        let install_path = if config.install_path.exists() {
            config.install_path.clone()
        } else if let Some(parent) = config.exe_path.parent() {
            parent.to_path_buf()
        } else {
            config.install_path.clone()
        };
        drop(dbs);

        storage::extract_gacha_url(&install_path).map_err(|e| e.to_string())
    }

    pub async fn get_library(&self) -> Result<HashMap<String, LibraryDatabase>, String> {
        let dbs = self.game_dbs.lock().await.clone();
        for (id, db) in dbs.iter() {
            for (gid, g) in db.games.iter() {
                eprintln!("DIAG get_library {id}/{gid} status={:?}", g.install_status);
            }
        }
        Ok(dbs)
    }

    pub async fn get_skin_inventory(
        &self,
        game_id: String,
    ) -> Result<HashMap<String, storage::queries::CharacterGroup>, String> {
        let dbs = self.game_dbs.lock().await;
        let db = dbs
            .get(&game_id)
            .ok_or_else(|| format!("Game {} not found", game_id))?;
        Ok(storage::queries::Queries::get_character_roster(
            db, &game_id,
        ))
    }

    pub async fn open_path(&self, path: String) -> Result<(), String> {
        #[cfg(target_os = "linux")]
        let opener = "xdg-open";
        #[cfg(target_os = "windows")]
        let opener = "explorer";
        #[cfg(target_os = "macos")]
        let opener = "open";
        std::process::Command::new(opener)
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn identify_game(&self, path: String) -> Result<IdentifiedGame, String> {
        let mut install_path = PathBuf::from(&path);
        if !install_path.exists() {
            return Err("Path does not exist".to_string());
        }
        let mut exe_path = PathBuf::new();
        let mut exe_name = String::new();
        if install_path.is_file() {
            exe_path = install_path.clone();
            exe_name = exe_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            if let Some(parent) = install_path.parent() {
                install_path = parent.to_path_buf();
            }
        } else {
            let walker = walkdir::WalkDir::new(&install_path).max_depth(1);
            for entry in walker.into_iter().filter_map(|e| e.ok()) {
                if entry.path().is_file() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.ends_with(".exe")
                        || name
                            == install_path
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                    {
                        exe_path = entry.path().to_path_buf();
                        exe_name = name;
                        break;
                    }
                }
            }
        }
        if exe_name.is_empty() {
            return Err("Could not identify game executable".to_string());
        }
        let game_id = exe_name.to_lowercase();
        let templates_guard = self.game_templates.lock().await;
        let template = templates_guard.get(&game_id).or_else(|| {
            if game_id.ends_with(".exe") {
                templates_guard.get(game_id.trim_end_matches(".exe"))
            } else {
                None
            }
        });
        let size_bytes = Safety::get_dir_size(&install_path).map_err(|e| e.to_string())?;
        let size_str = format!("{:.1} GB", size_bytes as f64 / 1024.0 / 1024.0 / 1024.0);
        let mut version = "Unknown".to_string();
        if exe_path.exists() {
            let path_clone = exe_path.clone();
            if let Ok(Ok(v)) =
                tokio::task::spawn_blocking(move || ExeInspector::get_version(&path_clone)).await
            {
                version = v;
            }
        }
        if let Some(t) = template {
            let t_cloned = t.clone();
            drop(templates_guard);

            let librarian = self.librarian.lock().await;

            Ok(IdentifiedGame {
                id: game_id,
                name: t_cloned.name,
                short_name: t_cloned.short_name,
                developer: t_cloned.developer,
                description: t_cloned.description,
                version,
                size: size_str,
                color: t_cloned.color,
                accent_color: t_cloned.accent_color,
                cover_image: librarian.resolve_template_asset(&t_cloned.cover_image),
                icon: librarian.resolve_template_asset(&t_cloned.icon),
                logo_initial: t_cloned.logo_initial,
                install_path: install_path.to_string_lossy().to_string(),
                exe_name,
                supported_injection_methods: t_cloned
                    .supported_injection_methods
                    .unwrap_or_default()
                    .into_iter()
                    .map(map_injection)
                    .collect(),
                injection_method: {
                    let method = if cfg!(target_os = "windows") {
                        t_cloned.injection_method_windows
                    } else {
                        t_cloned.injection_method_linux
                    };
                    map_injection(method.unwrap_or(storage::InjectionMethod::None))
                },
                modloader_enabled: t_cloned.modloader_enabled.unwrap_or(true),
            })
        } else {
            drop(templates_guard);
            let app_config = self.app_config.lock().await;
            let name = install_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            Ok(IdentifiedGame {
                id: game_id,
                name: name.clone(),
                short_name: name.clone(),
                developer: "Unknown".to_string(),
                description: "A custom added game.".to_string(),
                version,
                size: size_str,
                color: "slate-400".to_string(),
                accent_color: "#94a3b8".to_string(),
                cover_image: app_config.default_cover_image.clone(),
                icon: app_config.default_icon_image.clone(),
                logo_initial: name.chars().next().unwrap_or('?').to_string(),
                install_path: install_path.to_string_lossy().to_string(),
                exe_name,
                supported_injection_methods: vec![InjectionMethod::Proxy],
                injection_method: InjectionMethod::Proxy,
                modloader_enabled: true,
            })
        }
    }
}
impl AppCore {
    pub async fn sync_game_assets(&self, game_id: String) -> Result<(), String> {
        let result = {
            let mut dbs = self.game_dbs.lock().await;
            let templates = self.game_templates.lock().await;

            if let (Some(db), Some(template)) = (dbs.get_mut(&game_id), templates.get(&game_id)) {
                if let Some(config) = db.games.get_mut(&game_id) {
                    let librarian = self.librarian.lock().await;

                    if config.cover_image.is_empty() || config.cover_image.contains("/templates/") {
                        config.cover_image =
                            librarian.resolve_template_asset(&template.cover_image);
                    }
                    if config.icon.is_empty() || config.icon.contains("/templates/") {
                        config.icon = librarian.resolve_template_asset(&template.icon);
                    }
                    config.accent_color = template.accent_color.clone();
                    config.color = template.color.clone();
                    config.logo_initial = template.logo_initial.clone();

                    Some((game_id.clone(), db.clone()))
                } else {
                    None
                }
            } else {
                None
            }
        };

        if let Some((id, db)) = result {
            self.librarian
                .lock()
                .await
                .save_game_db(&id, &db)
                .await
                .map_err(|e| e.to_string())?;
            self.emit_library().await;
            return Ok(());
        }

        Err("Game or Template not found".to_string())
    }

    pub async fn add_game(&self, path: String) -> Result<String, String> {
        let path_buf = PathBuf::from(&path);
        let templates_guard = self.game_templates.lock().await;

        let game_id = {
            let librarian = self.librarian.lock().await;
            Discovery::add_game_by_path(&librarian, path_buf, &templates_guard)
                .await
                .map_err(|e| e.to_string())?
        };
        drop(templates_guard);

        #[cfg(target_os = "linux")]
        {
            let settings = self.global_settings.lock().await;
            let base_storage = if settings.yago_storage_path.as_os_str().is_empty() {
                self.app_data_dir.clone()
            } else {
                settings.yago_storage_path.clone()
            };
            let prefix_path = if !settings.prefixes_path.as_os_str().is_empty() {
                settings.prefixes_path.join(&game_id)
            } else {
                base_storage.join("prefixes").join(&game_id)
            };
            drop(settings);
            if !prefix_path.exists() {
                std::fs::create_dir_all(prefix_path.join("pfx")).map_err(|e| e.to_string())?;
            }
            let mut dbs = self.game_dbs.lock().await;
            if let Some(db) = dbs.get_mut(&game_id) {
                if let Some(config) = db.games.get_mut(&game_id) {
                    config.prefix_path = Some(prefix_path);
                    self.librarian
                        .lock()
                        .await
                        .save_game_db(&game_id, db)
                        .await
                        .map_err(|e| e.to_string())?;
                }
            }
        }

        let db_res = {
            let librarian = self.librarian.lock().await;
            librarian.load_game_db(&game_id).await
        };

        if let Ok(mut db) = db_res {
            if let Some(config) = db.games.get_mut(&game_id) {
                let templates = self.game_templates.lock().await;
                if let Some(template) = templates.get(&game_id) {
                    if !template.sophon_package_id.is_empty() {
                        let client = SophonClient::new();
                        let build_res = client
                            .get_build(
                                &template.sophon_branch,
                                &template.sophon_package_id,
                                &template.sophon_password,
                                &template.sophon_plat_app,
                                &template.sophon_game_biz,
                                &template.sophon_launcher_id,
                                &template.sophon_channel_id,
                                &template.sophon_sub_channel_id,
                                true,
                            )
                            .await;

                        if let Ok(build) = build_res {
                            config.remote_version = Some(build.version);
                            let _ = self
                                .librarian
                                .lock()
                                .await
                                .save_game_db(&game_id, &db)
                                .await;
                        }
                    }
                }
            }

            let mut dbs = self.game_dbs.lock().await;
            dbs.insert(game_id.clone(), db);
            drop(dbs);
            self.emit_library().await;
        }
        Ok(game_id)
    }

    pub async fn remove_game(&self, game_id: String) -> Result<(), String> {
        {
            let controls = self.download_controls.lock().await;
            if let Some(tx) = controls.get(&game_id) {
                let _ = tx.send(true);
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        self.download_controls.lock().await.remove(&game_id);

        let mut dbs = self.game_dbs.lock().await;
        if let Some(db) = dbs.remove(&game_id) {
            let librarian = self.librarian.lock().await;
            let game_paths = librarian.game_paths(&game_id);

            if game_paths.root.exists() {
                std::fs::remove_dir_all(game_paths.root).map_err(|e| e.to_string())?;
            }
            #[cfg(target_os = "linux")]
            {
                if let Some(config) = db.games.get(&game_id) {
                    if let Some(prefix_path) = &config.prefix_path {
                        if prefix_path.exists() {
                            let _ = std::fs::remove_dir_all(prefix_path);
                        }
                    }
                }
            }
            drop(dbs);
            self.emit_library().await;
            return Ok(());
        }
        Err("Game not found".to_string())
    }

    pub async fn scan_for_games(&self) -> Result<Vec<DiscoveredGame>, String> {
        let librarian = self.librarian.lock().await;
        let templates_root = &librarian.templates_root;
        if !templates_root.exists() {
            return Ok(vec![]);
        }
        let templates_guard = self.game_templates.lock().await;
        let templates_vec: Vec<_> = templates_guard.values().cloned().collect();
        Ok(storage::scanner::scan(&templates_vec))
    }

    pub async fn recursive_scan_path(&self, path: String) -> Result<Vec<DiscoveredGame>, String> {
        let templates_guard = self.game_templates.lock().await;
        let templates_vec: Vec<_> = templates_guard.values().cloned().collect();

        let path_buf = PathBuf::from(path);
        if !path_buf.exists() {
            return Err("Path does not exist".to_string());
        }

        Ok(storage::scanner::recursive_scan(
            &path_buf,
            &templates_vec,
            4,
        ))
    }

    pub async fn trust_game_installation(&self, game_id: String) -> Result<(), String> {
        let mut dbs = self.game_dbs.lock().await;
        if let Some(db) = dbs.get_mut(&game_id) {
            if let Some(config) = db.games.get_mut(&game_id) {
                config.install_status = storage::models::InstallStatus::Installed;

                if config.version == "Unknown" || config.version == "Pending" {
                    if let Some(rv) = &config.remote_version {
                        config.version = rv.clone();
                    }
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
        Err("Game not found".to_string())
    }
}
impl AppCore {
    pub async fn sync_templates(&self) -> Result<(), String> {
        let templates_root = self.librarian.lock().await.templates_root.clone();

        if !templates_root.exists() {
            std::fs::create_dir_all(&templates_root).map_err(|e| e.to_string())?;
        }
        let new_templates = self.load_all_templates().await;
        *self.game_templates.lock().await = new_templates;

        let ids: Vec<String> = self.game_dbs.lock().await.keys().cloned().collect();
        let client = SophonClient::new();

        for id in ids {
            let template = self.game_templates.lock().await.get(&id).cloned();

            if let Some(template) = template {
                let mut remote_v = None;

                if !template.sophon_package_id.is_empty() {
                    let build_res = client
                        .get_build(
                            &template.sophon_branch,
                            &template.sophon_package_id,
                            &template.sophon_password,
                            &template.sophon_plat_app,
                            &template.sophon_game_biz,
                            &template.sophon_launcher_id,
                            &template.sophon_channel_id,
                            &template.sophon_sub_channel_id,
                            true,
                        )
                        .await;

                    if let Ok(build) = build_res {
                        remote_v = Some(build.version);
                    }
                }

                let (_changed, db_to_save) = {
                    let mut dbs = self.game_dbs.lock().await;
                    if let Some(db) = dbs.get_mut(&id) {
                        let mut changed = false;
                        if let Some(config) = db.games.get_mut(&id) {
                            if config.version == "Unknown" || config.version == "Pending" {
                                let mut current = Some(config.install_path.as_path());
                                for _ in 0..3 {
                                    if let Some(p) = current {
                                        let exe_path = p.join(&config.exe_name);
                                        if let Ok(v) = ExeInspector::get_version(&exe_path) {
                                            if v != "Unknown" {
                                                config.version = v;
                                                changed = true;
                                                break;
                                            }
                                        }
                                        current = p.parent();
                                    }
                                }
                            }

                            if let Some(rv) = remote_v {
                                if config.remote_version != Some(rv.clone()) {
                                    config.remote_version = Some(rv);
                                    changed = true;
                                }
                            }
                        }
                        (changed, if changed { Some(db.clone()) } else { None })
                    } else {
                        (false, None)
                    }
                };

                if let Some(db) = db_to_save {
                    self.librarian
                        .lock()
                        .await
                        .save_game_db(&id, &db)
                        .await
                        .map_err(|e| e.to_string())?;
                    self.emit_library().await;
                }
            }
        }

        Ok(())
    }

    pub async fn list_runners(&self) -> Result<Vec<String>, String> {
        let mut runners = std::collections::HashSet::new();
        let librarian = self.librarian.lock().await;
        let runners_dir = &librarian.runners_root;
        if runners_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(runners_dir) {
                for entry in entries.filter_map(|e| e.ok()) {
                    if entry.path().is_dir() {
                        runners.insert(entry.file_name().to_string_lossy().to_string());
                    }
                }
            }
        }
        drop(librarian);

        let settings = self.global_settings.lock().await;
        if !settings.steam_compat_tools_path.as_os_str().is_empty()
            && settings.steam_compat_tools_path.exists()
        {
            if let Ok(entries) = std::fs::read_dir(&settings.steam_compat_tools_path) {
                for entry in entries.filter_map(|e| e.ok()) {
                    if entry.path().is_dir() {
                        runners.insert(entry.file_name().to_string_lossy().to_string());
                    }
                }
            }
        }
        let mut result: Vec<String> = runners.into_iter().collect();
        result.sort();
        Ok(result)
    }

    pub async fn remove_runner(&self, runner_id: String) -> Result<(), String> {
        let librarian = self.librarian.lock().await;
        let runners_dir = librarian.runners_root.join(&runner_id);
        if !runners_dir.exists() {
            return Err("Runner not found".to_string());
        }
        if runner_id.contains("..") || runner_id.contains('/') || runner_id.contains('\\') {
            return Err("Invalid runner ID".to_string());
        }
        std::fs::remove_dir_all(&runners_dir).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn detect_steam_proton_path(&self) -> Result<Option<String>, String> {
        detect_steam_proton_path_internal().await
    }

    pub async fn wipe_game_mods(&self, game_id: String) -> Result<(), String> {
        let mut dbs = self.game_dbs.lock().await;
        if let Some(db) = dbs.get_mut(&game_id) {
            let librarian = self.librarian.lock().await;
            let mods_root = librarian.game_paths(&game_id).mods;
            if mods_root.exists() {
                std::fs::remove_dir_all(&mods_root).map_err(|e| e.to_string())?;
                std::fs::create_dir_all(&mods_root).map_err(|e| e.to_string())?;
            }

            db.mods.clear();
            for profile in db.profiles.values_mut() {
                profile.enabled_mod_ids.clear();
                profile.load_order.clear();
            }

            librarian
                .save_game_db(&game_id, db)
                .await
                .map_err(|e| e.to_string())?;
            drop(dbs);
            self.emit_library().await;
            return Ok(());
        }
        Err("Game not found".to_string())
    }

    pub async fn reset_game_profiles(&self, game_id: String) -> Result<(), String> {
        let mut dbs = self.game_dbs.lock().await;
        if let Some(db) = dbs.get_mut(&game_id) {
            let librarian = self.librarian.lock().await;

            db.profiles.clear();
            let default_profile = storage::models::Profile::default();
            let p_id = default_profile.id;

            if let Some(config) = db.games.get_mut(&game_id) {
                config.active_profile_id = p_id.to_string();
            }

            db.profiles.insert(p_id, default_profile);

            librarian
                .save_game_db(&game_id, db)
                .await
                .map_err(|e| e.to_string())?;
            drop(dbs);
            self.emit_library().await;
            return Ok(());
        }
        Err("Game not found".to_string())
    }

    pub async fn remove_game_prefix(&self, game_id: String) -> Result<(), String> {
        let dbs = self.game_dbs.lock().await;
        if let Some(db) = dbs.get(&game_id) {
            if let Some(config) = db.games.get(&game_id) {
                if let Some(prefix_path) = &config.prefix_path {
                    if prefix_path.exists() {
                        std::fs::remove_dir_all(prefix_path).map_err(|e| e.to_string())?;
                        return Ok(());
                    }
                }
            }
        }
        Err("Prefix not found or game missing".to_string())
    }

    pub async fn uninstall_game_files(&self, game_id: String) -> Result<(), String> {
        let install_path = {
            let dbs = self.game_dbs.lock().await;
            dbs.get(&game_id)
                .and_then(|db| db.games.get(&game_id))
                .map(|c| c.install_path.clone())
        };

        self.remove_game(game_id).await?;

        if let Some(path) = install_path {
            if path.exists() && path.is_dir() {
                std::fs::remove_dir_all(path).map_err(|e| e.to_string())?;
            }
        }

        Ok(())
    }

    pub async fn sync_hash_db(&self, game_id: String) -> Result<usize, String> {
        let url = {
            let templates = self.game_templates.lock().await;
            templates
                .get(&game_id)
                .or_else(|| templates.get(game_id.trim_end_matches(".exe")))
                .and_then(|t| t.hash_db_url.clone())
        };

        let url = match url {
            Some(u) => u,
            None => {
                let dbs = self.game_dbs.lock().await;
                dbs.get(&game_id)
                    .and_then(|db| db.games.get(&game_id))
                    .and_then(|c| c.hash_db_url.clone())
                    .ok_or_else(|| {
                        format!("No hash database URL configured for game '{}'", game_id)
                    })?
            }
        };

        let hashes = storage::cloud::CloudSync::fetch_latest_hashes(&url)
            .await
            .map_err(|e| format!("Failed to fetch remote hashes: {}", e))?;

        let count = if let Some(chars) = hashes.get("characters").and_then(|c| c.as_object()) {
            chars.len()
        } else if let Some(map) = hashes.as_object() {
            map.len()
        } else {
            0
        };

        let librarian = self.librarian.lock().await;
        let hashes_dir = librarian.assets_root.join("hashes");
        if !hashes_dir.exists() {
            std::fs::create_dir_all(&hashes_dir).map_err(|e| e.to_string())?;
        }

        let target_file = hashes_dir.join(format!("{}.json", game_id));
        let tmp_file = hashes_dir.join(format!("{}.json.tmp.{}", game_id, uuid::Uuid::new_v4()));

        let json_str = serde_json::to_string_pretty(&hashes).map_err(|e| e.to_string())?;
        std::fs::write(&tmp_file, json_str).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp_file, &target_file).map_err(|e| e.to_string())?;

        Ok(count)
    }
}

pub async fn detect_steam_proton_path_internal() -> Result<Option<String>, String> {
    #[cfg(target_os = "linux")]
    {
        let home = std::env::var("HOME").map_err(|e| e.to_string())?;
        let paths = [
            format!("{}/.steam/steam/compatibilitytools.d", home),
            format!("{}/.steam/root/compatibilitytools.d", home),
            format!("{}/.local/share/Steam/compatibilitytools.d", home),
            format!(
                "{}/.var/app/com.valvesoftware.Steam/data/Steam/compatibilitytools.d",
                home
            ),
        ];
        for path_str in paths {
            let path = std::path::PathBuf::from(&path_str);
            if path.exists() && path.is_dir() {
                return Ok(Some(path_str));
            }
        }
    }
    Ok(None)
}
