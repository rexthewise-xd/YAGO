pub mod archive;
pub mod error;
pub mod inspector;
pub mod safety;
pub mod transcoder;
pub mod vfs;

pub use archive::{
    extract_and_sanitize, extract_raw_archive, extract_targz, is_allowed, sanitize_path,
    ExtractionReport,
};
pub use error::{FsError, Result};
pub use inspector::ExeInspector;
pub use safety::Safety;
pub use transcoder::Transcoder;
pub use vfs::Vfs;

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DeploymentPlan {
    // Map<SourcePath, RelativeTargetPath>
    // SourcePath: Absolute path to the file/folder in the mod storage
    // RelativeTargetPath: Path relative to the game's "Mods/YAGO" directory
    pub symlink_map: Vec<(PathBuf, PathBuf)>,
    // Generated file content (e.g., d3dx.ini, merged.ini)
    // Path is relative to the game's "Mods" directory (or specific target)
    pub generated_files: Vec<(PathBuf, String)>,
    // Unique hash representing the state of this plan
    pub state_hash: String,
}

impl DeploymentPlan {
    pub fn validate(&self) -> Result<()> {
        for (source, relative_target) in &self.symlink_map {
            // Check for circular symlinks:
            // A common "Zip Slip" or "Cycle" issue in symlinking is pointing back to a parent.
            // Here we check if the source is an ancestor of the target, or vice versa,
            // which might happen if relative paths are incorrectly calculated.

            // In the context of YAGO, the 'source' is typically an absolute path to a mod file,
            // and the 'relative_target' is a path within the game's Mods/YAGO directory.

            // Basic cycle check: if target is inside source (e.g. symlinking a folder into itself)
            if relative_target.starts_with(source) || source.starts_with(relative_target) {
                // This is a loose check but catches the most obvious recursive loops
                return Err(FsError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!(
                        "Potential circular symlink detected: {:?} -> {:?}",
                        source, relative_target
                    ),
                )));
            }
        }
        Ok(())
    }
}

pub fn execute_deployment(
    target_root: &std::path::Path,
    plan: &DeploymentPlan,
    mods_folder_name: Option<&str>,
    force: bool,
) -> Result<()> {
    // 0. Safety Check
    plan.validate()?;

    if !target_root.exists() {
        return Err(FsError::NotFound(target_root.to_path_buf()));
    }

    let folder_name = mods_folder_name.unwrap_or("Mods");
    let mods_dir = target_root.join(folder_name);

    // 1. Ensure Mods directory exists
    if !mods_dir.exists() {
        std::fs::create_dir_all(&mods_dir).map_err(FsError::Io)?;
    }

    // 2. Check Lock File
    let lock_file = mods_dir.join("deployment.lock");
    if !force && lock_file.exists() {
        if let Ok(last_hash) = std::fs::read_to_string(&lock_file) {
            if last_hash == plan.state_hash {
                let all_generated_exist = plan
                    .generated_files
                    .iter()
                    .all(|(rel_path, _)| mods_dir.join(rel_path).exists());

                if all_generated_exist {
                    println!("Deployment state matches. Skipping VFS rebuild.");
                    return Ok(());
                }
            }
        }
    }

    // 3. Clean Target Domains
    // We must clean all managed folders to ensure no ghosts remain.
    let managed_domains = [
        "YAGO",
        "00_Configs",
        "11_GlobalUI",
        "22_Characters",
        "33_Overrides",
        "99_Unknown",
    ];

    for domain in managed_domains {
        let domain_dir = mods_dir.join(domain);
        if domain_dir.exists() {
            // We must be careful not to follow symlinks when deleting!
            // Standard remove_dir_all is fine for the YAGO managed dir as long as it doesn't follow symlinks (it usually doesn't on modern OS/Rust)
            std::fs::remove_dir_all(&domain_dir).map_err(FsError::Io)?;
        }
    }

    // Use a closure to wrap steps that can fail, allowing for rollback
    let result: Result<()> = (|| {
        // 4. Create Symlinks
        for (source, relative_target) in &plan.symlink_map {
            let target = mods_dir.join(relative_target);

            // Ensure parent directory exists and is a directory
            if let Some(parent) = target.parent() {
                if parent.exists() {
                    if !parent.is_dir() {
                        return Err(FsError::Io(std::io::Error::other(format!(
                            "Parent component is a file, not a directory: {:?}",
                            parent
                        ))));
                    }
                } else {
                    std::fs::create_dir_all(parent).map_err(FsError::Io)?;
                }
            }

            make_symlink(source, &target)?;
        }

        // 5. Write Generated Files
        for (relative_path, content) in &plan.generated_files {
            let target = mods_dir.join(relative_path);

            // Ensure parent directory for generated file exists
            if let Some(parent) = target.parent() {
                if !parent.exists() {
                    std::fs::create_dir_all(parent).map_err(FsError::Io)?;
                }
            }

            std::fs::write(&target, content).map_err(FsError::Io)?;
        }
        Ok(())
    })();

    if let Err(e) = result {
        println!("Deployment failed: {}. Rolling back...", e);
        // ROLLBACK: Wipe all managed directories
        for domain in managed_domains {
            let domain_dir = mods_dir.join(domain);
            if domain_dir.exists() {
                let _ = std::fs::remove_dir_all(&domain_dir);
            }
        }
        return Err(e);
    }

    // 6. Update Lock File
    std::fs::write(&lock_file, &plan.state_hash).map_err(FsError::Io)?;

    Ok(())
}

/// Normalizes a path, ensuring absolute paths on Windows use the long path prefix (\\?\)
pub fn normalize_path(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        if path.is_absolute() {
            let path_str = path.to_string_lossy();
            if !path_str.starts_with(r"\\?\") {
                return PathBuf::from(format!(r"\\?\{}", path_str.replace('/', "\\")));
            }
        }
    }
    path.to_path_buf()
}

#[cfg(unix)]
pub fn make_symlink(original: &std::path::Path, link: &std::path::Path) -> Result<()> {
    // If anything exists at 'link' (even a broken symlink), remove it.
    // 'exists()' follows symlinks and returns false for broken ones.
    if link.symlink_metadata().is_ok() {
        let _ = std::fs::remove_file(link);
    }
    std::os::unix::fs::symlink(original, link).map_err(FsError::Io)?;
    Ok(())
}

#[cfg(windows)]
pub fn make_symlink(original: &std::path::Path, link: &std::path::Path) -> Result<()> {
    // If link already exists as a junction/symlink or directory, remove it safely first.
    if let Ok(meta) = link.symlink_metadata() {
        if meta.file_type().is_symlink() || junction::exists(link).unwrap_or(false) {
            let _ = junction::delete(link)
                .or_else(|_| std::fs::remove_dir(link))
                .or_else(|_| std::fs::remove_file(link));
        } else if meta.is_dir() {
            let _ = std::fs::remove_dir_all(link);
        } else {
            let _ = std::fs::remove_file(link);
        }
    }

    if let Some(parent) = link.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent).map_err(FsError::Io)?;
        }
    }

    if original.is_dir() {
        if let Err(_e) = junction::create(original, link) {
            Safety::copy_recursive_sync(original, link)?;
        }
    } else {
        if let Err(_e) = std::os::windows::fs::symlink_file(original, link) {
            std::fs::copy(original, link).map_err(FsError::Io)?;
        }
    }
    Ok(())
}
mod snapshots_test;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_execute_deployment_cache_hit_and_miss() {
        let dir = tempdir().unwrap();
        let target_root = dir.path().to_path_buf();

        let source_dir = dir.path().join("source_mod");
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::write(source_dir.join("texture.dds"), b"dds").unwrap();

        let merged_rel_path = PathBuf::from("22_Characters/Keqing/merged.ini");
        let plan = DeploymentPlan {
            symlink_map: vec![(
                source_dir.clone(),
                PathBuf::from("22_Characters/Keqing/Skin_0"),
            )],
            generated_files: vec![(
                merged_rel_path.clone(),
                "[KeyCycleKeqing]\nkey = ]\n".to_string(),
            )],
            state_hash: "hash_12345".to_string(),
        };

        // 1. Initial deployment
        execute_deployment(&target_root, &plan, Some("Mods"), false)
            .expect("Initial deployment must succeed");

        let mods_dir = target_root.join("Mods");
        let lock_file = mods_dir.join("deployment.lock");
        let merged_file = mods_dir.join(&merged_rel_path);

        assert!(lock_file.exists());
        assert!(merged_file.exists());
        assert_eq!(std::fs::read_to_string(&lock_file).unwrap(), "hash_12345");

        // 2. Cache Hit: call execute_deployment again with force = false
        // Write a canary file into the managed domain
        let canary_file = mods_dir.join("22_Characters/Keqing/canary.txt");
        std::fs::write(&canary_file, "canary").unwrap();

        execute_deployment(&target_root, &plan, Some("Mods"), false)
            .expect("Cache-hit deployment must succeed");

        // Because it was a cache hit, the managed domain wasn't wiped, so canary_file still exists!
        assert!(
            canary_file.exists(),
            "Canary file should still exist on cache hit (rebuild skipped)"
        );

        // 3. Cache Miss: delete the generated file (22_Characters/Keqing/merged.ini)
        std::fs::remove_file(&merged_file).unwrap();
        assert!(!merged_file.exists());

        execute_deployment(&target_root, &plan, Some("Mods"), false)
            .expect("Cache-miss deployment must succeed");

        // Now merged_file was regenerated!
        assert!(
            merged_file.exists(),
            "Generated file must be recreated on cache miss"
        );
        // And managed domain was wiped and rebuilt, so canary_file is gone!
        assert!(
            !canary_file.exists(),
            "Canary file should be gone because cache-miss triggered rebuild"
        );
    }
}
