use anyhow::{Result, Context};
use std::process::Command;
use std::path::{Path, PathBuf};
#[cfg(feature = "rustpython")] use rustpython_vm;

use uuid::Uuid;
use crate::python_env::{ensure_python, ensure_local_python, pip_install, get_config};
use regex::Regex;

/// Detect top‑level import statements (simple regex).
pub fn detect_imports(script_path: &PathBuf) -> Result<Vec<String>> {
    let content = std::fs::read_to_string(script_path)
        .with_context(|| format!("Reading script {:?}", script_path))?;
    let re = Regex::new(r"^\s*(?:import|from)\s+([a-zA-Z0-9_]+)")
        .context("Compiling import regex")?;
    let mut pkgs = Vec::new();
    for cap in re.captures_iter(&content) {
        let name = cap[1].to_string();
        if !pkgs.contains(&name) {
            pkgs.push(name);
        }
    }
    Ok(pkgs)
}

/// CPython backend – run a script using the downloaded interpreter.
#[cfg(feature = "cp")]
pub async fn run_cp_script(script: &PathBuf) -> Result<()> {
    let config = get_config();
    let python_dir = ensure_python(&config.python_version).await?;

    let env_dir = std::env::temp_dir()
        .join(format!("pydrop_env_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&env_dir)
        .with_context(|| format!("Creating env dir {:?}", env_dir))?;

    let imports = detect_imports(script)?;
    if !imports.is_empty() {
        println!("Installing detected packages: {:?}", imports);
        pip_install(&python_dir, &env_dir, &imports)?;
    }

    let python_exe = python_dir.join("python.exe");
    let status = Command::new(python_exe)
        .arg(script)
        .env("PYTHONPATH", env_dir.to_str().unwrap())
        .status()
        .with_context(|| "Running python script")?;
    if !status.success() {
        anyhow::bail!("Script exited with status {:?}", status);
    }
    Ok(())
}

/// Local CPython backend - run script using python environment in the folder of the script.
pub async fn run_local_script(script: &PathBuf) -> Result<()> {
    let script_dir = script.parent().unwrap_or_else(|| Path::new("."));
    let python_dir = ensure_local_python(script_dir).await?;

    let env_dir = std::env::temp_dir()
        .join(format!("pydrop_env_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&env_dir)
        .with_context(|| format!("Creating env dir {:?}", env_dir))?;

    let imports = detect_imports(script)?;
    if !imports.is_empty() {
        println!("Installing detected packages: {:?}", imports);
        pip_install(&python_dir, &env_dir, &imports)?;
    }

    let python_exe = python_dir.join("python.exe");
    let status = Command::new(python_exe)
        .arg(script)
        .env("PYTHONPATH", env_dir.to_str().unwrap())
        .status()
        .with_context(|| "Running python script")?;
    if !status.success() {
        anyhow::bail!("Script exited with status {:?}", status);
    }
    Ok(())
}

/// Launches the native python.exe REPL.
pub fn run_interactive_shell(python_dir: &Path) -> Result<()> {
    let python_exe = python_dir.join("python.exe");
    let status = Command::new(python_exe)
        .status()
        .with_context(|| "Launching interactive python shell")?;
    if !status.success() {
        anyhow::bail!("Python shell exited with error: {:?}", status);
    }
    Ok(())
}

/// RustPython backend – run a script using the pure‑Rust interpreter.
#[cfg(feature = "rustpython")]
pub async fn run_rustpython_script(script: &PathBuf) -> Result<()> {
    let source = std::fs::read_to_string(script)
        .with_context(|| format!("Reading script {:?}", script))?;
    rustpython_vm::Interpreter::without_stdlib(Default::default()).enter(|vm| {
        let code = vm.compile(&source, rustpython_vm::compiler::Mode::Exec, "<script>".to_owned())
            .map_err(|err| anyhow::anyhow!("Compiling script with RustPython: {:?}", err))?;
        let scope = vm.new_scope_with_builtins();
        vm.run_code_obj(code, scope)
            .map_err(|err| anyhow::anyhow!("Running script with RustPython: {:?}", err))
            .map(|_| ())
    })
}
