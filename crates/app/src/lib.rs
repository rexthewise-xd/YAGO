pub mod assets;
pub mod commands;
pub mod config;
pub mod events;
pub mod init;
pub mod safety;
pub mod state;

#[cfg(test)]
mod init_tests;

pub use config::AppConfig;
pub use events::{AppEvent, EventBus};
pub use init::{default_app_data_dir, init, init_with_dir};
pub use state::AppCore;

pub use commands::library::{
    FileNode, GameConfigUpdate, IdentifiedGame, LoaderProgress, ProfileUpdate, ProtonProgress,
    SetupStatus,
};
pub use launcher::InjectionMethod;
pub use storage::{DiscoveredGame, RemoteCatalogEntry};

use include_dir::{include_dir, Dir};

pub(crate) static ASSETS_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../resources");