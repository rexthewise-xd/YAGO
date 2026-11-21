use std::collections::HashMap;
use std::sync::Arc;

use dioxus::prelude::*;
use storage::{GameConfig, GameTemplate, GlobalSettings, LibraryDatabase};
use sync::orchestrator::ProgressDetailed;
use yago_app::{AppConfig, AppCore, AppEvent, LoaderProgress, ProtonProgress};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum View {
    Overview,
    Mods,
    Loadouts,
    Skins,
    Settings,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Overlay {
    AddGame,
    Install { game_id: String, name: String },
    CoverManager { game_id: String },
}

impl View {
    pub fn title(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Mods => "Mods",
            Self::Loadouts => "Loadouts",
            Self::Skins => "Skins",
            Self::Settings => "Settings",
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct Toast {
    pub id: u64,
    pub kind: ToastKind,
    pub message: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Error,
}

#[derive(Clone, PartialEq)]
pub struct Task {
    pub id: u64,
    pub label: String,
    pub detail: String,
    pub state: TaskState,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Running,
    Done,
    Failed,
}

#[derive(Clone)]
pub struct Store {
    pub core: AppCore,
    pub library: Signal<Arc<HashMap<String, LibraryDatabase>>>,
    pub templates: Signal<Arc<HashMap<String, GameTemplate>>>,
    pub selected: Signal<Option<String>>,
    pub view: Signal<View>,
    pub overlay: Signal<Option<Overlay>>,
    pub settings: Signal<GlobalSettings>,
    pub app_config: Signal<AppConfig>,
    pub downloads: Signal<HashMap<String, ProgressDetailed>>,
    pub loader_progress: Signal<HashMap<String, LoaderProgress>>,
    pub proton_progress: Signal<Option<ProtonProgress>>,
    pub running: Signal<bool>,
    pub launching: Signal<bool>,
    pub panic: Signal<bool>,
    pub close_requested: Signal<bool>,
    pub toasts: Signal<Vec<Toast>>,
    pub tasks: Signal<Vec<Task>>,
    pub setup_done: Signal<Option<bool>>,
    pub loaded: Signal<bool>,
    next_id: Signal<u64>,
}

impl Store {
    pub fn new(core: AppCore) -> Self {
        Self {
            core,
            library: Signal::new(Arc::new(HashMap::new())),
            templates: Signal::new(Arc::new(HashMap::new())),
            selected: Signal::new(None),
            view: Signal::new(View::Overview),
            overlay: Signal::new(None),
            settings: Signal::new(GlobalSettings::default()),
            app_config: Signal::new(AppConfig::default()),
            downloads: Signal::new(HashMap::new()),
            loader_progress: Signal::new(HashMap::new()),
            proton_progress: Signal::new(None),
            running: Signal::new(false),
            launching: Signal::new(false),
            panic: Signal::new(false),
            close_requested: Signal::new(false),
            toasts: Signal::new(Vec::new()),
            tasks: Signal::new(Vec::new()),
            setup_done: Signal::new(None),
            loaded: Signal::new(false),
            next_id: Signal::new(1),
        }
    }

    fn next_id(&mut self) -> u64 {
        let id = (self.next_id)();
        self.next_id.set(id + 1);
        id
    }

    pub fn toast(&mut self, kind: ToastKind, message: impl Into<String>) {
        let id = self.next_id();
        let toast = Toast {
            id,
            kind,
            message: message.into(),
        };
        self.toasts.write().push(toast);
        let mut toasts = self.toasts;
        spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            toasts.write().retain(|t| t.id != id);
        });
    }

    pub fn task_start(&mut self, label: impl Into<String>, detail: impl Into<String>) -> u64 {
        let id = self.next_id();
        self.tasks.write().push(Task {
            id,
            label: label.into(),
            detail: detail.into(),
            state: TaskState::Running,
        });
        id
    }

    pub fn task_finish(&mut self, id: u64, ok: bool, detail: impl Into<String>) {
        let mut tasks = self.tasks.write();
        if let Some(t) = tasks.iter_mut().find(|t| t.id == id) {
            t.state = if ok { TaskState::Done } else { TaskState::Failed };
            t.detail = detail.into();
        }
    }

    /// Run a facade call, reporting errors as toasts. Returns the result.
    pub fn call<T: Send + 'static>(
        &self,
        fut: impl std::future::Future<Output = Result<T, String>> + Send + 'static,
        on_ok: impl FnOnce(&mut Store, T) + Send + 'static,
    ) {
        let mut store = self.clone();
        spawn(async move {
            match fut.await {
                Ok(value) => on_ok(&mut store, value),
                Err(e) => store.toast(ToastKind::Error, e),
            }
        });
    }

    pub fn game(&self, id: &str) -> Option<GameConfig> {
        self.library
            .read()
            .get(id)
            .and_then(|db| db.games.get(id))
            .cloned()
    }

    pub fn selected_game(&self) -> Option<GameConfig> {
        let id = (self.selected)()?;
        self.game(&id)
    }

    pub fn db(&self, id: &str) -> Option<LibraryDatabase> {
        self.library.read().get(id).cloned()
    }

    pub fn game_ids(&self) -> Vec<String> {
        let lib = self.library.read();
        let mut ids: Vec<String> = lib
            .iter()
            .filter(|(_, db)| db.games.values().any(|g| g.enabled))
            .map(|(id, _)| id.clone())
            .collect();
        ids.sort_by(|a, b| {
            let ga = self.game(a);
            let gb = self.game(b);
            ga.map(|g| g.name)
                .cmp(&gb.map(|g| g.name))
                .then(a.cmp(b))
        });
        ids
    }

    pub fn apply_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::LibraryUpdated(dbs) => {
                let had_selection = (self.selected)().is_some();
                self.library.set(dbs);
                if !had_selection {
                    let first = self.game_ids().first().cloned();
                    self.selected.set(first);
                } else if let Some(sel) = (self.selected)() {
                    if self.game(&sel).is_none() {
                        let first = self.game_ids().first().cloned();
                        self.selected.set(first);
                    }
                }
            }
            AppEvent::DownloadProgress(p) => {
                self.downloads.write().insert(p.game_id.clone(), p);
            }
            AppEvent::DownloadError(msg) => {
                let duplicate = self
                    .toasts
                    .read()
                    .last()
                    .is_some_and(|t| t.kind == ToastKind::Error && t.message == msg);
                if !duplicate {
                    self.toast(ToastKind::Error, msg);
                }
            }
            AppEvent::DownloadComplete(game_id) => {
                self.downloads.write().remove(&game_id);
                let name = self
                    .game(&game_id)
                    .map(|g| g.name)
                    .unwrap_or_else(|| game_id.clone());
                self.toast(ToastKind::Success, format!("{name} download complete"));
            }
            AppEvent::TaskCompleted(msg) => {
                self.toast(ToastKind::Success, msg);
            }
            AppEvent::GameStarted => {
                self.running.set(true);
                self.launching.set(false);
            }
            AppEvent::GameStopped => {
                self.running.set(false);
                self.launching.set(false);
            }
            AppEvent::SettingsUpdated(s) => self.settings.set(s),
            AppEvent::AppConfigUpdated(c) => self.app_config.set(c),
            AppEvent::PanicTriggered => {
                self.panic.set(true);
                self.toast(
                    ToastKind::Success,
                    "Panic Triggered: Active mods purged and reload (F10) sent.",
                );
            }
            AppEvent::LoaderProgress(p) => {
                self.loader_progress.write().insert(p.game_id.clone(), p);
            }
            AppEvent::ProtonProgress(p) => self.proton_progress.set(Some(p)),
        }
    }

    pub fn save_settings(&mut self, settings: GlobalSettings) {
        let core = self.core.clone();
        self.settings.set(settings.clone());
        self.call(
            async move { core.update_settings(settings).await },
            |_, _| {},
        );
    }

    pub async fn refresh(&mut self) {
        let core = self.core.clone();
        if let Ok(lib) = core.get_library().await {
            self.library.set(Arc::new(lib));
        }
        let templates = core.game_templates.lock().await.clone();
        self.templates.set(Arc::new(templates));
        if let Ok(s) = core.get_settings().await {
            self.settings.set(s);
        }
        if let Ok(c) = core.get_app_config().await {
            self.app_config.set(c);
        }
        if let Ok(done) = core.check_setup().await {
            self.setup_done.set(Some(done));
        }
        if (self.selected)().is_none() {
            let first = self.game_ids().first().cloned();
            self.selected.set(first);
        }
        self.loaded.set(true);
    }
}