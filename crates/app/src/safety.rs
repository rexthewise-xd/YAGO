use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use vfs::Vfs;

use crate::events::AppEvent;
use crate::state::AppCore;

pub struct Emergency;

impl Emergency {
    /// The "Panic Option". Removes mods and reloads the modloader without killing the game.
    pub async fn trigger(core: &AppCore) {
        println!("PANIC SWITCH TRIGGERED! (Alt+F12)");

        println!("Purging deployments for all known games...");
        let dbs = core.game_dbs.lock().await;
        for (game_id, db) in dbs.iter() {
            if let Some(game_config) = db.games.get(game_id) {
                let game_root = game_config
                    .install_path
                    .parent()
                    .unwrap_or(&game_config.install_path);
                let yago_dir = game_root.join("Mods").join("YAGO");
                if yago_dir.exists() {
                    println!("Purging: {:?}", yago_dir);
                    if let Err(e) = Vfs::wipe_deployment(&yago_dir).await {
                        eprintln!("Failed to wipe deployment for {}: {}", game_id, e);
                    }
                }
            }
        }
        drop(dbs);

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