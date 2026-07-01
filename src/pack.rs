use anyhow::{Result, Context};

#[path = "pack_rustpython.rs"]
pub mod pack_rustpython;
use std::path::PathBuf;
use uuid::Uuid;
use crate::python_env::{ensure_python, pip_install, get_config};
use crate::run::detect_imports;
use std::io::Write;

/// Packs a Python script into a self‑extracting standalone CPython executable.
pub async fn pack_script(script: &PathBuf, output: &PathBuf) -> Result<()> {
    let config = get_config();
    let version = &config.python_version;
    let python_dir = ensure_python(version).await?;

    let env_dir = std::env::temp_dir().join(format!("pydrop_pack_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&env_dir)
        .with_context(|| format!("Creating pack env dir {:?}", env_dir))?;

    let imports = detect_imports(script)?;
    if !imports.is_empty() {
        println!("Installing packages for packing: {:?}", imports);
        pip_install(&python_dir, &env_dir, &imports)?;
    }

    let stage_dir = std::env::temp_dir().join(format!("pydrop_stage_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&stage_dir)?;

    for entry in std::fs::read_dir(&python_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let file_name = path.file_name().unwrap();
            let dest = stage_dir.join(file_name);
            std::fs::copy(&path, &dest)?;
        }
    }

    let script_name = script.file_name().unwrap();
    std::fs::copy(&script, stage_dir.join(script_name))?;

    let site_packages = env_dir.join("site-packages");
    if site_packages.exists() {
        let target_sp = stage_dir.join("Lib").join("site-packages");
        std::fs::create_dir_all(&target_sp)?;
        for entry in walkdir::WalkDir::new(&site_packages) {
            let entry = entry?;
            let rel_path = entry.path().strip_prefix(&site_packages)?;
            let dest_path = target_sp.join(rel_path);
            if entry.file_type().is_dir() {
                std::fs::create_dir_all(&dest_path)?;
            } else {
                if let Some(parent) = dest_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::copy(entry.path(), &dest_path)?;
            }
        }
    }

    // Write a marker file indicating this is a CP packed application
    std::fs::write(stage_dir.join("engine.txt"), b"cp")?;

    let zip_path = std::env::temp_dir().join(format!("pydrop_bundle_{}.zip", Uuid::new_v4()));
    let zip_file = std::fs::File::create(&zip_path)?;
    let mut zip = zip::ZipWriter::new(zip_file);
    let options = zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut buffer = Vec::new();
    for entry in walkdir::WalkDir::new(&stage_dir) {
        let entry = entry?;
        let path = entry.path();
        let name = path.strip_prefix(&stage_dir)?.to_string_lossy();
        if entry.file_type().is_file() {
            zip.start_file(name.clone(), options)?;
            std::io::Read::read_to_end(&mut std::fs::File::open(path)?, &mut buffer)?;
            zip.write_all(&buffer)?;
            buffer.clear();
        } else if entry.file_type().is_dir() {
            zip.add_directory(name.clone(), options)?;
        }
    }
    zip.finish()?;

    let current_exe = std::env::current_exe()?;
    let mut exe_bytes = std::fs::read(&current_exe)
        .with_context(|| format!("Failed to read current exe: {:?}", current_exe))?;
    let zip_bytes = std::fs::read(&zip_path)
        .with_context(|| format!("Failed to read bundle zip: {:?}", zip_path))?;
    exe_bytes.extend_from_slice(&zip_bytes);
    std::fs::write(output, exe_bytes)
        .with_context(|| format!("Failed to write packed executable output: {:?}", output))?;

    println!("Packed executable created at {}", output.display());
    Ok(())
}

/// Packs a Python script into a self-extracting standalone Local CPython executable.
pub async fn pack_local_script(script: &PathBuf, output: &PathBuf) -> Result<()> {
    let config = get_config();
    let version = &config.python_version;
    let python_dir = ensure_python(version).await?;
    let zip_path = python_dir.with_extension("zip");
    if !zip_path.exists() {
        println!("Downloading Python zip for packing...");
        let resp = reqwest::blocking::get(&config.python_download_url)
            .with_context(|| "Failed to download python zip for packing")?;
        let bytes = resp.bytes()?;
        std::fs::write(&zip_path, &bytes)?;
    }

    let env_dir = std::env::temp_dir().join(format!("pydrop_pack_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&env_dir)?;

    let imports = detect_imports(script)?;
    if !imports.is_empty() {
        println!("Installing packages for packing: {:?}", imports);
        pip_install(&python_dir, &env_dir, &imports)?;
    }

    let stage_dir = std::env::temp_dir().join(format!("pydrop_stage_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&stage_dir)?;

    // Copy python_runtime.zip to staging
    std::fs::copy(&zip_path, stage_dir.join("python_runtime.zip"))?;

    // Copy the script
    let script_name = script.file_name().unwrap();
    std::fs::copy(&script, stage_dir.join(script_name))?;

    // Copy installed packages (site-packages) if any
    let site_packages = env_dir.join("site-packages");
    if site_packages.exists() {
        let target_sp = stage_dir.join("site-packages");
        std::fs::create_dir_all(&target_sp)?;
        for entry in walkdir::WalkDir::new(&site_packages) {
            let entry = entry?;
            let rel_path = entry.path().strip_prefix(&site_packages)?;
            let dest_path = target_sp.join(rel_path);
            if entry.file_type().is_dir() {
                std::fs::create_dir_all(&dest_path)?;
            } else {
                if let Some(parent) = dest_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::copy(entry.path(), &dest_path)?;
            }
        }
    }

    // Write a marker file indicating this is a Local packed application
    std::fs::write(stage_dir.join("engine.txt"), b"local")?;

    let bundle_zip = std::env::temp_dir().join(format!("pydrop_bundle_{}.zip", Uuid::new_v4()));
    let zip_file = std::fs::File::create(&bundle_zip)?;
    let mut zip = zip::ZipWriter::new(zip_file);
    let options = zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut buffer = Vec::new();
    for entry in walkdir::WalkDir::new(&stage_dir) {
        let entry = entry?;
        let path = entry.path();
        let name = path.strip_prefix(&stage_dir)?.to_string_lossy();
        if entry.file_type().is_file() {
            zip.start_file(name.clone(), options)?;
            std::io::Read::read_to_end(&mut std::fs::File::open(path)?, &mut buffer)?;
            zip.write_all(&buffer)?;
            buffer.clear();
        } else if entry.file_type().is_dir() {
            zip.add_directory(name.clone(), options)?;
        }
    }
    zip.finish()?;

    let current_exe = std::env::current_exe()?;
    let mut exe_bytes = std::fs::read(&current_exe)
        .with_context(|| format!("Failed to read current exe: {:?}", current_exe))?;
    let zip_bytes = std::fs::read(&bundle_zip)
        .with_context(|| format!("Failed to read bundle zip: {:?}", bundle_zip))?;
    exe_bytes.extend_from_slice(&zip_bytes);
    std::fs::write(output, exe_bytes)
        .with_context(|| format!("Failed to write packed executable output: {:?}", output))?;

    println!("Packed Local executable created at {}", output.display());
    Ok(())
}
