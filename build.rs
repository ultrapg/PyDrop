use std::env;
use std::path::Path;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=src/terminal.rs");
    println!("cargo:rerun-if-changed=config.json");

    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("terminal.exe");

    let status = Command::new("rustc")
        .args(&[
            "src/terminal.rs",
            "-o",
            dest_path.to_str().unwrap(),
            "-C",
            "opt-level=3",
        ])
        .status()
        .expect("Failed to run rustc to compile src/terminal.rs");

    if !status.success() {
        panic!("Failed to compile src/terminal.rs with rustc");
    }
}
