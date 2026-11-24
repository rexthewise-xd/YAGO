use crate::error::{MarshalError, Result};
use std::path::PathBuf;
use tokio::process::Command;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RunnerType {
    Wine,
    Proton,
    Native,
}

#[derive(Clone)]
pub struct RunnerConfig {
    pub runner_type: RunnerType,
    pub path: PathBuf, // Path to 'wine' or 'proton' executable (empty for Native)
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum InjectionMethod {
    None,
    Proxy,        // d3d11.dll replacement
    Loader,       // Legacy/Direct loader exec (keep for compat)
    RemoteThread, // CreateRemoteThread (Windows only)
    ManualMap,    // Stealth mapping (Advanced/Experimental)
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum StartMethod {
    #[default]
    Native,
    Shell,
    Steam,
    Manual,
}

fn is_in_path(binary: &str) -> bool {
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join(binary);
            if candidate.is_file() {
                return true;
            }
        }
    }
    false
}

#[derive(Clone)]
pub struct LaunchOptions {
    pub exe_path: PathBuf,
    pub args: Vec<String>,
    pub current_dir: Option<PathBuf>,
    pub runner: RunnerConfig,
    pub prefix_path: PathBuf, // WINEPREFIX or STEAM_COMPAT_DATA_PATH
    pub use_gamescope: bool,
    pub gamescope_args: Vec<String>,
    pub use_gamemode: bool,
    pub use_mangohud: bool,
    pub injection_method: InjectionMethod,
    pub modloader_enabled: bool,
    pub loader_path: Option<PathBuf>, // Directory for Proxy source
    pub injected_dlls: Vec<PathBuf>,  // Specific DLLs for RemoteThread
    pub resolution: (u32, u32),
    pub fps_target: Option<u32>,
    pub sandbox_config: Option<crate::SandboxConfig>,
    pub sandbox_data_dir: Option<PathBuf>,
    pub enable_linux_shield: bool,
    pub shield_path: Option<PathBuf>,
    pub helper_path: Option<PathBuf>,
    pub start_method: StartMethod,
    pub use_umu: bool,
    pub umu_game_id: Option<String>,
    pub proton_wayland: bool,
    pub dxvk_low_latency: bool,
    pub obs_vkcapture: bool,
    pub lsfg_vk: bool,
    pub pre_launch_cmd: Option<String>,
    pub post_exit_cmd: Option<String>,
    pub dll_overrides: Option<String>,
}

use crate::SandboxManager;

pub struct Launcher;

impl Launcher {
    pub async fn launch(&self, options: LaunchOptions) -> Result<()> {
        // Strategy Execution: Pre-Launch
        if options.injection_method == InjectionMethod::RemoteThread {
            #[cfg(target_os = "windows")]
            {} // OK
            #[cfg(not(target_os = "windows"))]
            {
                // Linux RemoteThread via helper?
                // If helper is missing, fail.
                if options.helper_path.is_none() {
                    return Err(MarshalError::Io(std::io::Error::new(
                        std::io::ErrorKind::Unsupported,
                        "Remote Injection on Linux requires win_helper.exe",
                    )));
                }
            }
        }

        // Pre-launch command
        if let Some(pre) = &options.pre_launch_cmd {
            let pre_trimmed = pre.trim();
            if !pre_trimmed.is_empty() {
                println!("Launcher: Executing pre-launch command: {}", pre_trimmed);
                #[cfg(windows)]
                let _ = std::process::Command::new("cmd")
                    .args(["/C", pre_trimmed])
                    .status();
                #[cfg(not(windows))]
                let _ = std::process::Command::new("sh")
                    .args(["-c", pre_trimmed])
                    .status();
            }
        }

        // 1. Ensure Prefix is Primed (Linux only)
        #[cfg(unix)]
        {
            if !options.prefix_path.as_os_str().is_empty()
                && !matches!(options.runner.runner_type, RunnerType::Native)
            {
                self.prepare_prefix(&options).await?;
            }
        }

        // Windows Hook Setup (Loader Method)
        #[cfg(windows)]
        let _loader_hook = if options.injection_method == InjectionMethod::Loader {
            if let Some(loader_path) = &options.loader_path {
                let dll_path = loader_path.join("3dmloader.dll");
                if dll_path.exists() {
                    println!("Marshal: Initializing 3dmloader hook from {:?}", dll_path);
                    match crate::windows::LoaderHook::load(&dll_path) {
                        Ok(hook) => {
                            let exe_name = options
                                .exe_path
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy();
                            if let Err(e) = hook.set_hook(&exe_name) {
                                eprintln!("Marshal: Failed to set hook: {}", e);
                                None
                            } else {
                                Some(hook)
                            }
                        }
                        Err(e) => {
                            eprintln!("Marshal: Failed to load 3dmloader.dll: {}", e);
                            None
                        }
                    }
                } else {
                    eprintln!("Marshal: 3dmloader.dll not found at {:?}", dll_path);
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        // 2. Active Sandbox: Restore
        if let (Some(game_dir), Some(profile_data_dir), Some(sandbox_config)) = (
            &options.current_dir,
            &options.sandbox_data_dir,
            &options.sandbox_config,
        ) {
            println!("Sandbox: Restoring active profile data...");
            if let Err(e) = SandboxManager::restore(
                game_dir,
                profile_data_dir,
                sandbox_config,
                &options.runner,
                &options.prefix_path,
            ) {
                eprintln!("Sandbox Restore Error: {}", e);
            }
        }

        // Linux Hook (Loader Method via Helper)
        #[cfg(unix)]
        if options.injection_method == InjectionMethod::Loader {
            if let (Some(helper), Some(loader_path)) = (&options.helper_path, &options.loader_path)
            {
                if helper.exists() {
                    let dll_path = loader_path.join("3dmloader.dll");
                    let exe_name = options
                        .exe_path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy();

                    println!("Marshal: Spawning win_helper for Hook mode...");

                    let dll_win_path =
                        format!("Z:{}", dll_path.to_string_lossy().replace('/', "\\"));

                    let mut h_cmd =
                        self.build_runner_command(&options.runner, &options.prefix_path);
                    h_cmd
                        .arg(helper)
                        .arg("hook")
                        .arg(exe_name.to_string())
                        .arg(dll_win_path);

                    // Set working directory to loader path so dependencies like d3dcompiler are found
                    h_cmd.current_dir(loader_path);

                    h_cmd.stdout(std::process::Stdio::inherit());
                    h_cmd.stderr(std::process::Stdio::inherit());
                    h_cmd.kill_on_drop(true);

                    match h_cmd.spawn() {
                        Ok(_) => {
                            println!("Marshal: Helper spawned successfully. Waiting for hook initialization...");
                            // Increase delay to 5 seconds for slow Proton prefixes
                            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                        }
                        Err(e) => eprintln!("Marshal: Failed to spawn helper: {}", e),
                    }
                } else {
                    eprintln!("Marshal: win_helper.exe not found at {:?}", helper);
                }
            }
        }

        let mut cmd = self.build_command(&options)?;
        cmd.kill_on_drop(true);

        // Windows Remote Thread: Spawn Suspended
        #[cfg(windows)]
        if options.injection_method == InjectionMethod::RemoteThread {
            const CREATE_SUSPENDED: u32 = 0x00000004;
            cmd.creation_flags(CREATE_SUSPENDED);
        }

        println!("Marshal: Launching game process...");
        let mut child = cmd.spawn().map_err(MarshalError::Io)?;

        // Strategy Execution: Post-Launch (Windows Only)
        #[cfg(windows)]
        if options.injection_method == InjectionMethod::RemoteThread {
            let pid = child.id().ok_or(MarshalError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                "No PID",
            )))?;

            for dll in &options.injected_dlls {
                crate::windows::inject_remote(pid, dll)?;
            }

            crate::windows::resume_process(pid)?;
        }

        // Windows Hook: Wait for Injection
        #[cfg(windows)]
        if let Some(hook) = &_loader_hook {
            println!("Marshal: Waiting for injection via hook...");
            if let Err(e) = hook.wait_for_injection() {
                eprintln!("Marshal: WaitForInjection failed: {}", e);
            } else {
                println!("Marshal: Injection sequence completed.");
            }
        }

        // Post-Launch Monitor (Lifecycle Management)
        let game_dir = options.current_dir.clone();

        let sandbox_data_dir = options.sandbox_data_dir.clone();
        let sandbox_config = options.sandbox_config.clone();
        let runner = RunnerConfig {
            runner_type: options.runner.runner_type,
            path: options.runner.path.clone(),
        };
        let prefix_path = options.prefix_path.clone();
        let method = options.injection_method;
        let post_exit_cmd = options.post_exit_cmd.clone();

        tokio::spawn(async move {
            let _ = child.wait().await;

            // Execute post-exit command if configured
            if let Some(post) = post_exit_cmd {
                let post_trimmed = post.trim();
                if !post_trimmed.is_empty() {
                    println!("Launcher: Executing post-exit command: {}", post_trimmed);
                    #[cfg(windows)]
                    let _ = tokio::process::Command::new("cmd")
                        .args(["/C", &post_trimmed])
                        .status()
                        .await;
                    #[cfg(not(windows))]
                    let _ = tokio::process::Command::new("sh")
                        .args(["-c", &post_trimmed])
                        .status()
                        .await;
                }
            }

            // 1. Active Sandbox: Snapshot
            if let (Some(dir), Some(profile_dir), Some(config)) =
                (&game_dir, sandbox_data_dir, sandbox_config)
            {
                println!("Sandbox: Saving session snapshot...");
                if let Err(e) =
                    SandboxManager::snapshot(dir, &profile_dir, &config, &runner, &prefix_path)
                {
                    eprintln!("Sandbox Snapshot Error: {}", e);
                }
            }

            // CLEANUP: ReShade.dll (If Loader mode, we copied it to game root)
            if method == InjectionMethod::Loader {
                if let Some(dir) = &game_dir {
                    let reshade = dir.join("ReShade.dll");
                    if reshade.exists() {
                        println!("Marshal: Cleaning up ReShade.dll from game root.");
                        let _ = std::fs::remove_file(reshade);
                    }
                }
            }
        });

        Ok(())
    }

    #[cfg(unix)]
    pub async fn prepare_prefix(&self, options: &LaunchOptions) -> Result<()> {
        let pfx_dir = options.prefix_path.join("pfx");
        let drive_c = pfx_dir.join("drive_c");

        if !drive_c.exists() {
            println!(
                "Prefix at {:?} is not initialized. Priming...",
                options.prefix_path
            );

            // Skip priming if runner path doesn't exist (e.g. in integration tests)
            if !options.runner.path.exists()
                && !matches!(options.runner.runner_type, RunnerType::Native)
            {
                // Check if it's just 'wine' in PATH
                if options.runner.path.to_string_lossy() != "wine" {
                    println!(
                        "Runner path {:?} not found. Skipping prefix initialization.",
                        options.runner.path
                    );
                    return Ok(());
                }
            }

            // Ensure structure
            if !pfx_dir.exists() {
                std::fs::create_dir_all(&pfx_dir).map_err(MarshalError::Io)?;
            }

            let mut cmd = if matches!(options.runner.runner_type, RunnerType::Proton) {
                let mut c = Command::new(&options.runner.path);
                c.arg("run").arg("wineboot").arg("-u");
                c.env("STEAM_COMPAT_DATA_PATH", &options.prefix_path);
                if let Some(parent) = options.runner.path.parent() {
                    c.env("STEAM_COMPAT_CLIENT_INSTALL_PATH", parent);
                }
                c
            } else {
                // For Wine, WINEPREFIX must point to the folder containing drive_c
                let mut c = Command::new(&options.runner.path);
                c.arg("wineboot").arg("-u");
                c.env("WINEPREFIX", &pfx_dir);
                c
            };

            // Suppress GUI dialogs during init
            cmd.env("WINEDEBUG", "-all");
            // CRITICAL: Disable LD_PRELOAD (Shield) during prefix init/upgrade
            // Blocking unlinks during upgrade causes Proton to fail.
            cmd.env_remove("LD_PRELOAD");

            let mut child = cmd.spawn().map_err(MarshalError::Io)?;

            // Wait for initialization to complete
            let status = child.wait().await.map_err(MarshalError::Io)?;

            if !status.success() {
                println!(
                    "Prefix initialization returned non-zero status: {:?}",
                    status
                );
            }
        }
        Ok(())
    }

    fn build_runner_command(&self, runner: &RunnerConfig, prefix: &PathBuf) -> Command {
        match runner.runner_type {
            RunnerType::Proton => {
                let mut c = Command::new(&runner.path);
                c.arg("run");
                c.env("STEAM_COMPAT_DATA_PATH", prefix);
                if let Some(parent) = runner.path.parent() {
                    c.env("STEAM_COMPAT_CLIENT_INSTALL_PATH", parent);
                }
                c.env("WINEDEBUG", "-all");
                c
            }
            RunnerType::Wine => {
                let mut c = Command::new(&runner.path);
                // For Wine, WINEPREFIX must point to the actual prefix folder (pfx)
                let pfx_dir = prefix.join("pfx");
                c.env("WINEPREFIX", pfx_dir);
                c.env("WINEDEBUG", "-all");
                c
            }
            RunnerType::Native => Command::new("true"),
        }
    }

    pub fn build_command(&self, options: &LaunchOptions) -> Result<Command> {
        #[cfg(unix)]
        {
            let umu_in_path = options.use_umu && is_in_path("umu-run");

            // ... (outermost wrapper logic)
            let base_cmd = if options.use_gamemode {
                "gamemoderun".to_string()
            } else if options.use_gamescope {
                "gamescope".to_string()
            } else if umu_in_path {
                "umu-run".to_string()
            } else if matches!(options.runner.runner_type, RunnerType::Native) {
                options.exe_path.to_string_lossy().to_string()
            } else {
                options.runner.path.to_string_lossy().to_string()
            };

            let mut cmd = Command::new(&base_cmd);

            // ... (gamescope/mangohud logic)
            if options.use_gamemode && options.use_gamescope {
                cmd.arg("gamescope");
            }

            if options.use_gamescope {
                let (w, h) = if options.resolution.0 == 0 || options.resolution.1 == 0 {
                    (1920, 1080)
                } else {
                    options.resolution
                };

                cmd.arg("-W")
                    .arg(w.to_string())
                    .arg("-H")
                    .arg(h.to_string())
                    .arg("-w")
                    .arg(w.to_string())
                    .arg("-h")
                    .arg(h.to_string())
                    .arg("-f");

                // Add custom gamescope arguments
                if !options.gamescope_args.is_empty() {
                    cmd.args(&options.gamescope_args);
                }

                cmd.arg("--");
            }

            if (options.use_gamemode || options.use_gamescope) && umu_in_path {
                cmd.arg("umu-run");
            }

            if options.use_mangohud {
                cmd.env("MANGOHUD", "1");
            }

            // Linux profile runtime flags
            if options.proton_wayland {
                cmd.env("PROTON_ENABLE_WAYLAND", "1");
            }
            if options.dxvk_low_latency {
                cmd.env("PROTON_DXVK_LOWLATENCY", "1");
            }
            if options.obs_vkcapture {
                cmd.env("OBS_VKCAPTURE", "1");
            }
            if options.lsfg_vk {
                cmd.env("ENABLE_LSFG_VK", "1");
                cmd.env("LSFG_PROCESS", "1");
            }

            // injection overrides
            if !matches!(options.runner.runner_type, RunnerType::Native) {
                match options.injection_method {
                    InjectionMethod::Proxy | InjectionMethod::Loader => {
                        let ov = options
                            .dll_overrides
                            .as_deref()
                            .unwrap_or("d3d11,dxgi,d3dcompiler_47=n,b");
                        cmd.env("WINEDLLOVERRIDES", ov);
                    }
                    _ => {}
                }
            }

            if umu_in_path {
                let game_id_str = options.umu_game_id.as_deref().unwrap_or("0");
                cmd.env("GAMEID", game_id_str);
                cmd.env("PROTONPATH", &options.runner.path);
                if !options.prefix_path.as_os_str().is_empty() {
                    let pfx_dir = options.prefix_path.join("pfx");
                    cmd.env("WINEPREFIX", pfx_dir);
                }
                cmd.env("WINEDEBUG", "-all");
                cmd.env("WINE_LARGE_ADDRESS_AWARE", "1");

                if options.modloader_enabled && options.enable_linux_shield {
                    if let Some(shield_path) = &options.shield_path {
                        if shield_path.exists() {
                            cmd.env("LD_PRELOAD", shield_path);
                            if let Some(dir) = &options.current_dir {
                                cmd.env("YAGO_PROTECT_PATH", dir);
                            }
                        }
                    }
                }

                cmd.arg(&options.exe_path);
            } else if !matches!(options.runner.runner_type, RunnerType::Native) {
                if base_cmd != options.runner.path.to_string_lossy() {
                    cmd.arg(&options.runner.path);
                }

                cmd.env("WINEDEBUG", "-all");

                if options.modloader_enabled && options.enable_linux_shield {
                    if let Some(shield_path) = &options.shield_path {
                        if shield_path.exists() {
                            println!(
                                "Sandbox: Injecting Integrity Shield via LD_PRELOAD: {:?}",
                                shield_path
                            );
                            cmd.env("LD_PRELOAD", shield_path);

                            if let Some(dir) = &options.current_dir {
                                println!("Sandbox: Shield protecting path: {:?}", dir);
                                cmd.env("YAGO_PROTECT_PATH", dir);
                            }
                        }
                    }
                }

                // Optimization: Address huge address space allocations in HoYo games
                cmd.env("WINE_LARGE_ADDRESS_AWARE", "1");

                match options.runner.runner_type {
                    RunnerType::Wine => {
                        if !options.prefix_path.as_os_str().is_empty() {
                            let pfx_dir = options.prefix_path.join("pfx");
                            println!("Marshal: Setting WINEPREFIX={:?}", pfx_dir);
                            cmd.env("WINEPREFIX", pfx_dir);
                        }
                    }
                    RunnerType::Proton => {
                        if !options.prefix_path.as_os_str().is_empty() {
                            println!(
                                "Marshal: Setting STEAM_COMPAT_DATA_PATH={:?}",
                                options.prefix_path
                            );
                            cmd.env("STEAM_COMPAT_DATA_PATH", &options.prefix_path);
                        }
                        if let Some(parent) = options.runner.path.parent() {
                            cmd.env("STEAM_COMPAT_CLIENT_INSTALL_PATH", parent);
                        }
                        // Set SteamAppId and STEAM_COMPAT_APP_ID to help protonfixes identify the game
                        let app_id = "999680";
                        cmd.env("SteamAppId", app_id);
                        cmd.env("STEAM_COMPAT_APP_ID", app_id);
                        cmd.arg("run");
                    }
                    RunnerType::Native => {}
                }

                cmd.arg(&options.exe_path);
            }

            if let Some(dir) = &options.current_dir {
                cmd.current_dir(dir);
            }

            cmd.args(&options.args);

            println!("Executing command: {:?}", cmd);
            Ok(cmd)
        }

        #[cfg(windows)]
        {
            let mut cmd = if options.start_method == StartMethod::Shell {
                let mut c = Command::new("cmd");
                c.arg("/C").arg("start").arg("").arg("/D");
                if let Some(dir) = &options.current_dir {
                    c.arg(dir);
                } else {
                    c.arg(".");
                }
                c.arg(&options.exe_path);
                c
            } else {
                let mut c = Command::new(&options.exe_path);
                if let Some(dir) = &options.current_dir {
                    c.current_dir(dir);
                }
                c
            };
            cmd.args(&options.args);
            Ok(cmd)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_command_env_vars() {
        let options = LaunchOptions {
            exe_path: PathBuf::from("/game/GenshinImpact.exe"),
            args: vec![],
            current_dir: Some(PathBuf::from("/game")),
            runner: RunnerConfig {
                runner_type: RunnerType::Proton,
                path: PathBuf::from("/runner/proton"),
            },
            prefix_path: PathBuf::from("/prefix"),
            use_gamescope: false,
            gamescope_args: vec![],
            use_gamemode: false,
            use_mangohud: false,
            injection_method: InjectionMethod::Proxy,
            modloader_enabled: true,
            loader_path: None,
            injected_dlls: vec![],
            resolution: (1920, 1080),
            fps_target: None,
            sandbox_config: None,
            sandbox_data_dir: None,
            enable_linux_shield: false,
            shield_path: None,
            helper_path: None,
            start_method: StartMethod::Native,
            use_umu: false,
            umu_game_id: Some("umu-genshin".to_string()),
            proton_wayland: true,
            dxvk_low_latency: true,
            obs_vkcapture: true,
            lsfg_vk: true,
            pre_launch_cmd: None,
            post_exit_cmd: None,
            dll_overrides: None,
        };

        let launcher = Launcher;
        let cmd = launcher.build_command(&options).unwrap();
        #[cfg(unix)]
        {
            let envs: Vec<_> = cmd.as_std().get_envs().collect();
            let find_env = |key: &str| {
                envs.iter()
                    .find(|(k, _)| k.to_str() == Some(key))
                    .and_then(|(_, v)| v.as_ref().and_then(|v| v.to_str()))
            };
            assert_eq!(find_env("PROTON_ENABLE_WAYLAND"), Some("1"));
            assert_eq!(find_env("PROTON_DXVK_LOWLATENCY"), Some("1"));
            assert_eq!(find_env("OBS_VKCAPTURE"), Some("1"));
            assert_eq!(find_env("ENABLE_LSFG_VK"), Some("1"));
            assert_eq!(
                find_env("WINEDLLOVERRIDES"),
                Some("d3d11,dxgi,d3dcompiler_47=n,b")
            );
        }
    }
}
