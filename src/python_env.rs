use std::path::{Path, PathBuf};
use anyhow::{Result, Context};
use std::fs;
use std::process::Command;
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct AppConfig {
    pub python_version: String,
    pub python_download_url: String,
    pub rustpython_marker: String,
    pub local_env_dir_name: String,
}

pub fn get_config() -> AppConfig {
    let config_str = include_str!("../config.json");
    serde_json::from_str(config_str).expect("Failed to parse embedded config.json")
}

/// Directory where downloaded Python runtimes are stored per version in cache.
pub fn python_runtime_dir(version: &str) -> PathBuf {
    let mut base = dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("pydrop");
    base.push("python");
    base.push(version);
    base
}

/// Helper to download and extract Python to a target directory.
pub async fn download_and_extract_python(url: &str, target_dir: &Path) -> Result<()> {
    fs::create_dir_all(target_dir).with_context(|| format!("Creating target dir {:?}", target_dir))?;
    let zip_path = target_dir.with_extension("zip");
    
    // Download
    println!("Downloading Python from {}...", url);
    let resp = reqwest::blocking::get(url)
        .with_context(|| format!("Failed to GET {}", url))?;
    let bytes = resp.bytes().with_context(|| "Failed to read response bytes")?;
    fs::write(&zip_path, &bytes).with_context(|| format!("Writing zip to {:?}", zip_path))?;
    
    // Extract
    extract_zip_file(&zip_path, target_dir)?;
    
    // Keep the zip file in the cache so we can use it for Local engine packing

    // Bootstrap pip
    bootstrap_pip(target_dir).await?;
    Ok(())
}

pub async fn bootstrap_pip(python_dir: &Path) -> Result<()> {
    // 1. Uncomment "import site" in the ._pth file
    for entry in fs::read_dir(python_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map_or(false, |ext| ext == "_pth") {
            let content = fs::read_to_string(&path)?;
            let new_content = content.replace("#import site", "import site")
                                     .replace("# import site", "import site");
            fs::write(&path, new_content)?;
        }
    }

    // 2. Download get-pip.py
    let get_pip_url = "https://bootstrap.pypa.io/get-pip.py";
    let get_pip_path = python_dir.join("get-pip.py");
    println!("Downloading get-pip.py to bootstrap pip...");
    let resp = reqwest::blocking::get(get_pip_url)
        .with_context(|| "Failed to download get-pip.py")?;
    let bytes = resp.bytes()?;
    fs::write(&get_pip_path, &bytes)?;

    // 3. Run python.exe get-pip.py
    let python_exe = python_dir.join("python.exe");
    let status = Command::new(&python_exe)
        .arg(&get_pip_path)
        .status()
        .with_context(|| "Failed to run get-pip.py")?;
        
    // Clean up get-pip.py
    let _ = fs::remove_file(&get_pip_path);

    if !status.success() {
        anyhow::bail!("get-pip.py failed with status {:?}", status);
    }
    Ok(())
}

pub fn extract_zip_file(zip_path: &Path, target_dir: &Path) -> Result<()> {
    let file = fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let outpath = target_dir.join(file.sanitized_name());
        if (*file.name()).ends_with('/') {
            fs::create_dir_all(&outpath)?;
        } else {
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    fs::create_dir_all(p)?;
                }
            }
            let mut outfile = fs::File::create(&outpath)?;
            std::io::copy(&mut file, &mut outfile)?;
        }
    }
    Ok(())
}

/// Ensure that the specified Python version is present. If not, download the embeddable zip and extract.
pub async fn ensure_python(version: &str) -> Result<PathBuf> {
    let dir = python_runtime_dir(version);
    if dir.exists() {
        return Ok(dir);
    }
    let config = get_config();
    download_and_extract_python(&config.python_download_url, &dir).await?;
    Ok(dir)
}

/// Ensure local python environment is present in the specified directory.
pub async fn ensure_local_python(base_dir: &Path) -> Result<PathBuf> {
    let config = get_config();
    let dir = base_dir.join(&config.local_env_dir_name);
    if dir.exists() {
        return Ok(dir);
    }
    download_and_extract_python(&config.python_download_url, &dir).await?;
    Ok(dir)
}

/// Install pip packages into a provided virtual‑env directory.
pub fn pip_install(python_dir: &Path, env_dir: &Path, packages: &[String]) -> Result<()> {
    let python = python_dir.join("python.exe");
    // Ensure venv exists (use --target to install into env_dir)
    let mut args = vec!["-m", "pip", "install", "--target", env_dir.to_str().unwrap()];
    args.extend(packages.iter().map(|s| s.as_str()));
    let status = Command::new(python).args(&args).status()?;
    if !status.success() {
        anyhow::bail!("pip install failed");
    }
    Ok(())
}
