use crate::error::{LibrarianError, Result};
use serde::{de::DeserializeOwned, Serialize};
use std::fs::File;
use std::io::Write;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryLoad<T> {
    pub data: T,
    pub recovered_from_backup: bool,
}

/// Serializes pretty JSON to <path>.tmp, calls file.sync_all(), rotates existing <path>
/// to <path>.bak (if present), and atomically renames <path>.tmp -> <path>.
pub fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let tmp_path = path.with_extension("tmp");
    let bak_path = path.with_extension("bak");

    // 1. Serialize pretty JSON to <path>.tmp
    let json_bytes = serde_json::to_vec_pretty(value)?;
    {
        let mut file = File::create(&tmp_path)?;
        file.write_all(&json_bytes)?;
        file.sync_all()?;
    }

    // 2. Rotate existing <path> to <path>.bak (if present)
    if path.exists() {
        let _ = std::fs::copy(path, &bak_path);
    }

    // 3. Atomically rename <path>.tmp -> <path>
    std::fs::rename(&tmp_path, path)?;

    Ok(())
}

/// Attempts to read and parse <path>. If <path> is corrupted/truncated, quarantines it
/// by renaming to <path>.corrupt.<timestamp>, attempts to load <path>.bak, restores <path>
/// from <path>.bak if valid, and returns recovered_from_backup: true.
/// Only returns an error if both <path> and <path>.bak are missing or unreadable.
pub fn read_json_with_recovery<T: DeserializeOwned>(path: &Path) -> Result<RecoveryLoad<T>> {
    let bak_path = path.with_extension("bak");

    // 1. If path exists, attempt to read and parse
    if path.exists() {
        match std::fs::read_to_string(path) {
            Ok(content) => match serde_json::from_str::<T>(&content) {
                Ok(data) => {
                    return Ok(RecoveryLoad {
                        data,
                        recovered_from_backup: false,
                    });
                }
                Err(err) => {
                    eprintln!(
                        "Corrupted JSON in {:?}: {}. Quarantining and attempting recovery...",
                        path, err
                    );
                    quarantine_file(path);
                }
            },
            Err(err) => {
                eprintln!(
                    "Unreadable file {:?}: {}. Quarantining and attempting recovery...",
                    path, err
                );
                quarantine_file(path);
            }
        }
    }

    // 2. Attempt recovery from <path>.bak
    if bak_path.exists() {
        match std::fs::read_to_string(&bak_path) {
            Ok(content) => match serde_json::from_str::<T>(&content) {
                Ok(data) => {
                    eprintln!(
                        "Successfully recovered {:?} from backup {:?}",
                        path, bak_path
                    );
                    // Restore <path> from <path>.bak
                    let _ = std::fs::copy(&bak_path, path);
                    return Ok(RecoveryLoad {
                        data,
                        recovered_from_backup: true,
                    });
                }
                Err(err) => {
                    eprintln!("Backup file {:?} is also invalid: {}", bak_path, err);
                }
            },
            Err(err) => {
                eprintln!("Failed to read backup file {:?}: {}", bak_path, err);
            }
        }
    }

    Err(LibrarianError::Io(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        format!(
            "Failed to load {:?}: primary file and backup are missing or corrupt",
            path
        ),
    )))
}

fn quarantine_file(path: &Path) {
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
    let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("file");
    let corrupt_name = format!("{}.corrupt.{}", file_name, timestamp);
    let corrupt_path = path.with_file_name(corrupt_name);
    let _ = std::fs::rename(path, corrupt_path);
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_write_json_atomic_creates_bak_on_second_write() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("config.json");
        let bak_path = dir.path().join("config.bak");

        // First write
        write_json_atomic(&file_path, &"version_1").unwrap();
        assert!(file_path.exists());
        assert!(!bak_path.exists(), ".bak should not exist on initial write");

        // Second write
        write_json_atomic(&file_path, &"version_2").unwrap();
        assert!(file_path.exists());
        assert!(bak_path.exists(), ".bak must exist on second write");

        let bak_val: String =
            serde_json::from_str(&std::fs::read_to_string(&bak_path).unwrap()).unwrap();
        assert_eq!(bak_val, "version_1");

        let current_val: String =
            serde_json::from_str(&std::fs::read_to_string(&file_path).unwrap()).unwrap();
        assert_eq!(current_val, "version_2");
    }

    #[test]
    fn test_read_json_with_recovery_quarantines_corrupt_and_recovers_bak() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("data.json");
        let bak_path = dir.path().join("data.bak");

        // Create initial backup file and a corrupt primary file
        write_json_atomic(&file_path, &"original_good_data").unwrap();
        write_json_atomic(&file_path, &"second_good_data").unwrap();
        assert!(bak_path.exists());

        // Corrupt the primary file
        std::fs::write(&file_path, b"{{{ corrupt broken json truncated").unwrap();

        // Read with recovery
        let loaded = read_json_with_recovery::<String>(&file_path).unwrap();
        assert!(
            loaded.recovered_from_backup,
            "Must report recovered_from_backup == true"
        );
        assert_eq!(loaded.data, "original_good_data");

        // Primary file should now be restored
        assert!(file_path.exists());
        let restored_val: String =
            serde_json::from_str(&std::fs::read_to_string(&file_path).unwrap()).unwrap();
        assert_eq!(restored_val, "original_good_data");

        // Verify that a *.corrupt.* quarantine file was created
        let corrupt_found = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .any(|e| e.file_name().to_string_lossy().contains(".corrupt."));
        assert!(
            corrupt_found,
            "A quarantined .corrupt. file must be preserved in the directory"
        );
    }
}
