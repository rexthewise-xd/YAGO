use crate::error::{LoaderError, Result};
use crate::spec::LoaderSpec;
use ini::{IniDocument, IniPatcher};
use std::path::Path;

pub struct LoaderContext;

#[derive(Debug, Clone, Default)]
pub struct InstallOptions {
    pub game_id: String,
    pub install_reshade: bool,
    pub injection_method: Option<String>, // "Proxy" or "Loader"
    pub unsafe_mode: bool,
    pub use_rabbit_wrapper: bool,
    pub loader_spec: Option<LoaderSpec>,
}

impl LoaderContext {
    pub async fn install_loader(
        game_dir: &Path,
        library_root: &Path,
        options: InstallOptions,
    ) -> Result<()> {
        let game_lib = library_root.join(&options.game_id);
        let method = options.injection_method.as_deref().unwrap_or("Proxy");
        let spec = options
            .loader_spec
            .clone()
            .unwrap_or_else(LoaderSpec::default_xxmi);

        if method == "ReShadeOnly" {
            // ReShade Only Strategy:
            // Install ReShade as 'dxgi.dll'.
            // We do NOT install GIMI (d3d11.dll).
            // Wine/Proton will use the 'dxgi=n,b' override to load our local dxgi.dll.

            println!("Loader: ReShadeOnly method active. Installing ReShade as dxgi.dll...");

            let common_lib = library_root.join("common");
            let reshade_source = if common_lib.join("ReShade.dll").exists() {
                common_lib.join("ReShade.dll")
            } else {
                common_lib.join("reshade.dll")
            };

            let target_dxgi = game_dir.join("dxgi.dll");

            if reshade_source.exists() {
                println!(
                    "Loader: Copying ReShade to Game Root as dxgi.dll: {:?}",
                    target_dxgi
                );
                std::fs::copy(&reshade_source, &target_dxgi).map_err(LoaderError::Io)?;
            } else {
                println!("Loader: ReShade requested but not found in common loaders.");
            }

            // Clean up d3d11.dll (GIMI) if present
            let d3d11 = game_dir.join("d3d11.dll");
            if d3d11.exists() {
                let _ = std::fs::remove_file(d3d11);
            }

            // We are done for ReShadeOnly.
            return Ok(());
        }

        // 1. Base Loader
        if method == "Proxy" {
            Self::install_proxy_with_spec(
                game_dir,
                &game_lib,
                &options.game_id,
                options.unsafe_mode,
                &spec,
            )
            .await?;
        } else if method == "Loader" {
            // In Loader mode, we don't copy the proxy DLLs to game root.
            // But we might need to ensure the loader folder itself is ready (handled by quartermaster/download_loader)
            // d3dx.ini patching for ReShade happens below.
        }

        // Determine ini path: Proxy -> GameDir/d3dx.ini, Loader -> Library/Game/d3dx.ini ?
        let cfg_file = spec.config_file.as_deref().unwrap_or("d3dx.ini");
        let ini_path = if method == "Loader" {
            game_lib.join(cfg_file)
        } else {
            game_dir.join(cfg_file)
        };

        // 2. ReShade / Chaining
        // Strategy: We install ReShade as "dxgi.dll" (Dual Proxy).
        // This allows independent loading alongside GIMI (d3d11.dll).
        let target_reshade = game_dir.join("dxgi.dll");

        if options.install_reshade {
            let common_lib = library_root.join("common");
            // Look for "ReShade.dll" or "reshade.dll" in common
            let reshade_source = if common_lib.join("ReShade.dll").exists() {
                common_lib.join("ReShade.dll")
            } else {
                common_lib.join("reshade.dll")
            };

            let rabbit_source = if common_lib.join("RabbitWrapper.dll").exists() {
                Some(common_lib.join("RabbitWrapper.dll"))
            } else if common_lib.join("rabbitwrapper.dll").exists() {
                Some(common_lib.join("rabbitwrapper.dll"))
            } else {
                None
            };

            if options.use_rabbit_wrapper && rabbit_source.is_some() {
                let wrapper = rabbit_source.unwrap();
                println!(
                    "Loader: Copying RabbitWrapper to Game Root as dxgi.dll: {:?}",
                    target_reshade
                );
                std::fs::copy(&wrapper, &target_reshade).map_err(LoaderError::Io)?;

                if reshade_source.exists() {
                    let target_reshade_dll = game_dir.join("ReShade.dll");
                    let _ = std::fs::copy(&reshade_source, target_reshade_dll);
                }
            } else if reshade_source.exists() {
                println!(
                    "Loader: Copying ReShade to Game Root as dxgi.dll: {:?}",
                    target_reshade
                );
                // Always overwrite to ensure version match
                std::fs::copy(&reshade_source, &target_reshade).map_err(LoaderError::Io)?;

                // No d3dx.ini patching required for Dual Proxy method.
            } else {
                println!("Loader: ReShade requested but not found in common loaders.");
            }
        } else {
            // ONLY remove ReShade if we are sure it's OURS or we are in a pure cleanup.
            // For safety, if install_reshade is false, we just leave whatever is there alone
            // unless we are specifically doing a full uninstall.

            // Legacy Cleanup: Check for iphlpapi.dll and ReShade.dll just in case
            for legacy in &["iphlpapi.dll", "ReShade.dll"] {
                let legacy_path = game_dir.join(legacy);
                if legacy_path.exists() {
                    let _ = std::fs::remove_file(legacy_path);
                }
            }

            // Unpatch d3dx.ini (Legacy Cleanup)
            if ini_path.exists() {
                println!("Loader: Unpatching d3dx.ini ReShade Proxy (Legacy)...");
                let compiler = ini::IniCompiler::default();
                if let Ok(mut doc) = compiler.compile(&ini_path) {
                    if let Some(sec) = doc.sections.iter_mut().find(|s| s.name == "System") {
                        sec.items.retain(|i| {
                            if let ini::IniItem::Pair { key, .. } = i {
                                key != "proxy_d3d11" && key != "proxy_dxgi"
                            } else {
                                true
                            }
                        });
                    }

                    // Cleanup [Import] sections
                    doc.sections
                        .retain(|s| s.name != "Import" && s.name != "Import.ReShade");

                    let output = compiler.serialize(&doc);
                    let _ = std::fs::write(&ini_path, output);
                }
            }
        }
        Ok(())
    }

    /// Installs the loader as a Proxy using the default XXMI / 3DMigoto spec.
    pub async fn install_proxy(
        game_dir: &Path,
        loader_source: &Path,
        game_exe: &str,
        unsafe_mode: bool,
    ) -> Result<()> {
        let spec = LoaderSpec::default_xxmi();
        Self::install_proxy_with_spec(game_dir, loader_source, game_exe, unsafe_mode, &spec).await
    }

    /// Installs the loader as a Proxy according to the given LoaderSpec.
    pub async fn install_proxy_with_spec(
        game_dir: &Path,
        loader_source: &Path,
        game_exe: &str,
        unsafe_mode: bool,
        spec: &LoaderSpec,
    ) -> Result<()> {
        // Copy proxy files specified in spec
        for file_name in &spec.proxy_files {
            let source_file = loader_source.join(file_name);
            if source_file.exists() {
                let target_file = game_dir.join(file_name);
                let is_pinned = unsafe_mode
                    && (file_name == "d3d11.dll"
                        || spec.config_file.as_deref() == Some(file_name.as_str()));
                if is_pinned && target_file.exists() {
                    println!(
                        "Proxy: Unsafe mode active. Preserving existing {:?}",
                        target_file
                    );
                } else {
                    println!("Proxy: Copying {} to {:?}", file_name, target_file);
                    std::fs::copy(&source_file, &target_file).map_err(LoaderError::Io)?;
                }
            }
        }

        // If config file is specified, patch for proxy mode
        if let Some(cfg_file) = &spec.config_file {
            let target_cfg = game_dir.join(cfg_file);
            if target_cfg.exists() {
                println!(
                    "Proxy: Patching {} (target={}, module=d3d11.dll)",
                    cfg_file, game_exe
                );
                let _ = IniDocument::patch_file(&target_cfg, "Loader", "target", game_exe);
                let primary_dll = spec
                    .proxy_files
                    .iter()
                    .find(|f| f.ends_with(".dll"))
                    .map(|s| s.as_str())
                    .unwrap_or("d3d11.dll");
                let _ = IniDocument::patch_file(&target_cfg, "Loader", "module", primary_dll);
            }
        }

        // Symlink support directories
        for dir_name in &spec.support_dirs {
            let source_dir = loader_source.join(dir_name);
            if source_dir.exists() {
                let target_dir = game_dir.join(dir_name);
                println!(
                    "Proxy: Linking support directory {} -> {:?}",
                    dir_name, target_dir
                );
                #[cfg(unix)]
                {
                    let _ = std::fs::remove_file(&target_dir);
                    let _ = std::fs::remove_dir_all(&target_dir);
                    let _ = std::os::unix::fs::symlink(&source_dir, &target_dir);
                }
                #[cfg(windows)]
                {
                    let _ = std::fs::remove_dir_all(&target_dir);
                    let _ = std::os::windows::fs::symlink_dir(&source_dir, &target_dir);
                }
            }
        }

        Ok(())
    }

    /// Uninstalls the loader files using the default XXMI / 3DMigoto spec.
    pub async fn uninstall_loader(
        game_dir: &Path,
        prefix_path: Option<&Path>,
        remove_reshade: bool,
        unsafe_mode: bool,
    ) -> Result<()> {
        let spec = LoaderSpec::default_xxmi();
        Self::uninstall_loader_with_spec(game_dir, prefix_path, remove_reshade, unsafe_mode, &spec)
            .await
    }

    /// Uninstalls the loader files driven by the given LoaderSpec.
    pub async fn uninstall_loader_with_spec(
        game_dir: &Path,
        prefix_path: Option<&Path>,
        remove_reshade: bool,
        unsafe_mode: bool,
        spec: &LoaderSpec,
    ) -> Result<()> {
        let mut files_to_remove = spec.proxy_files.clone();
        if !files_to_remove.iter().any(|f| f == "ReShade.ini") {
            files_to_remove.push("ReShade.ini".to_string());
        }

        if remove_reshade {
            if !files_to_remove.iter().any(|f| f == "dxgi.dll") {
                files_to_remove.push("dxgi.dll".to_string());
            }
            if !files_to_remove.iter().any(|f| f == "ReShade.dll") {
                files_to_remove.push("ReShade.dll".to_string());
            }
        }

        for file in files_to_remove {
            let is_pinned = unsafe_mode
                && (file == "d3d11.dll" || spec.config_file.as_deref() == Some(file.as_str()));
            if !is_pinned {
                let path = game_dir.join(file);
                if path.exists() {
                    let _ = std::fs::remove_file(&path);
                }
            }
        }

        // Cleanup Stealth DLL in Prefix (Legacy)
        if let Some(pfx) = prefix_path {
            let stealth_dll = pfx.join("pfx/drive_c/windows/system32/d3d11.dll");
            if stealth_dll.exists() {
                let _ = std::fs::remove_file(stealth_dll);
            }
        }

        // Cleanup support directories
        for dir in &spec.support_dirs {
            let path = game_dir.join(dir);
            if path.exists() {
                if path.is_dir() {
                    let _ = std::fs::remove_dir_all(&path);
                } else {
                    let _ = std::fs::remove_file(&path);
                }
            }
        }

        // Specifically handle mods_target_dir symlink separately to avoid accidental recursion
        let mods_path = game_dir.join(&spec.mods_target_dir);
        if mods_path.exists() {
            if let Ok(meta) = std::fs::symlink_metadata(&mods_path) {
                if meta.file_type().is_symlink() {
                    let _ = std::fs::remove_file(&mods_path);
                }
            }
        }

        Ok(())
    }

    /// Legacy support function
    pub async fn remove_loader(game_path: &Path) -> Result<()> {
        Self::uninstall_loader(game_path, None, true, false).await
    }
}
