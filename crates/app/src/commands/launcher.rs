use std::path::{Path, PathBuf};

use ini::{IniDocument, IniPatcher};
use launcher::{LaunchOptions, Launcher, Monitor, RunnerConfig, RunnerType};
use loader::LoaderContext;
use mod_patches::ConflictReport;
use uuid::Uuid;

use crate::commands::library::{detect_steam_proton_path_internal, GameConfigUpdate};
use crate::events::AppEvent;
use crate::safety::Emergency;
use crate::state::AppCore;

pub async fn resolve_runner_path(
    rid_opt: Option<String>,
    app_data_dir: &Path,
    settings: &storage::settings::GlobalSettings,
) -> (PathBuf, RunnerType) {
    let base_storage = if settings.yago_storage_path.as_os_str().is_empty() {
        app_data_dir.to_path_buf()
    } else {
        settings.yago_storage_path.clone()
    };

    let find_proton = |dir: PathBuf| -> Option<PathBuf> {
        let possible = ["proton", "proton.sh", "files/bin/proton"];
        for p in possible {
            let path = dir.join(p);
            if path.exists() {
                return Some(path);
            }
        }
        None
    };

    if let Some(rid) = rid_opt {
        let local_dir = if !settings.runners_path.as_os_str().is_empty() {
            settings.runners_path.join(&rid)
        } else {
            base_storage.join("runners").join(&rid)
        };
        let settings_dir = settings.steam_compat_tools_path.join(&rid);

        if let Some(p) = find_proton(local_dir.clone()) {
            (p, RunnerType::Proton)
        } else if let Some(p) = find_proton(settings_dir.clone()) {
            (p, RunnerType::Proton)
        } else {
            let mut detected_p = None;
            if cfg!(target_os = "linux") {
                if let Ok(Some(detected)) = detect_steam_proton_path_internal().await {
                    let detected_dir = PathBuf::from(detected).join(&rid);
                    detected_p = find_proton(detected_dir);
                }
            }

            if let Some(p) = detected_p {
                (p, RunnerType::Proton)
            } else {
                (PathBuf::from("wine"), RunnerType::Wine)
            }
        }
    } else {
        (PathBuf::from("wine"), RunnerType::Wine)
    }
}

fn resource_lib(name: &str) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let path = exe.parent()?.join("libs").join(name);
    if path.exists() {
        Some(path)
    } else {
        None
    }
}

impl AppCore {
    pub async fn trigger_panic(&self) -> Result<(), String> {
        Emergency::trigger(self).await;
        Ok(())
    }

    pub async fn kill_game(&self) -> Result<(), String> {
        let mut running = self.running_game_name.lock().await;
        if let Some(exe_name) = running.as_ref() {
            if Monitor::kill_by_name(exe_name) {
                *running = None;
                Ok(())
            } else {
                Err("Could not find process to kill".to_string())
            }
        } else {
            let mut launching = self.is_launching.lock().await;
            if *launching {
                *launching = false;
                Ok(())
            } else {
                Err("No game running".into())
            }
        }
    }

    pub async fn deploy_mods(
        &self,
        game_path: String,
        force: bool,
    ) -> Result<ConflictReport, String> {
        let exe_path = PathBuf::from(&game_path);
        let exe_name = exe_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        let game_id = exe_name.to_lowercase();
        let game_root = exe_path.parent().ok_or("Invalid path")?.to_path_buf();
        let dbs = self.game_dbs.lock().await;
        let db = dbs.get(&game_id).ok_or("Game not found")?;
        let config = db.games.get(&game_id).ok_or("Config missing")?;
        let p_uuid = Uuid::parse_str(&config.active_profile_id).map_err(|e| e.to_string())?;
        let profile = db.profiles.get(&p_uuid).ok_or("Profile missing")?;

        if !config.modloader_enabled {
            if !profile.use_reshade {
                let _ = LoaderContext::uninstall_loader(
                    &game_root,
                    config.prefix_path.as_deref(),
                    true,
                    config.unsafe_mode,
                )
                .await;
            }
            return Ok(ConflictReport::default());
        }

        let suppress_global = self.global_settings.lock().await.suppress_creditinfo_global;

        let mut profiles_for_weaver = Vec::new();
        let mut add_to_list = |mod_id: &Uuid| {
            if let Some(record) = db.mods.get(mod_id) {
                let mut ini_paths = Vec::new();
                for entry in walkdir::WalkDir::new(&record.path)
                    .max_depth(3)
                    .into_iter()
                    .filter_map(|e| e.ok())
                {
                    let path = entry.path();
                    let name_str = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                    let is_ini = path
                        .extension()
                        .and_then(|s| s.to_str())
                        .map(|s| s.to_lowercase())
                        == Some("ini".to_string());
                    let is_source = name_str.to_lowercase().ends_with(".ini.yago_source");

                    if path.is_file() && (is_ini || is_source) {
                        ini_paths.push(entry.path().to_path_buf());
                    }
                }

                let mod_type = record.config.mod_type.clone().unwrap_or_else(|| {
                    if record.compatibility.character != "Unknown"
                        && record.compatibility.character != "Global/Other"
                    {
                        "character".to_string()
                    } else {
                        "global".to_string()
                    }
                });

                let active_preset_vars = record.config.active_preset_id.as_ref().and_then(|pid| {
                    if pid == "default" {
                        Some(mod_patches::extract_default_preset(&record.path).variables)
                    } else {
                        record
                            .config
                            .variant_presets
                            .iter()
                            .find(|p| p.id == *pid)
                            .map(|p| p.variables.clone())
                    }
                });

                let suppress_creditinfo = record.config.suppress_creditinfo || suppress_global;

                profiles_for_weaver.push(mod_patches::ModProfile {
                    uuid: record.id.to_string(),
                    mod_root: record.path.clone(),
                    ini_paths,
                    deploy_path: "".to_string(),
                    character: record.compatibility.character.clone(),
                    mod_type,
                    nsfw: record
                        .config
                        .tags
                        .iter()
                        .any(|t| t.to_lowercase() == "nsfw"),
                    tags: record.config.tags.clone(),
                    active_preset_vars,
                    ini_anchors: record.config.ini_anchors.clone(),
                    suppress_creditinfo,
                    context_pool: record.config.context_pool.clone(),
                });
            }
        };
        for mid in &profile.load_order {
            if profile.enabled_mod_ids.contains(mid) {
                add_to_list(mid);
            }
        }
        for mid in db.mods.keys() {
            if !profile.load_order.contains(mid) && profile.enabled_mod_ids.contains(mid) {
                add_to_list(mid);
            }
        }
        let compile_options = mod_patches::CompileOptions {
            enable_sfw_toggle: profile.enable_sfw_toggle,
            sfw_toggle_key: profile
                .sfw_toggle_key
                .clone()
                .unwrap_or_else(|| "F9".to_string()),
            enable_context_swaps: profile.enable_context_swaps,
        };

        let (mut plan, report) = mod_patches::generate_deployment_plan_with_options(
            profiles_for_weaver,
            compile_options,
        )
        .map_err(|e| e.to_string())?;

        let lib_guard = self.librarian.lock().await;
        let paths = lib_guard.game_paths(&game_id);
        let loaders_root = lib_guard.loaders_root.clone();

        // Include enabled and installed shared effect libraries in the deployment plan
        for eff_id in &config.enabled_effects {
            let effect_dir = lib_guard.assets_root.join("effects").join(eff_id);
            if effect_dir.exists() {
                let target_rel = PathBuf::from("00_Configs").join("Effects").join(eff_id);
                plan.symlink_map.push((effect_dir, target_rel));
            }
        }

        // Bridge gui_collect auto_generated.ini if configured and exists
        if let Some(gui_path) = &config.gui_collect_ini {
            if gui_path.exists() {
                let bridge_content = format!(
                    "; YAGO gui_collect Bridge\ninclude = {}\n",
                    gui_path.display()
                );
                plan.generated_files.push((
                    PathBuf::from("00_Configs/yago_gui_collect_bridge.ini"),
                    bridge_content,
                ));
            }
        }

        let profile_dir = paths.profile_mods.parent().ok_or("Invalid profile path")?;
        if !profile_dir.exists() {
            std::fs::create_dir_all(profile_dir).map_err(|e| e.to_string())?;
        }

        vfs::execute_deployment(profile_dir, &plan, Some("Mods"), force)
            .map_err(|e| e.to_string())?;

        let game_mods_dir = game_root.join("Mods");
        let is_proxy = config.injection_method == storage::InjectionMethod::Proxy
            || (cfg!(target_os = "linux")
                && config.injection_method == storage::InjectionMethod::Loader);

        if is_proxy {
            vfs::make_symlink(&paths.profile_mods, &game_mods_dir).map_err(|e| e.to_string())?;
        } else if game_mods_dir.exists() {
            #[cfg(unix)]
            let _ = std::fs::remove_file(&game_mods_dir);
            #[cfg(windows)]
            let _ = std::fs::remove_dir_all(&game_mods_dir);
        }

        // Link ShaderFixes for enabled mods and cleanup stale symlinks for disabled mods
        let active_shader_fixes_dir = game_root.join("ShaderFixes");
        let has_any_shaders = db.mods.values().any(|m| !m.config.shader_files.is_empty());
        if active_shader_fixes_dir.exists() || has_any_shaders {
            let _ = std::fs::create_dir_all(&active_shader_fixes_dir);
            let mut active_shader_names = std::collections::HashSet::new();

            for mid in &profile.enabled_mod_ids {
                if let Some(record) = db.mods.get(mid) {
                    for rel_shader in &record.config.shader_files {
                        let src_path = record.path.join(rel_shader);
                        if src_path.exists() {
                            if let Some(file_name) = Path::new(rel_shader).file_name() {
                                let dest_path = active_shader_fixes_dir.join(file_name);
                                active_shader_names.insert(file_name.to_string_lossy().to_string());
                                let _ = vfs::make_symlink(&src_path, &dest_path);
                            }
                        }
                    }
                }
            }

            for (mid, record) in &db.mods {
                if !profile.enabled_mod_ids.contains(mid) {
                    for rel_shader in &record.config.shader_files {
                        if let Some(file_name) = Path::new(rel_shader).file_name() {
                            let name_str = file_name.to_string_lossy().to_string();
                            if !active_shader_names.contains(&name_str) {
                                let stale_path = active_shader_fixes_dir.join(file_name);
                                if stale_path.exists() {
                                    let _ = std::fs::remove_file(stale_path);
                                }
                            }
                        }
                    }
                }
            }
        }

        let target_ini = if !is_proxy {
            loaders_root.join("d3dx.ini")
        } else {
            game_root.join("d3dx.ini")
        };
        if target_ini.exists() {
            <IniDocument as IniPatcher>::patch_file(&target_ini, "Loader", "target", exe_name)
                .map_err(|e| e.to_string())?;
            if let Some(patches) = &config.patch_logic {
                <IniDocument as IniPatcher>::patch_config(&target_ini, patches)
                    .map_err(|e| e.to_string())?;
            } else {
                let guard = self.game_templates.lock().await;
                if let Some(t) = guard.get(&game_id) {
                    if let Some(p) = &t.patch_logic {
                        <IniDocument as IniPatcher>::patch_config(&target_ini, p)
                            .map_err(|e| e.to_string())?;
                    }
                }
            }

            // Patch hunting mode
            let hunting_val = if config.hunting_mode { "2" } else { "0" };
            let _ = <IniDocument as IniPatcher>::patch_file(
                &target_ini,
                "Hunting",
                "hunting",
                hunting_val,
            );
        }
        self.emit(AppEvent::TaskCompleted("Deployment successful".to_string()));
        Ok(report)
    }
}
impl AppCore {
    pub async fn redeploy_mods(&self, game_path: String) -> Result<(), String> {
        self.deploy_mods(game_path, true).await?;

        launcher::input::send_reload_f10().map_err(|e| format!("Failed to send F10: {}", e))?;

        self.emit(AppEvent::TaskCompleted("Hot Reload successful".to_string()));
        Ok(())
    }

    pub async fn update_game_config(
        &self,
        game_id: String,
        update: GameConfigUpdate,
    ) -> Result<(), String> {
        let mut dbs = self.game_dbs.lock().await;
        if let Some(db) = dbs.get_mut(&game_id) {
            if let Some(config) = db.games.get_mut(&game_id) {
                if let Some(n) = update.name {
                    config.name = n;
                }
                if let Some(c) = update.cover_image {
                    config.cover_image = c;
                }
                if let Some(i) = update.icon {
                    config.icon = i;
                }
                if let Some(dev) = update.developer {
                    config.developer = dev;
                }
                if let Some(d) = update.description {
                    config.description = d;
                }
                if let Some(p) = update.install_path {
                    config.install_path = PathBuf::from(p);
                    config.exe_path = config.install_path.join(&config.exe_name);
                }
                if let Some(e) = update.exe_name {
                    config.exe_name = e;
                    config.exe_path = config.install_path.join(&config.exe_name);
                }
                if let Some(a) = update.launch_args {
                    config.launch_args = a;
                }
                if let Some(ga) = update.gamescope_args {
                    config.gamescope_args = ga;
                }
                if let Some(f) = update.fps_config {
                    config.fps_config = Some(f);
                }
                if let Some(sn) = update.short_name {
                    config.short_name = sn;
                }
                if let Some(c) = update.color {
                    config.color = c;
                }
                if let Some(ac) = update.accent_color {
                    config.accent_color = ac;
                }
                if let Some(l) = update.logo_initial {
                    config.logo_initial = l;
                }
                if let Some(im) = update.injection_method {
                    config.injection_method = match im {
                        launcher::InjectionMethod::None => storage::InjectionMethod::None,
                        launcher::InjectionMethod::Proxy => storage::InjectionMethod::Proxy,
                        launcher::InjectionMethod::Loader => storage::InjectionMethod::Loader,
                        launcher::InjectionMethod::RemoteThread => {
                            storage::InjectionMethod::RemoteThread
                        }
                        launcher::InjectionMethod::ManualMap => storage::InjectionMethod::ManualMap,
                    };
                }
                if let Some(me) = update.modloader_enabled {
                    config.modloader_enabled = me;
                }
                if let Some(au) = update.auto_update {
                    config.auto_update = au;
                }
                if let Some(pid) = update.active_profile_id {
                    config.active_profile_id = pid;
                }
                if let Some(rid) = update.active_runner_id {
                    config.active_runner_id = rid;
                }
                if let Some(pp) = update.prefix_path {
                    config.prefix_path = pp.map(PathBuf::from);
                }
                if let Some(els) = update.enable_linux_shield {
                    config.enable_linux_shield = els;
                }
                if let Some(sm) = update.start_method {
                    config.start_method = sm;
                }
                if let Some(um) = update.unsafe_mode {
                    config.unsafe_mode = um;
                }
                if let Some(ugid) = update.umu_game_id {
                    config.umu_game_id = ugid;
                }
                if let Some(uu) = update.use_umu {
                    config.use_umu = uu;
                }
                if let Some(plc) = update.pre_launch_cmd {
                    config.pre_launch_cmd = plc;
                }
                if let Some(pec) = update.post_exit_cmd {
                    config.post_exit_cmd = pec;
                }
                if let Some(urw) = update.use_rabbit_wrapper {
                    config.use_rabbit_wrapper = urw;
                }
                if let Some(hm) = update.hunting_mode {
                    config.hunting_mode = hm;
                }
                if let Some(gci) = update.gui_collect_ini {
                    config.gui_collect_ini = gci;
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
    pub async fn launch_game(&self, game_id: String) -> Result<(), String> {
        {
            let mut launching = self.is_launching.lock().await;
            if *launching {
                return Err("Already launching".into());
            }
            *launching = true;
        }
        let res = async {
            let (
                exe_path,
                exe_name,
                game_dir,
                profile,
                settings,
                active_runner_id,
                config_prefix_path,
                config_injection_method,
                config_gamescope_args,
                sandbox_config,
                profile_data_dir,
                enable_linux_shield,
                modloader_enabled,
                base_storage,
                config_start_method,
                config_unsafe_mode,
                config_umu_game_id,
                config_use_umu,
                config_pre_launch_cmd,
                config_post_exit_cmd,
                config_use_rabbit_wrapper,
            ) = {
                let dbs = self.game_dbs.lock().await;
                let db = dbs.get(&game_id).ok_or("Game not found")?;
                let config = db.games.get(&game_id).ok_or("Config missing")?;
                let p_uuid =
                    Uuid::parse_str(&config.active_profile_id).map_err(|e| e.to_string())?;
                let profile = db.profiles.get(&p_uuid).ok_or("Profile missing")?.clone();
                let settings = self.global_settings.lock().await.clone();
                let base_storage = if settings.yago_storage_path.as_os_str().is_empty() {
                    self.app_data_dir.clone()
                } else {
                    settings.yago_storage_path.clone()
                };
                let profile_data_dir = self
                    .librarian
                    .lock()
                    .await
                    .get_profile_data_dir(&game_id, &p_uuid);
                (
                    config.exe_path.clone(),
                    config.exe_name.clone(),
                    config.install_path.clone(),
                    profile.clone(),
                    settings,
                    config.active_runner_id.clone(),
                    config.prefix_path.clone(),
                    config.injection_method,
                    profile.gamescope_args.clone(),
                    config.sandbox.clone(),
                    profile_data_dir,
                    config.enable_linux_shield,
                    config.modloader_enabled,
                    base_storage,
                    config.start_method,
                    config.unsafe_mode,
                    config.umu_game_id.clone(),
                    config.use_umu,
                    config.pre_launch_cmd.clone(),
                    config.post_exit_cmd.clone(),
                    config.use_rabbit_wrapper,
                )
            };
            let method = if modloader_enabled {
                match config_injection_method {
                    storage::InjectionMethod::None => launcher::InjectionMethod::None,
                    storage::InjectionMethod::Proxy => launcher::InjectionMethod::Proxy,
                    storage::InjectionMethod::Loader => launcher::InjectionMethod::Loader,
                    storage::InjectionMethod::RemoteThread => {
                        launcher::InjectionMethod::RemoteThread
                    }
                    storage::InjectionMethod::ManualMap => launcher::InjectionMethod::ManualMap,
                }
            } else {
                launcher::InjectionMethod::None
            };
            let prefix_path = config_prefix_path.unwrap_or_else(|| {
                if !settings.wine_prefix_path.as_os_str().is_empty() {
                    settings.wine_prefix_path.clone()
                } else if !settings.prefixes_path.as_os_str().is_empty() {
                    settings.prefixes_path.join(&game_id)
                } else {
                    base_storage.join("prefixes").join(&game_id)
                }
            });

            let rid_opt = active_runner_id
                .as_ref()
                .filter(|s: &&String| !s.is_empty())
                .or(settings
                    .default_runner_id
                    .as_ref()
                    .filter(|s: &&String| !s.is_empty()))
                .cloned();

            let (runner_path, runner_type) =
                resolve_runner_path(rid_opt, &self.app_data_dir, &settings).await;

            let final_args = if profile.launch_args.is_empty() {
                vec![
                    "-popupwindow".into(),
                    "-screen-fullscreen".into(),
                    "0".into(),
                ]
            } else {
                profile.launch_args.clone()
            };
            let mut options = LaunchOptions {
                exe_path: exe_path.clone(),
                args: final_args,
                current_dir: Some(game_dir.clone()),
                runner: RunnerConfig {
                    runner_type,
                    path: runner_path,
                },
                prefix_path: prefix_path.clone(),
                use_gamescope: profile.use_gamescope,
                gamescope_args: config_gamescope_args,
                use_gamemode: profile.use_gamemode,
                use_mangohud: profile.use_mangohud,
                injection_method: method,
                modloader_enabled,
                loader_path: Some(base_storage.join("loaders").join(&game_id)),
                injected_dlls: vec![],
                resolution: profile.resolution.unwrap_or((1920, 1080)),
                fps_target: None,
                sandbox_config: Some(launcher::SandboxConfig {
                    registry_keys: sandbox_config.registry_keys,
                    files: sandbox_config.files,
                }),
                sandbox_data_dir: Some(profile_data_dir.clone()),
                enable_linux_shield,
                shield_path: {
                    let stable = self.app_data_dir.join("libs/libshield.so");
                    if stable.exists() {
                        Some(stable)
                    } else {
                        resource_lib("libshield.so")
                    }
                },
                helper_path: {
                    let stable = self.app_data_dir.join("libs/win_helper.exe");
                    if stable.exists() {
                        Some(stable)
                    } else {
                        resource_lib("win_helper.exe")
                    }
                },
                start_method: crate::commands::library::map_start_method(config_start_method),
                use_umu: config_use_umu,
                umu_game_id: config_umu_game_id.clone(),
                proton_wayland: profile.proton_wayland,
                dxvk_low_latency: profile.dxvk_low_latency,
                obs_vkcapture: profile.obs_vkcapture,
                lsfg_vk: profile.lsfg_vk,
                pre_launch_cmd: config_pre_launch_cmd.clone(),
                post_exit_cmd: config_post_exit_cmd.clone(),
            };
            if method == launcher::InjectionMethod::Loader {
                let _ = LoaderContext::uninstall_loader(&game_dir, None, false, config_unsafe_mode)
                    .await;
            }
            #[cfg(target_os = "linux")]
            {
                Launcher
                    .prepare_prefix(&options)
                    .await
                    .map_err(|e| e.to_string())?;
            }
            let loaders_root = base_storage.join("loaders");
            let config_use_rabbit = config_use_rabbit_wrapper;
            if method == launcher::InjectionMethod::Proxy {
                LoaderContext::install_loader(
                    &game_dir,
                    &loaders_root,
                    loader::InstallOptions {
                        game_id: game_id.clone(),
                        install_reshade: profile.use_reshade,
                        injection_method: Some("Proxy".to_string()),
                        unsafe_mode: config_unsafe_mode,
                        use_rabbit_wrapper: config_use_rabbit,
                    },
                )
                .await
                .map_err(|e| e.to_string())?;
            } else if method == launcher::InjectionMethod::None && profile.use_reshade {
                LoaderContext::install_loader(
                    &game_dir,
                    &loaders_root,
                    loader::InstallOptions {
                        game_id: game_id.clone(),
                        install_reshade: true,
                        injection_method: Some("ReShadeOnly".to_string()),
                        unsafe_mode: config_unsafe_mode,
                        use_rabbit_wrapper: config_use_rabbit,
                    },
                )
                .await
                .map_err(|e| e.to_string())?;
                options.injection_method = launcher::InjectionMethod::Proxy;
            }
            if profile.shuffle_on_launch {
                let _ = self.shuffle_character_skins(game_id.clone()).await;
            }

            self.deploy_mods(exe_path.to_string_lossy().to_string(), false)
                .await?;
            let session_start = std::time::Instant::now();
            match Launcher.launch(options).await {
                Ok(_) => {
                    *self.running_game_name.lock().await = Some(exe_name.clone());
                    self.emit(AppEvent::GameStarted);
                    let core = self.clone();
                    let post_game_id = game_id.clone();

                    tokio::spawn(async move {
                        Monitor::wait_for_exit(exe_name).await;
                        let elapsed = session_start.elapsed().as_secs();

                        {
                            let mut dbs = core.game_dbs.lock().await;
                            if let Some(db) = dbs.get_mut(&post_game_id) {
                                if let Some(config) = db.games.get_mut(&post_game_id) {
                                    config.total_playtime_secs += elapsed;
                                    config.last_played = Some(chrono::Utc::now());
                                    let _ = core
                                        .librarian
                                        .lock()
                                        .await
                                        .save_game_db(&post_game_id, db)
                                        .await;
                                }
                            }
                        }
                        core.emit_library().await;

                        if method == launcher::InjectionMethod::Proxy {
                            let _ = LoaderContext::uninstall_loader(
                                &game_dir,
                                Some(&prefix_path),
                                profile.use_reshade,
                                config_unsafe_mode,
                            )
                            .await;
                        }
                        *core.running_game_name.lock().await = None;
                        core.emit(AppEvent::GameStopped);
                    });
                    Ok(())
                }
                Err(e) => Err(format!("Launch failed: {}", e)),
            }
        }
        .await;
        *self.is_launching.lock().await = false;
        res
    }

    pub async fn run_graphics_audit(
        &self,
        game_id: String,
    ) -> Result<Vec<launcher::GraphicsAuditWarning>, String> {
        let (exe_path, install_path) = {
            let dbs = self.game_dbs.lock().await;
            let db = dbs.get(&game_id).ok_or("Game not found")?;
            let config = db.games.get(&game_id).ok_or("Config missing")?;
            (config.exe_path.clone(), config.install_path.clone())
        };

        let game_dir = if install_path.exists() {
            install_path
        } else {
            exe_path
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or(install_path)
        };

        Ok(launcher::audit_game_graphics(&game_id, &game_dir))
    }

    pub async fn apply_wuwa_engine_tweaks(&self, game_id: String) -> Result<(), String> {
        let install_path = {
            let dbs = self.game_dbs.lock().await;
            let db = dbs.get(&game_id).ok_or("Game not found")?;
            let config = db.games.get(&game_id).ok_or("Config missing")?;
            config.install_path.clone()
        };

        launcher::apply_wuwa_engine_tweaks(&install_path).map_err(|e| e.to_string())
    }

    pub async fn read_d3d11_diagnostics(
        &self,
        game_id: String,
    ) -> Result<Vec<mod_patches::LogDiagnostic>, String> {
        let install_path = {
            let dbs = self.game_dbs.lock().await;
            let db = dbs.get(&game_id).ok_or("Game not found")?;
            let config = db.games.get(&game_id).ok_or("Config missing")?;
            if config.install_path.exists() {
                config.install_path.clone()
            } else if let Some(parent) = config.exe_path.parent() {
                parent.to_path_buf()
            } else {
                config.install_path.clone()
            }
        };

        let log_path = install_path.join("d3d11_log.txt");
        mod_patches::parse_d3d11_log(&log_path, 500).map_err(|e| e.to_string())
    }
}
