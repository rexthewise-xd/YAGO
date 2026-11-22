pub mod audit;
pub mod error;
pub mod input;
pub mod launcher;
pub mod manual_map;
pub mod monitor;
pub mod patcher;
pub mod pattern_scanner;
pub mod sandbox;
pub mod tuner;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "linux")]
pub mod linux;

pub use audit::{apply_wuwa_engine_tweaks, audit_game_graphics, GraphicsAuditWarning};
pub use error::{MarshalError, Result};
pub use input::send_reload_f10;
pub use launcher::{
    InjectionMethod, LaunchOptions, Launcher, RunnerConfig, RunnerType, StartMethod,
};
pub use monitor::Monitor;
pub use patcher::MemoryPatcher;
pub use pattern_scanner::PatternScanner;
pub use sandbox::{SandboxConfig, SandboxManager};
pub use tuner::Tuner;

#[cfg(target_os = "linux")]
pub use linux::{launch_with_proton, LaunchConfig};
