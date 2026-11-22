use std::collections::HashMap;
use std::sync::Arc;

use storage::{GlobalSettings, LibraryDatabase};
use sync::orchestrator::ProgressDetailed;
use tokio::sync::broadcast;

use crate::commands::library::{LoaderProgress, ProtonProgress};
use crate::config::AppConfig;

#[derive(Clone, Debug)]
pub enum AppEvent {
    LibraryUpdated(Arc<HashMap<String, LibraryDatabase>>),
    DownloadProgress(ProgressDetailed),
    DownloadError(String),
    DownloadComplete(String),
    TaskCompleted(String),
    GameStarted,
    GameStopped,
    SettingsUpdated(GlobalSettings),
    AppConfigUpdated(AppConfig),
    PanicTriggered,
    LoaderProgress(LoaderProgress),
    ProtonProgress(ProtonProgress),
}

#[derive(Clone)]
pub struct EventBus {
    tx: broadcast::Sender<AppEvent>,
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(512);
        Self { tx }
    }

    pub fn emit(&self, event: AppEvent) {
        let _ = self.tx.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AppEvent> {
        self.tx.subscribe()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}
