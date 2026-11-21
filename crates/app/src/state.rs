use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use storage::{GameTemplate, GlobalSettings, Librarian, LibraryDatabase, SettingsManager};
use tokio::sync::{watch, Mutex};

use crate::config::AppConfig;
use crate::events::{AppEvent, EventBus};

#[derive(Clone)]
pub struct AppCore {
    pub app_data_dir: PathBuf,
    pub librarian: Arc<Mutex<Librarian>>,
    pub game_templates: Arc<Mutex<HashMap<String, GameTemplate>>>,
    pub game_dbs: Arc<Mutex<HashMap<String, LibraryDatabase>>>,
    pub running_game_name: Arc<Mutex<Option<String>>>,
    pub is_launching: Arc<Mutex<bool>>,
    pub settings_manager: Arc<SettingsManager>,
    pub global_settings: Arc<Mutex<GlobalSettings>>,
    pub app_config: Arc<Mutex<AppConfig>>,
    pub download_controls: Arc<Mutex<HashMap<String, watch::Sender<bool>>>>,
    pub events: EventBus,
}

impl AppCore {
    pub fn emit(&self, event: AppEvent) {
        self.events.emit(event);
    }

    pub async fn emit_library(&self) {
        let dbs = self.game_dbs.lock().await;
        self.emit(AppEvent::LibraryUpdated(Arc::new(dbs.clone())));
    }
}