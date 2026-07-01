use std::process::Command;
use std::env;
use std::path::PathBuf;

fn main() {
    println!("=== PyDrop Portable Terminal Environment ===");
    println!("Type 'pydrop' to execute python, pip, or bundle commands.");
    println!("Type 'exit' to close this terminal.\n");

    // Get the directory containing this terminal.exe
    let mut exe_dir = env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
    exe_dir.pop(); // Remove terminal.exe filename to get the directory

    // Prep PATH environment variable
    let path_var = env::var("PATH").unwrap_or_default();
    let new_path = format!("{};{}", exe_dir.display(), path_var);

    // Spawn cmd.exe as an interactive shell
    let mut child = Command::new("cmd.exe")
        .env("PATH", new_path)
        .spawn()
        .expect("Failed to start cmd.exe process");

    // Wait for the command shell to finish
    let _ = child.wait();
}
