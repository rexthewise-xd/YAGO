pub mod assets;
pub mod commands;
pub mod config;
pub mod diagnostics_bundle;
pub mod events;
pub mod init;
pub mod instance;
pub mod logging;
pub mod reconcile;
pub mod safety;
pub mod state;

#[cfg(test)]
mod init_tests;

pub use config::AppConfig;
pub use diagnostics_bundle::{create_bug_report_bundle, redact_text, SystemInfoReport};
pub use events::{AppEvent, EventBus};
pub use init::{default_app_data_dir, init, init_with_dir};
pub use instance::{acquire_or_forward, InstanceGuard, InstanceResult};
pub use logging::{
    acknowledge_crash_log, find_recent_crash_log, init_logging, install_panic_hook, WorkerGuard,
};
pub use state::AppCore;

pub use commands::config::AppUpdateInfo;
pub use commands::download::VoicePackStatus;
pub use commands::gamebanana::ModUpdateInfo;
pub use commands::launcher::PreflightIssue;
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
pub use reconcile::{reconcile_library, ReconcileReport};
pub use resources::effects::{EffectLibraryStatus, SharedEffectLibrary, TexFxConfig};
pub use resources::gamebanana::{GbModDetail, GbModFile, GbModSummary, YagoDeepLink};
pub use resources::registry::{RegistryEntry, RegistryIndex, RegistrySource, RegistrySyncReport};
pub use storage::{BundleModEntry, DiscoveredGame, LoadoutBundle, RemoteCatalogEntry, StartMethod};

use include_dir::{include_dir, Dir};

pub(crate) static ASSETS_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../resources");
