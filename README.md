# PyDrop 🐍🦀

PyDrop is a portable Python execution and packaging utility written in Rust. It enables running Python scripts and interactive environments without pre-installing Python, resolves dependencies dynamically, and packages Python code into standalone, self-extracting executable binaries.

---

## Key Features

- ⚙️ **Multi-Engine Execution**:
  - **CPython (`cp`)**: Uses official, high-performance embeddable Python distributions (default version `3.11.9`) cached globally in user AppData.
  - **Local (`local`)**: Runs a fully self-contained Python runtime directory (`./python_env`) next to the script or binary.
  - **RustPython (`rustpython`)**: Uses an embedded, pure-Rust Python interpreter for sandboxed execution.
- 📦 **Automated Standalone Packaging**:
  - Pack scripts and environments into single, self-extracting `.exe` binaries.
  - Choose between caching globally (`cp`) or bundling a portable python directory (`local`) that extracts next to the application on double-click.
- 💻 **Standalone Portable Terminal (`terminal.exe`)**:
  - Automatically extracts a terminal helper on first run. Double-clicking `terminal.exe` launches an interactive command prompt preconfigured with `pydrop` in the `PATH` context—allowing full execution of `pydrop pip` and `pydrop python` anywhere without system environment variables.
- 🛠️ **Config-JSON Built-in Settings**:
  - Build constants, version settings, markers, and download URLs are offloaded to [config.json](file:///e:/PyDrop/config.json) which is compiled directly into the binary.
- 🐚 **Python REPL Shell**:
  - Run `pydrop` with no arguments, or run `pydrop shell` to launch a native interactive Python shell.

---

## How It Works Under the Hood

### 1. Robust Pip Bootstrapping
Because the official Windows embeddable Python zip excludes `ensurepip` and disables site directories by default to save space, PyDrop:
1. Automatically parses the custom Python runtime path configurations (`python*._pth`) and uncomments `import site` so dependencies can load.
2. Downloads `get-pip.py` to bootstrap a clean `pip` installation on the fly.
3. Automatically scans import statements and runs `pip install --target` to install dependencies in an isolated virtual environment.

### 2. Standalone Terminal Integration
During compile-time, a custom build script (`build.rs`) compiles `src/terminal.rs` and embeds it directly into the executable via `include_bytes!`. On launch, if `terminal.exe` is missing next to `pydrop.exe`, it is automatically extracted. This terminal prepends the executable's directory to the shell's environment `PATH` variables.

---

## CLI Command Reference

```text
PyDrop – portable Python executor

Usage: pydrop.exe [COMMAND]

Commands:
  run       Run a Python script
  pack      Pack a script into a standalone executable
  pip       Run pip commands using the cached/local environment
  python    Run python command or interactive REPL
  shell     Launch the interactive python shell directly
  terminal  Launch the portable terminal environment
  help      Print this message or the help of the given subcommand(s)
```

### Running Scripts
```bash
# Run with cached global CPython runtime (default)
pydrop run myscript.py

# Run with local standalone runtime (extracts/downloads ./python_env in script dir)
pydrop run myscript.py --engine local

# Run using RustPython VM
pydrop run myscript.py --engine rustpython
```

### Packaging standalone Executables
```bash
# Bundles script and site-packages; extracts runtime to user temp directories at launch
pydrop pack myscript.py -o app.exe

# Bundles python zip; extracts ./python_env folder next to the app on first run
pydrop pack myscript.py -o app.exe --engine local
```

### REPL and Pip Pass-Throughs
```bash
# Launch interactive REPL
pydrop shell

# Pass commands directly to the portable python/pip
pydrop pip install requests urllib3
pydrop python -c "import sys; print(sys.path)"
```

---

## File Structure
PyDrop\
├── build.rs                # Compiles `terminal.rs` and injects it into `pydrop`.
├── Cargo.toml
├── config.json
├── README.md
└── src\
    ├── main.rs             # Entry point, self-extraction detector, CLI command router, and terminal extractor.
    ├── pack.rs             # Orchestrates building self-extracting zip-based executables.
    ├── pack_cp.rs
    ├── pack_rustpython.rs
    ├── python_env.rs       # Bootstrapper for python, `get-pip.py`, paths, and local environment folders.
    ├── run.rs              # Dependency detector and script/REPL runner.
    └── terminal.rs         # Separate executable wrapper that runs `cmd.exe` with `PATH` modifications.

---

## License

GNU General Public License v3.0
