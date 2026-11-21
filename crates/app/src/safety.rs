use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use std::path::Path;
use vfs::Vfs;

use crate::events::AppEvent;
use crate::state::AppCore;

pub struct Emergency;

impl Emergency {
    pub const MANAGED_DOMAINS: &'static [&'static str] = &[
        "00_Configs",
        "11_GlobalUI",
        "22_Characters",
        "33_Overrides",
        "99_Unknown",
        "YAGO",
    ];

    /// Purges active deployment inside a profile's Mods folder.
    pub async fn purge_profile_mods(profile_mods: &Path) -> vfs::Result<()> {
        if !profile_mods.exists() {
            return Ok(());
        }

        for domain in Self::MANAGED_DOMAINS {
            let domain_dir = profile_mods.join(domain);
            if domain_dir.exists() {
                let _ = std::fs::remove_dir_all(&domain_dir);
            }
        }

        let lock_file = profile_mods.join("deployment.lock");
        if lock_file.exists() {
            let _ = std::fs::remove_file(lock_file);
        }

        Vfs::wipe_deployment(profile_mods).await?;
        Ok(())
    }

    /// The "Panic Option". Removes mods and reloads the modloader without killing the game.
    pub async fn trigger(core: &AppCore) {
        println!("PANIC SWITCH TRIGGERED! (Alt+F12)");

        println!("Purging deployments for all known games...");
        let game_ids: Vec<String> = {
            let dbs = core.game_dbs.lock().await;
            dbs.keys().cloned().collect()
        };

        {
            let librarian = core.librarian.lock().await;
            for game_id in &game_ids {
                let paths = librarian.game_paths(game_id);
                println!(
                    "Purging deployment for {}: {:?}",
                    game_id, paths.profile_mods
                );
                if let Err(e) = Self::purge_profile_mods(&paths.profile_mods).await {
                    eprintln!("Failed to wipe deployment for {}: {}", game_id, e);
                }
            }
        }

        println!("Sending F10 to reload modloader...");
        match Enigo::new(&Settings::default()) {
            Ok(mut enigo) => {
                if let Err(e) = enigo.key(Key::F10, Direction::Click) {
                    eprintln!("Failed to send F10: {}", e);
                } else {
                    println!("F10 sent successfully.");
                }
            }
            Err(e) => eprintln!("Failed to initialize Enigo: {}", e),
        }

        core.emit(AppEvent::PanicTriggered);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init::init_with_dir;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_emergency_trigger_purges_profile_mods() {
        let dir = tempdir().unwrap();
        let core = init_with_dir(dir.path().to_path_buf()).await.unwrap();

        let game_id = "genshin".to_string();
        {
            let mut dbs = core.game_dbs.lock().await;
            dbs.insert(game_id.clone(), storage::LibraryDatabase::default());
        }

        let paths = {
            let librarian = core.librarian.lock().await;
            librarian.game_paths(&game_id)
        };

        // Create active deployment files inside paths.profile_mods
        let char_mod_dir = paths.profile_mods.join("22_Characters").join("Keqing");
        std::fs::create_dir_all(&char_mod_dir).unwrap();
        std::fs::write(char_mod_dir.join("merged.ini"), "test content").unwrap();

        let configs_dir = paths.profile_mods.join("00_Configs");
        std::fs::create_dir_all(&configs_dir).unwrap();
        std::fs::write(configs_dir.join("d3dx.ini"), "config content").unwrap();

        let lock_file = paths.profile_mods.join("deployment.lock");
        std::fs::write(&lock_file, "lock_hash").unwrap();

        assert!(char_mod_dir.exists());
        assert!(configs_dir.exists());
        assert!(lock_file.exists());

        let mut rx = core.events.subscribe();

        // Trigger Panic Purge
        Emergency::trigger(&core).await;

        // Verify that deployed domain folders and lock are purged
        assert!(
            !char_mod_dir.exists(),
            "22_Characters folder should be purged"
        );
        assert!(!configs_dir.exists(), "00_Configs folder should be purged");
        assert!(!lock_file.exists(), "deployment.lock should be purged");

        // Verify PanicTriggered event was emitted
        let event = rx.recv().await.unwrap();
        assert!(matches!(event, AppEvent::PanicTriggered));
    }
}
