use crate::{MarshalError, Result, RunnerConfig, RunnerType};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct SandboxConfig {
    pub registry_keys: Vec<String>,
    pub files: Vec<String>,
}

pub struct SandboxManager;

impl SandboxManager {
    /// Restores sandboxed data from profile_dir to game_dir
    pub fn restore(
        game_dir: &Path,
        profile_data_dir: &Path,
        config: &SandboxConfig,
        runner: &RunnerConfig,
        prefix_path: &Path,
    ) -> Result<()> {
        // 1. Restore Files
        for rel_path in &config.files {
            let source = profile_data_dir.join(rel_path);
            let dest = game_dir.join(rel_path);

            if source.exists() {
                if let Some(parent) = dest.parent() {
                    fs::create_dir_all(parent).map_err(MarshalError::Io)?;
                }
                fs::copy(&source, &dest).map_err(MarshalError::Io)?;
                println!("Sandbox: Restored file {:?}", rel_path);
            }
        }

        // 2. Restore Registry
        let reg_file = profile_data_dir.join("registry.reg");
        if reg_file.exists() {
            Self::import_registry(&reg_file, runner, prefix_path)?;
            println!("Sandbox: Restored registry from {:?}", reg_file);
        }

        Ok(())
    }

    /// Snapshots current game state back to profile_dir
    pub fn snapshot(
        game_dir: &Path,
        profile_data_dir: &Path,
        config: &SandboxConfig,
        runner: &RunnerConfig,
        prefix_path: &Path,
    ) -> Result<()> {
        if !profile_data_dir.exists() {
            fs::create_dir_all(profile_data_dir).map_err(MarshalError::Io)?;
        }

        // 1. Snapshot Files
        for rel_path in &config.files {
            let source = game_dir.join(rel_path);
            let dest = profile_data_dir.join(rel_path);

            if source.exists() {
                if let Some(parent) = dest.parent() {
                    fs::create_dir_all(parent).map_err(MarshalError::Io)?;
                }
                fs::copy(&source, &dest).map_err(MarshalError::Io)?;
                println!("Sandbox: Snapshotted file {:?}", rel_path);
            }
        }

        // 2. Snapshot Registry
        if !config.registry_keys.is_empty() {
            let reg_file = profile_data_dir.join("registry.reg");
            Self::export_registry(&reg_file, &config.registry_keys, runner, prefix_path)?;
            println!("Sandbox: Snapshotted registry to {:?}", reg_file);
        }

        Ok(())
    }

    fn import_registry(reg_file: &Path, runner: &RunnerConfig, prefix_path: &Path) -> Result<()> {
        match runner.runner_type {
            RunnerType::Native => {
                #[cfg(target_os = "windows")]
                {
                    let status = Command::new("reg")
                        .args(["import", reg_file.to_str().unwrap_or_default()])
                        .status()
                        .map_err(MarshalError::Io)?;
                    if !status.success() {
                        return Err(MarshalError::RegistryError(format!(
                            "Reg import failed with code {:?}",
                            status.code()
                        )));
                    }
                }
            }
            RunnerType::Proton | RunnerType::Wine => {
                let wine_bin = if runner.runner_type == RunnerType::Proton {
                    runner.path.parent().unwrap().join("bin/wine")
                } else {
                    PathBuf::from("wine")
                };

                let status = Command::new(&wine_bin)
                    .env("WINEPREFIX", prefix_path.join("pfx"))
                    .args(["regedit", "/s", reg_file.to_str().unwrap_or_default()])
                    .status()
                    .map_err(MarshalError::Io)?;

                if !status.success() {
                    return Err(MarshalError::RegistryError(
                        "Wine regedit import failed".to_string(),
                    ));
                }
            }
        }
        Ok(())
    }

    /// Merges multiple .reg export parts into a single valid .reg document.
    /// Preserves the Windows Registry Editor Version 5.00 header from the first part
    /// and strips duplicate headers from subsequent parts.
    pub fn merge_reg_parts(parts: &[&str]) -> String {
        if parts.is_empty() {
            return String::new();
        }

        let mut result = String::new();

        for (i, part) in parts.iter().enumerate() {
            let trimmed = part.trim_start_matches('\u{feff}');
            if i == 0 {
                let content = trimmed.trim_end();
                if !content.is_empty() {
                    result.push_str(content);
                    result.push_str("\r\n\r\n");
                }
            } else {
                let mut skipping_header = true;
                let mut part_lines = Vec::new();

                for line in trimmed.lines() {
                    let trimmed_line = line.trim();
                    if skipping_header {
                        if trimmed_line.eq_ignore_ascii_case("Windows Registry Editor Version 5.00")
                            || trimmed_line.is_empty()
                        {
                            continue;
                        }
                        skipping_header = false;
                    }
                    part_lines.push(line);
                }

                if !part_lines.is_empty() {
                    result.push_str(&part_lines.join("\r\n"));
                    result.push_str("\r\n\r\n");
                }
            }
        }

        let trimmed = result.trim_end();
        if trimmed.is_empty() {
            String::new()
        } else {
            format!("{}\r\n", trimmed)
        }
    }

    fn export_registry(
        reg_file: &Path,
        keys: &[String],
        runner: &RunnerConfig,
        prefix_path: &Path,
    ) -> Result<()> {
        let mut temp_files = Vec::new();
        let mut part_contents = Vec::new();

        for (i, key) in keys.iter().enumerate() {
            let temp_reg = reg_file.with_extension(format!("part{}.reg", i));

            match runner.runner_type {
                RunnerType::Native => {
                    #[cfg(target_os = "windows")]
                    {
                        Command::new("reg")
                            .args(["export", key, temp_reg.to_str().unwrap_or_default(), "/y"])
                            .status()
                            .map_err(MarshalError::Io)?;
                    }
                }
                RunnerType::Proton | RunnerType::Wine => {
                    let wine_bin = if runner.runner_type == RunnerType::Proton {
                        runner.path.parent().unwrap().join("bin/wine")
                    } else {
                        PathBuf::from("wine")
                    };

                    Command::new(&wine_bin)
                        .env("WINEPREFIX", prefix_path.join("pfx"))
                        .args(["regedit", "/e", temp_reg.to_str().unwrap_or_default(), key])
                        .status()
                        .map_err(MarshalError::Io)?;
                }
            }

            if temp_reg.exists() {
                // Read content, supporting UTF-8 and UTF-16LE fallback
                let content = fs::read_to_string(&temp_reg).or_else(|_| {
                    let bytes = fs::read(&temp_reg)?;
                    if bytes.len() >= 2 && bytes.len() % 2 == 0 {
                        let u16s: Vec<u16> = bytes
                            .as_chunks::<2>()
                            .0
                            .iter()
                            .map(|&[b0, b1]| u16::from_le_bytes([b0, b1]))
                            .collect();
                        String::from_utf16(&u16s)
                            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
                    } else {
                        Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "Invalid registry encoding",
                        ))
                    }
                });

                if let Ok(c) = content {
                    part_contents.push(c);
                }
                temp_files.push(temp_reg);
            }
        }

        let part_refs: Vec<&str> = part_contents.iter().map(|s| s.as_str()).collect();
        let merged = Self::merge_reg_parts(&part_refs);
        fs::write(reg_file, merged.as_bytes()).map_err(MarshalError::Io)?;

        for temp_reg in temp_files {
            let _ = fs::remove_file(temp_reg);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_reg_parts_multi_key() {
        let part1 = "Windows Registry Editor Version 5.00\r\n\r\n[HKEY_CURRENT_USER\\Software\\miHoYo\\Genshin Impact]\r\n\"GraphicsQuality\"=dword:00000003\r\n";
        let part2 = "Windows Registry Editor Version 5.00\r\n\r\n[HKEY_CURRENT_USER\\Software\\miHoYo\\Common]\r\n\"UserAgreement\"=dword:00000001\r\n";

        let merged = SandboxManager::merge_reg_parts(&[part1, part2]);

        // Header must appear exactly once
        assert_eq!(
            merged
                .matches("Windows Registry Editor Version 5.00")
                .count(),
            1,
            "Header must appear exactly once"
        );
        // Both keys and their values must be present
        assert!(merged.contains("[HKEY_CURRENT_USER\\Software\\miHoYo\\Genshin Impact]"));
        assert!(merged.contains("\"GraphicsQuality\"=dword:00000003"));
        assert!(merged.contains("[HKEY_CURRENT_USER\\Software\\miHoYo\\Common]"));
        assert!(merged.contains("\"UserAgreement\"=dword:00000001"));
    }
}
