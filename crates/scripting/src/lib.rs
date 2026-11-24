pub mod error;
pub mod sandbox;

pub use error::ScriptError;
pub use sandbox::{ScriptAuditWarning, ScriptExecutionResult, ScriptHost, ScriptLimits};

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_infinite_loop_terminated_by_op_limit() {
        let dir = tempdir().unwrap();
        let script = "loop {}";
        let res = ScriptHost::run_fixer_script(script, dir.path(), true);
        assert!(res.is_err());
        let err = res.unwrap_err();
        match err {
            ScriptError::Runtime(msg) => {
                assert!(
                    msg.to_lowercase().contains("operations")
                        || msg.to_lowercase().contains("limit")
                );
            }
            other => panic!(
                "Expected Runtime error with operations limit, got: {:?}",
                other
            ),
        }
    }

    #[test]
    fn test_path_traversal_blocked() {
        let dir = tempdir().unwrap();

        // 1. fs_read_text traversal
        let script_read = r#"fs_read_text("../secret.txt");"#;
        let res_read = ScriptHost::run_fixer_script(script_read, dir.path(), true);
        assert!(res_read.is_err());
        match res_read.unwrap_err() {
            ScriptError::SecurityViolation(msg) => {
                assert!(msg.contains("Path traversal rejected"));
            }
            other => panic!("Expected SecurityViolation, got: {:?}", other),
        }

        // 2. fs_write_text traversal
        let script_write = r#"fs_write_text("/etc/passwd", "malicious");"#;
        let res_write = ScriptHost::run_fixer_script(script_write, dir.path(), true);
        assert!(res_write.is_err());
        match res_write.unwrap_err() {
            ScriptError::SecurityViolation(msg) => {
                assert!(msg.contains("Path traversal rejected"));
            }
            other => panic!("Expected SecurityViolation, got: {:?}", other),
        }
    }

    #[test]
    fn test_fixer_script_dry_run_and_apply() {
        let dir = tempdir().unwrap();
        let mod_ini_path = dir.path().join("mod.ini");
        let initial_ini = "[TextureOverrideBody]\nhash = deadbeef\nmatch_first_index = 0\n";
        std::fs::write(&mod_ini_path, initial_ini).unwrap();

        let fixer_script = r#"
            log_info("Starting mod repair...");
            let content = fs_read_text("mod.ini");
            let replaced = ini_replace_hash(content, "deadbeef", "cafebabe");
            let patched = ini_set_value(replaced, "TextureOverrideBody", "match_priority", "10");
            fs_write_text("mod.ini", patched);
            log_info("Repair finished successfully.");
            #{ status: "repaired", count: 1 }
        "#;

        // 1. Dry run: disk remains untouched, modified_files reports mod.ini
        let dry_res = ScriptHost::run_fixer_script(fixer_script, dir.path(), true).unwrap();
        assert_eq!(dry_res.modified_files, vec!["mod.ini"]);
        assert!(dry_res
            .logs
            .iter()
            .any(|l| l.contains("Starting mod repair...")));
        assert!(dry_res
            .logs
            .iter()
            .any(|l| l.contains("Repair finished successfully.")));
        assert_eq!(dry_res.returned_json["status"], "repaired");

        let disk_content_after_dry = std::fs::read_to_string(&mod_ini_path).unwrap();
        assert_eq!(
            disk_content_after_dry, initial_ini,
            "Dry run must not modify disk"
        );

        // 2. Real run: disk is updated
        let real_res = ScriptHost::run_fixer_script(fixer_script, dir.path(), false).unwrap();
        assert_eq!(real_res.modified_files, vec!["mod.ini"]);

        let disk_content_after_real = std::fs::read_to_string(&mod_ini_path).unwrap();
        assert!(disk_content_after_real.contains("hash = cafebabe"));
        assert!(disk_content_after_real.contains("match_priority = 10"));
    }

    #[test]
    fn test_audit_script_read_only_and_emits_warning() {
        let dir = tempdir().unwrap();
        let engine_ini = dir.path().join("Engine.ini");
        std::fs::write(&engine_ini, "[SystemSettings]\nr.Streaming.MinBoost = 0\n").unwrap();

        // 1. Audit script emitting warnings
        let audit_script = r#"
            if fs_exists("Engine.ini") {
                let content = fs_read_text("Engine.ini");
                if ini_has_section(content, "SystemSettings") {
                    emit_warning("warning", "Texture Streaming Suboptimal", "r.Streaming.MinBoost is set to 0");
                }
            }
        "#;

        let audit_res = ScriptHost::run_audit_script(audit_script, dir.path()).unwrap();
        assert_eq!(audit_res.warnings.len(), 1);
        assert_eq!(audit_res.warnings[0].level, "warning");
        assert_eq!(audit_res.warnings[0].title, "Texture Streaming Suboptimal");
        assert_eq!(
            audit_res.warnings[0].message,
            "r.Streaming.MinBoost is set to 0"
        );

        // 2. Attempting to write in audit mode is blocked
        let malicious_audit = r#"fs_write_text("Engine.ini", "corrupted");"#;
        let write_res = ScriptHost::run_audit_script(malicious_audit, dir.path());
        assert!(write_res.is_err());
        match write_res.unwrap_err() {
            ScriptError::SecurityViolation(msg) => {
                assert!(msg.contains("read-only"));
            }
            other => panic!("Expected SecurityViolation, got: {:?}", other),
        }
    }
}
