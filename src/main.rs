use clap::{Parser, Subcommand, ValueEnum};
use anyhow::Result;
use uuid::Uuid;
use std::path::Path;

mod python_env;
mod run;
mod pack;

#[derive(Parser)]
#[command(author, version, about = "PyDrop – portable Python executor", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a Python script
    Run {
        /// Path to the script
        script: std::path::PathBuf,
        /// Choose backend (default = cp)
        #[arg(long, value_enum, default_value = "cp")]
        engine: Engine,
    },
    /// Pack a script into a standalone executable
    Pack {
        /// Path to the script
        script: std::path::PathBuf,
        /// Output executable
        #[arg(short, long)]
        output: std::path::PathBuf,
        /// Choose backend (default = cp)
        #[arg(long, value_enum, default_value = "cp")]
        engine: Engine,
    },
    /// Run pip commands using the cached/local environment
    Pip {
        /// Arguments for pip
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
        /// Choose backend environment (default = cp)
        #[arg(long, value_enum, default_value = "cp")]
        engine: Engine,
    },
    /// Run python command or interactive REPL
    Python {
        /// Arguments for python
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
        /// Choose backend environment (default = cp)
        #[arg(long, value_enum, default_value = "cp")]
        engine: Engine,
    },
    /// Launch the interactive python shell directly
    Shell {
        /// Choose backend environment (default = cp)
        #[arg(long, value_enum, default_value = "cp")]
        engine: Engine,
    },
    /// Launch the portable terminal environment
    Terminal,
}

#[derive(Copy, Clone, ValueEnum)]
enum Engine {
    Cp,
    Local,
    Rustpython,
}

fn find_rustpython_embedded(exe_bytes: &[u8], marker: &[u8]) -> Option<String> {
    if exe_bytes.len() < marker.len() + 8 {
        return None;
    }
    let mut i = exe_bytes.len() - marker.len() - 8;
    while i > 0 {
        if &exe_bytes[i..(i + marker.len())] == marker {
            let len_bytes = &exe_bytes[(i + marker.len())..(i + marker.len() + 8)];
            let script_len = u64::from_le_bytes(len_bytes.try_into().unwrap()) as usize;
            if i + marker.len() + 8 + script_len == exe_bytes.len() {
                let script_bytes = &exe_bytes[(i + marker.len() + 8)..];
                return String::from_utf8(script_bytes.to_vec()).ok();
            }
        }
        i -= 1;
    }
    None
}

fn run_packed_cp(mut archive: zip::ZipArchive<std::fs::File>) -> Result<()> {
    let temp_dir = std::env::temp_dir().join(format!("pydrop_run_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir)?;
    
    let mut script_name = String::new();
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let outpath = temp_dir.join(file.sanitized_name());
        if (*file.name()).ends_with('/') {
            std::fs::create_dir_all(&outpath)?;
        } else {
            if let Some(p) = outpath.parent() {
                std::fs::create_dir_all(p)?;
            }
            let mut outfile = std::fs::File::create(&outpath)?;
            std::io::copy(&mut file, &mut outfile)?;
            if file.name().ends_with(".py") && !file.name().contains('/') {
                script_name = file.name().to_string();
            }
        }
    }
    
    let python_exe = temp_dir.join("python.exe");
    let script_path = temp_dir.join(&script_name);
    let site_packages = temp_dir.join("Lib").join("site-packages");
    
    let mut cmd_args = vec![script_path.to_string_lossy().into_owned()];
    cmd_args.extend(std::env::args().skip(1));

    let status = std::process::Command::new(python_exe)
        .args(&cmd_args)
        .env("PYTHONPATH", site_packages.to_str().unwrap())
        .status()?;
        
    let _ = std::fs::remove_dir_all(&temp_dir);
    
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}

async fn run_packed_local(mut archive: zip::ZipArchive<std::fs::File>, exe_dir: &Path) -> Result<()> {
    let config = python_env::get_config();
    let python_dir = exe_dir.join(&config.local_env_dir_name);
    if !python_dir.exists() {
        println!("Extracting embedded portable Python runtime...");
        std::fs::create_dir_all(&python_dir)?;
        let mut runtime_zip_file = archive.by_name("python_runtime.zip")?;
        let temp_runtime_zip = std::env::temp_dir().join(format!("pydrop_runtime_{}.zip", Uuid::new_v4()));
        let mut out_zip = std::fs::File::create(&temp_runtime_zip)?;
        std::io::copy(&mut runtime_zip_file, &mut out_zip)?;
        drop(out_zip);
        
        python_env::extract_zip_file(&temp_runtime_zip, &python_dir)?;
        let _ = std::fs::remove_file(&temp_runtime_zip);
        
        python_env::bootstrap_pip(&python_dir).await?;
    }
    
    let temp_run_dir = std::env::temp_dir().join(format!("pydrop_local_run_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&temp_run_dir)?;
    
    let mut script_name = String::new();
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        if file.name() == "python_runtime.zip" {
            continue;
        }
        let outpath = temp_run_dir.join(file.sanitized_name());
        if (*file.name()).ends_with('/') {
            std::fs::create_dir_all(&outpath)?;
        } else {
            if let Some(p) = outpath.parent() {
                std::fs::create_dir_all(p)?;
            }
            let mut outfile = std::fs::File::create(&outpath)?;
            std::io::copy(&mut file, &mut outfile)?;
            if file.name().ends_with(".py") && !file.name().contains('/') {
                script_name = file.name().to_string();
            }
        }
    }
    
    let python_exe = python_dir.join("python.exe");
    let script_path = temp_run_dir.join(&script_name);
    let site_packages = temp_run_dir.join("site-packages");
    
    let mut cmd_args = vec![script_path.to_string_lossy().into_owned()];
    cmd_args.extend(std::env::args().skip(1));

    let status = std::process::Command::new(python_exe)
        .args(&cmd_args)
        .env("PYTHONPATH", site_packages.to_str().unwrap())
        .status()?;
        
    let _ = std::fs::remove_dir_all(&temp_run_dir);
    
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}

fn launch_terminal(exe_dir: &Path) -> Result<()> {
    let terminal_path = exe_dir.join("terminal.exe");
    if !terminal_path.exists() {
        let terminal_bytes = include_bytes!(concat!(env!("OUT_DIR"), "/terminal.exe"));
        let _ = std::fs::write(&terminal_path, terminal_bytes);
    }
    let status = std::process::Command::new(&terminal_path).status()?;
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let current_exe = std::env::current_exe()?;
    let exe_dir = current_exe.parent().unwrap();

    // 1️⃣ Extract terminal.exe if missing next to current binary to keep it portable and ready
    let terminal_path = exe_dir.join("terminal.exe");
    if !terminal_path.exists() {
        let terminal_bytes = include_bytes!(concat!(env!("OUT_DIR"), "/terminal.exe"));
        let _ = std::fs::write(&terminal_path, terminal_bytes);
    }

    // Check if we are running as a packed executable or from standard commands
    let exe_bytes = std::fs::read(&current_exe)?;
    let config = python_env::get_config();

    // 2️⃣ Check for RustPython packed executable
    #[cfg(feature = "rustpython")]
    {
        if let Some(script_source) = find_rustpython_embedded(&exe_bytes, config.rustpython_marker.as_bytes()) {
            rustpython_vm::Interpreter::without_stdlib(Default::default()).enter(|vm| {
                let code = vm.compile(&script_source, rustpython_vm::compiler::Mode::Exec, "<script>".to_owned())
                    .expect("Failed compiling embedded RustPython script");
                let scope = vm.new_scope_with_builtins();
                if let Err(err) = vm.run_code_obj(code, scope) {
                    eprintln!("Error executing embedded RustPython script: {:?}", err);
                }
            });
            return Ok(());
        }
    }

    // 3️⃣ Check for CPython or Local packed zip executable
    if let Ok(file) = std::fs::File::open(&current_exe) {
        if let Ok(mut archive) = zip::ZipArchive::new(file) {
            let mut engine_type = String::new();
            let has_engine = if let Ok(mut engine_file) = archive.by_name("engine.txt") {
                use std::io::Read;
                let _ = engine_file.read_to_string(&mut engine_type);
                true
            } else {
                false
            };
            if has_engine {
                if engine_type == "cp" {
                    run_packed_cp(archive)?;
                    return Ok(());
                } else if engine_type == "local" {
                    run_packed_local(archive, exe_dir).await?;
                    return Ok(());
                }
            }
        }
    }

    // Normal command CLI path
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 1 {
        // Default behavior: launch python shell REPL
        let python_dir = python_env::ensure_python(&config.python_version).await?;
        run::run_interactive_shell(&python_dir)?;
        return Ok(());
    }

    let cli = Cli::parse();
    if let Some(command) = cli.command {
        match command {
            Commands::Run { script, engine } => match engine {
                Engine::Cp => run::run_cp_script(&script).await?,
                Engine::Local => run::run_local_script(&script).await?,
                Engine::Rustpython => {
                    #[cfg(feature = "rustpython")]
                    run::run_rustpython_script(&script).await?;
                    #[cfg(not(feature = "rustpython"))]
                    anyhow::bail!("RustPython engine feature is not enabled");
                }
            },
            Commands::Pack { script, output, engine } => match engine {
                Engine::Cp => pack::pack_script(&script, &output).await?,
                Engine::Local => pack::pack_local_script(&script, &output).await?,
                Engine::Rustpython => {
                    #[cfg(feature = "rustpython")]
                    pack::pack_rustpython::pack_script(&script, &output).await?;
                    #[cfg(not(feature = "rustpython"))]
                    anyhow::bail!("RustPython engine feature is not enabled");
                }
            },
            Commands::Pip { args, engine } => {
                let python_dir = match engine {
                    Engine::Cp => python_env::ensure_python(&config.python_version).await?,
                    Engine::Local => python_env::ensure_local_python(Path::new(".")).await?,
                    Engine::Rustpython => anyhow::bail!("RustPython engine does not support pip."),
                };
                let python_exe = python_dir.join("python.exe");
                let mut cmd_args = vec!["-m", "pip"];
                cmd_args.extend(args.iter().map(|s| s.as_str()));
                let status = std::process::Command::new(python_exe)
                    .args(&cmd_args)
                    .status()?;
                if !status.success() {
                    std::process::exit(status.code().unwrap_or(1));
                }
            },
            Commands::Python { args, engine } => {
                let python_dir = match engine {
                    Engine::Cp => python_env::ensure_python(&config.python_version).await?,
                    Engine::Local => python_env::ensure_local_python(Path::new(".")).await?,
                    Engine::Rustpython => anyhow::bail!("RustPython engine does not support running raw python CLI arguments."),
                };
                let python_exe = python_dir.join("python.exe");
                let status = std::process::Command::new(python_exe)
                    .args(&args)
                    .status()?;
                if !status.success() {
                    std::process::exit(status.code().unwrap_or(1));
                }
            },
            Commands::Shell { engine } => {
                let python_dir = match engine {
                    Engine::Cp => python_env::ensure_python(&config.python_version).await?,
                    Engine::Local => python_env::ensure_local_python(Path::new(".")).await?,
                    Engine::Rustpython => anyhow::bail!("RustPython engine does not support native REPL shell."),
                };
                run::run_interactive_shell(&python_dir)?;
            },
            Commands::Terminal => {
                launch_terminal(exe_dir)?;
            }
        }
    }
    Ok(())
}
