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

pub use commands::download::VoicePackStatus;
pub use commands::gamebanana::ModUpdateInfo;
pub use commands::library::{
    FileNode, GameConfigUpdate, IdentifiedGame, LoaderProgress, ProfileUpdate, ProtonProgress,
    SetupStatus,
};
pub use commands::profiles::BundleImportReport;
pub use launcher::{GraphicsAuditWarning, InjectionMethod};
pub use mod_patches::{
    DvtSyncReport, FileFixDiff, FixApplyReport, FixPreview, FixRuleSet, IniAnchor, LogDiagnostic,
    MeshPreviewData, VariantPreset,
};
pub use resources::effects::{EffectLibraryStatus, SharedEffectLibrary, TexFxConfig};
pub use resources::gamebanana::{GbModDetail, GbModFile, GbModSummary, YagoDeepLink};
pub use storage::{BundleModEntry, DiscoveredGame, LoadoutBundle, RemoteCatalogEntry, StartMethod};

use include_dir::{include_dir, Dir};

pub(crate) static ASSETS_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../resources");
