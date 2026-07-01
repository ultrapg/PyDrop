use anyhow::{Result, Context};
use std::path::PathBuf;
use crate::python_env::get_config;

/// RustPython backend packing – embeds the script directly into the binary.
#[cfg(feature = "rustpython")]
pub async fn pack_script(script: &PathBuf, output: &PathBuf) -> Result<()> {
    let config = get_config();
    let script_bytes = std::fs::read(script)
        .with_context(|| format!("Reading script {:?}", script))?;

    // Marker that the runtime will look for
    let marker = config.rustpython_marker.as_bytes();

    let current_exe = std::env::current_exe()?;
    let mut exe_bytes = std::fs::read(&current_exe)?;

    // Append marker, length, and script
    exe_bytes.extend_from_slice(marker);
    exe_bytes.extend_from_slice(&(script_bytes.len() as u64).to_le_bytes());
    exe_bytes.extend_from_slice(&script_bytes);

    std::fs::write(output, exe_bytes)?;
    println!("RustPython‑packed executable written to {}", output.display());
    Ok(())
}
