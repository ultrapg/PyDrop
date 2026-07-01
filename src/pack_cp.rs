use anyhow::{Result, Context};
use std::path::{Path, PathBuf};
use uuid::Uuid;
use walkdir::WalkDir;
use crate::python_env::{ensure_python, pip_install};
use crate::run::detect_imports;

/// CPython backend packing – creates a self‑extracting executable.
#[cfg(feature = "cp")]
pub async fn pack_script(script: &PathBuf, output: &PathBuf) -> Result<()> {
    // 1️⃣ Ensure Python runtime (default 3.11)
    let version = "3.11";
    let python_dir = ensure_python(version).await?;

    // 2️⃣ Temporary virtual‑env for packaging
    let env_dir = std::env::temp_dir()
        .join(format!("rustpy_pack_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&env_dir)?;

    // 3️⃣ Detect imports & install packages
    let imports = detect_imports(script)?;
    if !imports.is_empty() {
        println!("Installing packages for packing: {:?}", imports);
        pip_install(&python_dir, &env_dir, &imports)?;
    }

    // 4️⃣ Stage files (python runtime, script, site‑packages)
    let stage_dir = std::env::temp_dir()
        .join(format!("rustpy_stage_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&stage_dir)?;

    // Copy python runtime files
    for entry in std::fs::read_dir(&python_dir)? {
        let entry = entry?;
        let src = entry.path();
        let dst = stage_dir.join(src.file_name().unwrap());
        std::fs::copy(&src, &dst)?;
    }

    // Copy the script
    let script_name = script.file_name().unwrap();
    std::fs::copy(&script, stage_dir.join(script_name))?;

    // Copy installed site‑packages
    let site = env_dir.join("site-packages");
    if site.exists() {
        let target = stage_dir.join("Lib").join("site-packages");
        std::fs::create_dir_all(&target)?;
        for entry in WalkDir::new(&site) {
            let entry = entry?;
            let rel = entry.path().strip_prefix(&site)?;
            let dest = target.join(rel);
            if entry.file_type().is_dir() {
                std::fs::create_dir_all(&dest)?;
            } else {
                if let Some(p) = dest.parent() {
                    std::fs::create_dir_all(p)?;
                }
                std::fs::copy(entry.path(), &dest)?;
            }
        }
    }

    // 5️⃣ Create zip archive
    let zip_path = std::env::temp_dir()
        .join(format!("rustpy_bundle_{}.zip", Uuid::new_v4()));
    let zip_file = std::fs::File::create(&zip_path)?;
    let mut zip = zip::ZipWriter::new(zip_file);
    let opts = zip::write::FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let mut buffer = Vec::new();
    for entry in WalkDir::new(&stage_dir) {
        let entry = entry?;
        let path = entry.path();
        let name = path.strip_prefix(&stage_dir)?.to_string_lossy();
        if entry.file_type().is_file() {
            zip.start_file(name.clone(), opts)?;
            std::io::Read::read_to_end(&mut std::fs::File::open(path)?, &mut buffer)?;
            zip.write_all(&buffer)?;
            buffer.clear();
        } else if entry.file_type().is_dir() {
            zip.add_directory(name.clone(), opts)?;
        }
    }
    zip.finish()?;

    // 6️⃣ Append zip to current binary (self‑extracting)
    let current = std::env::current_exe()?;
    let mut exe_bytes = std::fs::read(&current)?;
    let zip_bytes = std::fs::read(&zip_path)?;
    exe_bytes.extend_from_slice(&zip_bytes);
    std::fs::write(output, exe_bytes)?;

    println!("Packed executable created at {}", output.display());
    Ok(())
}
