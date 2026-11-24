use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DeployStrategy {
    #[serde(rename = "ini_ast_3dmigoto")]
    #[default]
    IniAst3dMigoto,
    #[serde(rename = "vfs_direct")]
    VfsDirect,
    #[serde(rename = "pak_merge")]
    PakMerge,
    #[serde(rename = "scripted")]
    Scripted,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct LoaderSpec {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub common_libs_repo: Option<String>,
    #[serde(default)]
    pub strategy: DeployStrategy,
    #[serde(default)]
    pub proxy_files: Vec<String>,
    #[serde(default)]
    pub support_dirs: Vec<String>,
    #[serde(default = "default_mods_dir")]
    pub mods_target_dir: String,
    #[serde(default)]
    pub config_file: Option<String>,
    #[serde(default)]
    pub reload_key: Option<String>,
    #[serde(default)]
    pub dll_overrides: Option<String>,
    #[serde(default)]
    pub shader_fixes_dir: Option<String>,
}

fn default_mods_dir() -> String {
    "Mods".to_string()
}

impl Default for LoaderSpec {
    fn default() -> Self {
        Self::default_xxmi()
    }
}

impl LoaderSpec {
    /// Returns the standard default XXMI / 3DMigoto specification.
    pub fn default_xxmi() -> Self {
        Self {
            id: "xxmi-3dmigoto".to_string(),
            name: "XXMI / 3DMigoto".to_string(),
            common_libs_repo: Some("SpectrumQT/XXMI-Libs-Package".to_string()),
            strategy: DeployStrategy::IniAst3dMigoto,
            proxy_files: vec![
                "d3d11.dll".to_string(),
                "d3dcompiler_47.dll".to_string(),
                "d3dcompiler_46.dll".to_string(),
                "d3dx.ini".to_string(),
                "nvapi64.dll".to_string(),
                "version.dll".to_string(),
                "winmm.dll".to_string(),
                "3dmloader.dll".to_string(),
                "3dmloader.exe".to_string(),
            ],
            support_dirs: vec!["Core".to_string(), "ShaderFixes".to_string()],
            mods_target_dir: "Mods".to_string(),
            config_file: Some("d3dx.ini".to_string()),
            reload_key: Some("F10".to_string()),
            dll_overrides: Some("d3d11,dxgi,d3dcompiler_47=n,b".to_string()),
            shader_fixes_dir: Some("ShaderFixes".to_string()),
        }
    }

    /// Scans registry_root/<reg>/loaders/<loader_id>.json, returning the first valid match
    /// or falling back to LoaderSpec::default_xxmi().
    pub fn load_from_registries(registry_root: &Path, loader_id: &str) -> LoaderSpec {
        if registry_root.exists() {
            // 1. Check direct registry_root/loaders/<loader_id>.json
            let direct = registry_root
                .join("loaders")
                .join(format!("{}.json", loader_id));
            if direct.exists() {
                if let Ok(content) = std::fs::read_to_string(&direct) {
                    if let Ok(spec) = serde_json::from_str::<LoaderSpec>(&content) {
                        return spec;
                    }
                }
            }

            // 2. Scan subdirectories under registry_root (e.g. registry_root/<reg_id>/loaders/<loader_id>.json)
            if let Ok(entries) = std::fs::read_dir(registry_root) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let candidate = entry
                        .path()
                        .join("loaders")
                        .join(format!("{}.json", loader_id));
                    if candidate.exists() {
                        if let Ok(content) = std::fs::read_to_string(&candidate) {
                            if let Ok(spec) = serde_json::from_str::<LoaderSpec>(&content) {
                                return spec;
                            }
                        }
                    }
                }
            }
        }

        Self::default_xxmi()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_deserialize_loader_specs() {
        let xxmi_json = r#"{
            "id": "xxmi-3dmigoto",
            "name": "XXMI / 3DMigoto",
            "common_libs_repo": "SpectrumQT/XXMI-Libs-Package",
            "strategy": "ini_ast_3dmigoto",
            "proxy_files": ["d3d11.dll", "d3dcompiler_47.dll"],
            "mods_target_dir": "Mods",
            "config_file": "d3dx.ini",
            "reload_key": "F10",
            "dll_overrides": "d3d11,dxgi,d3dcompiler_47=n,b"
        }"#;

        let xxmi: LoaderSpec = serde_json::from_str(xxmi_json).unwrap();
        assert_eq!(xxmi.id, "xxmi-3dmigoto");
        assert_eq!(xxmi.strategy, DeployStrategy::IniAst3dMigoto);
        assert_eq!(xxmi.mods_target_dir, "Mods");
        assert_eq!(xxmi.reload_key, Some("F10".to_string()));

        let vfs_direct_json = r#"{
            "id": "vfs-direct",
            "name": "Direct VFS Linker",
            "strategy": "vfs_direct",
            "proxy_files": [],
            "support_dirs": [],
            "mods_target_dir": "Paks/~mods",
            "config_file": null,
            "reload_key": null,
            "dll_overrides": null
        }"#;

        let vfs: LoaderSpec = serde_json::from_str(vfs_direct_json).unwrap();
        assert_eq!(vfs.id, "vfs-direct");
        assert_eq!(vfs.strategy, DeployStrategy::VfsDirect);
        assert_eq!(vfs.mods_target_dir, "Paks/~mods");
        assert!(vfs.proxy_files.is_empty());
    }

    #[test]
    fn test_load_from_registries() {
        let dir = tempdir().unwrap();
        let reg_loaders = dir.path().join("official").join("loaders");
        std::fs::create_dir_all(&reg_loaders).unwrap();

        let custom_spec = LoaderSpec {
            id: "custom-pak".to_string(),
            name: "Custom Pak Merger".to_string(),
            common_libs_repo: None,
            strategy: DeployStrategy::PakMerge,
            proxy_files: vec![],
            support_dirs: vec![],
            mods_target_dir: "~mods".to_string(),
            config_file: None,
            reload_key: None,
            dll_overrides: None,
            shader_fixes_dir: None,
        };

        std::fs::write(
            reg_loaders.join("custom-pak.json"),
            serde_json::to_string(&custom_spec).unwrap(),
        )
        .unwrap();

        // 1. Found custom loader
        let loaded = LoaderSpec::load_from_registries(dir.path(), "custom-pak");
        assert_eq!(loaded.id, "custom-pak");
        assert_eq!(loaded.strategy, DeployStrategy::PakMerge);
        assert_eq!(loaded.mods_target_dir, "~mods");

        // 2. Missing loader falls back to default_xxmi
        let fallback = LoaderSpec::load_from_registries(dir.path(), "non-existent");
        assert_eq!(fallback.id, "xxmi-3dmigoto");
        assert_eq!(fallback.strategy, DeployStrategy::IniAst3dMigoto);
    }
}
