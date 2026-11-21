fn main() {
    #[cfg(target_os = "linux")]
    {
        use std::path::Path;
        use std::process::Command;

        println!("cargo:rerun-if-changed=../loader/src/assets/shield.c");

        let out_dir = std::env::var("OUT_DIR").unwrap();
        let shield_out_path = Path::new(&out_dir).join("libshield.so");

        let status = Command::new("gcc")
            .args([
                "-shared",
                "-fPIC",
                "../loader/src/assets/shield.c",
                "-o",
                shield_out_path.to_str().unwrap(),
                "-ldl",
            ])
            .status()
            .expect("failed to run gcc to compile libshield.so");

        if !status.success() {
            panic!("gcc failed to compile libshield.so");
        }

        println!(
            "cargo:rustc-env=YAGO_SHIELD_PATH={}",
            shield_out_path.display()
        );
    }
}