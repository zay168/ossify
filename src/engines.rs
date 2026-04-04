use std::env;
use std::fs;
use std::io;
use std::io::Cursor;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use flate2::read::GzDecoder;
use reqwest::blocking::Client;
use serde::Deserialize;
use serde::Serialize;
use tar::Archive;
use zip::ZipArchive;

const ACTIONLINT_REPO: &str = "rhysd/actionlint";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ManagedEngineStatus {
    Managed,
    HeuristicFallback,
    BootstrapFailed,
    RuntimeMissing,
    ExecutionFailed,
    ParseFailed,
}

impl ManagedEngineStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Managed => "managed",
            Self::HeuristicFallback => "heuristic-fallback",
            Self::BootstrapFailed => "bootstrap-failed",
            Self::RuntimeMissing => "runtime-missing",
            Self::ExecutionFailed => "execution-failed",
            Self::ParseFailed => "parse-failed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManagedTool {
    Actionlint,
    CargoDeny,
    AuditCi,
    PipAudit,
    ReleasePlz,
    GitCliff,
    CargoDist,
    ReleasePlease,
}

impl ManagedTool {
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Actionlint => "actionlint",
            Self::CargoDeny => "cargo-deny",
            Self::AuditCi => "audit-ci",
            Self::PipAudit => "pip-audit",
            Self::ReleasePlz => "release-plz",
            Self::GitCliff => "git-cliff",
            Self::CargoDist => "cargo-dist",
            Self::ReleasePlease => "release-please",
        }
    }

    pub fn command_name(self) -> &'static str {
        match self {
            Self::CargoDist => "dist",
            _ => self.display_name(),
        }
    }

    fn env_name(self) -> String {
        format!(
            "OSSIFY_{}",
            self.display_name().replace('-', "_").to_ascii_uppercase()
        )
    }

    fn install_kind(self) -> ManagedInstallKind {
        match self {
            Self::Actionlint => ManagedInstallKind::GitHubRelease,
            Self::CargoDeny | Self::ReleasePlz | Self::GitCliff | Self::CargoDist => {
                ManagedInstallKind::Cargo
            }
            Self::AuditCi | Self::ReleasePlease => ManagedInstallKind::Node,
            Self::PipAudit => ManagedInstallKind::Python,
        }
    }

    fn install_package(self) -> &'static str {
        match self {
            Self::CargoDeny => "cargo-deny",
            Self::AuditCi => "audit-ci",
            Self::PipAudit => "pip-audit",
            Self::ReleasePlz => "release-plz",
            Self::GitCliff => "git-cliff",
            Self::CargoDist => "cargo-dist",
            Self::ReleasePlease => "release-please",
            Self::Actionlint => "actionlint",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ManagedInstallKind {
    GitHubRelease,
    Cargo,
    Node,
    Python,
}

#[derive(Debug, Clone)]
pub struct ManagedEngineError {
    pub tool: ManagedTool,
    pub status: ManagedEngineStatus,
    pub message: String,
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
}

#[derive(Debug, Clone)]
struct PythonSeed {
    program: &'static str,
    prefix_args: &'static [&'static str],
}

pub fn run_tool(
    tool: ManagedTool,
    cwd: &Path,
    args: &[&str],
) -> Result<Output, ManagedEngineError> {
    if let Some(explicit_path) = env::var_os(tool.env_name()).map(PathBuf::from) {
        return run_path(&explicit_path, cwd, args).map_err(|error| ManagedEngineError {
            tool,
            status: ManagedEngineStatus::ExecutionFailed,
            message: format!(
                "{} was explicitly configured via {} but could not be executed: {}",
                tool.display_name(),
                tool.env_name(),
                error
            ),
        });
    }

    if let Ok(path) = managed_tool_path(tool) {
        if path.is_file() {
            return run_path(&path, cwd, args).map_err(|error| ManagedEngineError {
                tool,
                status: ManagedEngineStatus::ExecutionFailed,
                message: format!(
                    "managed {} exists at {} but could not be executed: {}",
                    tool.display_name(),
                    path.display(),
                    error
                ),
            });
        }
    }

    let auto_install = should_auto_install_engines();
    let mut bootstrap_error = None;
    if auto_install {
        match bootstrap_tool(tool) {
            Ok(installed_path) => {
                return run_path(&installed_path, cwd, args).map_err(|error| ManagedEngineError {
                    tool,
                    status: ManagedEngineStatus::ExecutionFailed,
                    message: format!(
                        "{} was bootstrapped to {} but still failed to execute: {}",
                        tool.display_name(),
                        installed_path.display(),
                        error
                    ),
                })
            }
            Err(error) => bootstrap_error = Some(error),
        }
    }

    match run_name(tool.command_name(), cwd, args) {
        Ok(output) => return Ok(output),
        Err(error) if error.kind() != io::ErrorKind::NotFound => {
            return Err(ManagedEngineError {
                tool,
                status: ManagedEngineStatus::ExecutionFailed,
                message: format!(
                    "{} was found on PATH but failed to execute: {}",
                    tool.display_name(),
                    error
                ),
            });
        }
        Err(_) => {}
    }

    if let Some(error) = bootstrap_error {
        return Err(error);
    }

    Err(ManagedEngineError {
        tool,
        status: ManagedEngineStatus::HeuristicFallback,
        message: format!(
            "{} is not available and automatic managed-engine bootstrap is disabled.",
            tool.display_name()
        ),
    })
}

pub fn should_auto_install_engines() -> bool {
    if cfg!(test) {
        return false;
    }

    match env::var("OSSIFY_AUTO_INSTALL_ENGINES") {
        Ok(value) => {
            let normalized = value.trim().to_ascii_lowercase();
            !matches!(normalized.as_str(), "0" | "false" | "no" | "off")
        }
        Err(_) => true,
    }
}

fn run_path(program: &Path, cwd: &Path, args: &[&str]) -> io::Result<Output> {
    Command::new(program).args(args).current_dir(cwd).output()
}

fn run_name(program: &str, cwd: &Path, args: &[&str]) -> io::Result<Output> {
    Command::new(program).args(args).current_dir(cwd).output()
}

fn bootstrap_tool(tool: ManagedTool) -> Result<PathBuf, ManagedEngineError> {
    let install_result = match tool.install_kind() {
        ManagedInstallKind::GitHubRelease => install_managed_actionlint(),
        ManagedInstallKind::Cargo => install_managed_cargo_tool(tool),
        ManagedInstallKind::Node => install_managed_node_tool(tool),
        ManagedInstallKind::Python => install_managed_python_tool(tool),
    };

    install_result.map_err(|error| {
        let status = if error.kind() == io::ErrorKind::NotFound {
            ManagedEngineStatus::RuntimeMissing
        } else {
            ManagedEngineStatus::BootstrapFailed
        };
        ManagedEngineError {
            tool,
            status,
            message: error.to_string(),
        }
    })
}

pub fn managed_tool_path(tool: ManagedTool) -> io::Result<PathBuf> {
    match tool.install_kind() {
        ManagedInstallKind::GitHubRelease | ManagedInstallKind::Cargo => {
            Ok(managed_bin_dir()?.join(executable_name(tool.command_name())))
        }
        ManagedInstallKind::Node => Ok(node_bin_dir()?.join(executable_name(tool.command_name()))),
        ManagedInstallKind::Python => {
            Ok(python_bin_dir()?.join(executable_name(tool.command_name())))
        }
    }
}

fn executable_name(name: &str) -> String {
    if cfg!(windows) {
        match name {
            "audit-ci" | "release-please" => format!("{name}.cmd"),
            other => format!("{other}.exe"),
        }
    } else {
        name.to_owned()
    }
}

fn managed_root_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    if let Some(path) = env::var_os("OSSIFY_TOOLS_DIR") {
        let path = PathBuf::from(path);
        dirs.push(
            path.file_name()
                .and_then(|name| name.to_str())
                .filter(|name| *name == "bin")
                .and_then(|_| path.parent().map(Path::to_path_buf))
                .unwrap_or(path),
        );
    }

    if let Ok(current_exe) = env::current_exe() {
        if let Some(bin_dir) = current_exe.parent() {
            if let Some(root) = bin_dir.parent() {
                dirs.push(root.join("tools"));
            }
        }
    }

    if cfg!(windows) {
        if let Some(local_app_data) = env::var_os("LOCALAPPDATA") {
            dirs.push(
                PathBuf::from(local_app_data)
                    .join("Programs")
                    .join("ossify")
                    .join("tools"),
            );
        }
    } else {
        if let Some(xdg_data_home) = env::var_os("XDG_DATA_HOME") {
            dirs.push(PathBuf::from(xdg_data_home).join("ossify").join("tools"));
        }
        if let Some(home) = env::var_os("HOME") {
            dirs.push(
                PathBuf::from(home)
                    .join(".local")
                    .join("share")
                    .join("ossify")
                    .join("tools"),
            );
        }
    }

    let mut unique = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for dir in dirs {
        let key = dir.to_string_lossy().into_owned();
        if seen.insert(key) {
            unique.push(dir);
        }
    }
    unique
}

fn primary_managed_root() -> io::Result<PathBuf> {
    let root = managed_root_dirs()
        .into_iter()
        .next()
        .ok_or_else(|| io::Error::other("could not resolve a managed tools root"))?;
    fs::create_dir_all(&root)?;
    Ok(root)
}

fn managed_bin_dir() -> io::Result<PathBuf> {
    let dir = primary_managed_root()?.join("bin");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn node_bin_dir() -> io::Result<PathBuf> {
    let dir = primary_managed_root()?
        .join("node")
        .join("node_modules")
        .join(".bin");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn node_sandbox_dir() -> io::Result<PathBuf> {
    let dir = primary_managed_root()?.join("node");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn python_venv_dir() -> io::Result<PathBuf> {
    let dir = primary_managed_root()?.join("python").join("venv");
    if let Some(parent) = dir.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(dir)
}

fn python_bin_dir() -> io::Result<PathBuf> {
    let dir = if cfg!(windows) {
        python_venv_dir()?.join("Scripts")
    } else {
        python_venv_dir()?.join("bin")
    };
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn install_managed_cargo_tool(tool: ManagedTool) -> io::Result<PathBuf> {
    if !command_ok("cargo", &["--version"]) {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "cargo is required to bootstrap managed Rust engines",
        ));
    }

    let root = primary_managed_root()?;
    let output = Command::new("cargo")
        .arg("install")
        .arg("--locked")
        .arg("--root")
        .arg(&root)
        .arg(tool.install_package())
        .output()?;

    if !output.status.success() {
        return Err(io::Error::other(format!(
            "cargo install for {} failed: {}",
            tool.display_name(),
            output_snippet(&output)
        )));
    }

    let installed = managed_tool_path(tool)?;
    if !installed.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "{} bootstrap completed but the managed binary was not found at {}",
                tool.display_name(),
                installed.display()
            ),
        ));
    }

    Ok(installed)
}

fn install_managed_node_tool(tool: ManagedTool) -> io::Result<PathBuf> {
    if !command_ok("node", &["--version"]) || !command_ok("npm", &["--version"]) {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Node.js and npm are required to bootstrap managed Node engines",
        ));
    }

    let sandbox = node_sandbox_dir()?;
    let package_json = sandbox.join("package.json");
    if !package_json.is_file() {
        fs::write(
            &package_json,
            "{\n  \"name\": \"ossify-managed-node-tools\",\n  \"private\": true\n}\n",
        )?;
    }

    let npm_program = if cfg!(windows) { "npm.cmd" } else { "npm" };
    let output = Command::new(npm_program)
        .arg("install")
        .arg("--no-audit")
        .arg("--no-fund")
        .arg("--save-dev")
        .arg("--save-exact")
        .arg(tool.install_package())
        .current_dir(&sandbox)
        .output()?;

    if !output.status.success() {
        return Err(io::Error::other(format!(
            "npm install for {} failed: {}",
            tool.display_name(),
            output_snippet(&output)
        )));
    }

    let installed = managed_tool_path(tool)?;
    if !installed.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "{} bootstrap completed but the managed executable was not found at {}",
                tool.display_name(),
                installed.display()
            ),
        ));
    }

    Ok(installed)
}

fn install_managed_python_tool(tool: ManagedTool) -> io::Result<PathBuf> {
    let seed = detect_python_seed().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "Python 3 is required to bootstrap managed Python engines",
        )
    })?;

    let venv_dir = python_venv_dir()?;
    let venv_python = venv_python_path(&venv_dir);
    if !venv_python.is_file() {
        let mut command = Command::new(seed.program);
        command
            .args(seed.prefix_args)
            .arg("-m")
            .arg("venv")
            .arg(&venv_dir);
        let output = command.output()?;
        if !output.status.success() {
            return Err(io::Error::other(format!(
                "failed to create managed Python sandbox: {}",
                output_snippet(&output)
            )));
        }
    }

    let output = Command::new(&venv_python)
        .arg("-m")
        .arg("pip")
        .arg("install")
        .arg("--disable-pip-version-check")
        .arg(tool.install_package())
        .output()?;

    if !output.status.success() {
        return Err(io::Error::other(format!(
            "pip install for {} failed: {}",
            tool.display_name(),
            output_snippet(&output)
        )));
    }

    let installed = managed_tool_path(tool)?;
    if !installed.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "{} bootstrap completed but the managed executable was not found at {}",
                tool.display_name(),
                installed.display()
            ),
        ));
    }

    Ok(installed)
}

fn detect_python_seed() -> Option<PythonSeed> {
    #[cfg(windows)]
    if command_ok("py", &["-3", "--version"]) {
        return Some(PythonSeed {
            program: "py",
            prefix_args: &["-3"],
        });
    }

    if command_ok("python", &["--version"]) {
        return Some(PythonSeed {
            program: "python",
            prefix_args: &[],
        });
    }
    if command_ok("python3", &["--version"]) {
        return Some(PythonSeed {
            program: "python3",
            prefix_args: &[],
        });
    }

    None
}

fn venv_python_path(venv_dir: &Path) -> PathBuf {
    if cfg!(windows) {
        venv_dir.join("Scripts").join("python.exe")
    } else {
        venv_dir.join("bin").join("python")
    }
}

fn command_ok(program: &str, args: &[&str]) -> bool {
    if Command::new(program)
        .args(args)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
    {
        return true;
    }

    #[cfg(windows)]
    {
        if !program.contains('.') {
            for candidate in [format!("{program}.exe"), format!("{program}.cmd")] {
                if Command::new(&candidate)
                    .args(args)
                    .output()
                    .map(|output| output.status.success())
                    .unwrap_or(false)
                {
                    return true;
                }
            }
        }
    }

    false
}

fn output_snippet(output: &Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}\n{stderr}");
    let snippet = combined
        .lines()
        .filter(|line| !line.trim().is_empty())
        .take(6)
        .collect::<Vec<_>>()
        .join(" | ");
    if snippet.is_empty() {
        String::from("no diagnostic output")
    } else {
        snippet
    }
}

fn install_managed_actionlint() -> io::Result<PathBuf> {
    let version = fetch_latest_github_release_version(ACTIONLINT_REPO)?;
    let (asset_name, binary_name) = actionlint_asset_for_current_platform(&version)?;
    let url =
        format!("https://github.com/{ACTIONLINT_REPO}/releases/download/v{version}/{asset_name}");
    let client = github_client()?;
    let archive = client
        .get(&url)
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(http_error)?
        .bytes()
        .map_err(http_error)?;

    let destination = managed_bin_dir()?.join(binary_name);

    if asset_name.ends_with(".zip") {
        extract_actionlint_zip(archive.as_ref(), binary_name, &destination)?;
    } else {
        extract_actionlint_targz(archive.as_ref(), binary_name, &destination)?;
    }

    Ok(destination)
}

fn fetch_latest_github_release_version(repository: &str) -> io::Result<String> {
    let client = github_client()?;
    let release: GitHubRelease = client
        .get(format!(
            "https://api.github.com/repos/{repository}/releases/latest"
        ))
        .header("Accept", "application/vnd.github+json")
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(http_error)?
        .json()
        .map_err(http_error)?;

    let version = release.tag_name.trim_start_matches('v').trim().to_owned();
    if version.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("latest release for {repository} did not include a usable tag"),
        ));
    }

    Ok(version)
}

fn actionlint_asset_for_current_platform(version: &str) -> io::Result<(String, &'static str)> {
    match (env::consts::OS, env::consts::ARCH) {
        ("windows", "x86_64") => Ok((
            format!("actionlint_{version}_windows_amd64.zip"),
            "actionlint.exe",
        )),
        ("linux", "x86_64") => Ok((
            format!("actionlint_{version}_linux_amd64.tar.gz"),
            "actionlint",
        )),
        ("macos", "x86_64") => Ok((
            format!("actionlint_{version}_darwin_amd64.tar.gz"),
            "actionlint",
        )),
        (os, arch) => Err(io::Error::other(format!(
            "automatic actionlint bootstrap does not yet support {os}/{arch}"
        ))),
    }
}

fn extract_actionlint_zip(
    archive_bytes: &[u8],
    binary_name: &str,
    destination: &Path,
) -> io::Result<()> {
    let mut archive = ZipArchive::new(Cursor::new(archive_bytes))
        .map_err(|error| io::Error::other(error.to_string()))?;

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| io::Error::other(error.to_string()))?;
        let Some(name) = Path::new(entry.name())
            .file_name()
            .and_then(|value| value.to_str())
        else {
            continue;
        };
        if name != binary_name {
            continue;
        }

        write_engine_file(destination, &mut entry)?;
        return Ok(());
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!("downloaded archive did not contain {binary_name}"),
    ))
}

fn extract_actionlint_targz(
    archive_bytes: &[u8],
    binary_name: &str,
    destination: &Path,
) -> io::Result<()> {
    let decoder = GzDecoder::new(Cursor::new(archive_bytes));
    let mut archive = Archive::new(decoder);
    let entries = archive
        .entries()
        .map_err(|error| io::Error::other(error.to_string()))?;

    for entry in entries {
        let mut entry = entry.map_err(|error| io::Error::other(error.to_string()))?;
        let path = entry
            .path()
            .map_err(|error| io::Error::other(error.to_string()))?;
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if name != binary_name {
            continue;
        }

        write_engine_file(destination, &mut entry)?;
        return Ok(());
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!("downloaded archive did not contain {binary_name}"),
    ))
}

fn write_engine_file(destination: &Path, reader: &mut impl io::Read) -> io::Result<()> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }

    let temp_path = destination.with_extension("download");
    {
        let mut file = fs::File::create(&temp_path)?;
        io::copy(reader, &mut file)?;
    }
    set_executable_permissions(&temp_path)?;
    replace_file(&temp_path, destination)?;
    Ok(())
}

fn replace_file(source: &Path, destination: &Path) -> io::Result<()> {
    if destination.exists() {
        fs::remove_file(destination)?;
    }
    fs::rename(source, destination)
}

#[cfg(unix)]
fn set_executable_permissions(path: &Path) -> io::Result<()> {
    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
}

#[cfg(not(unix))]
fn set_executable_permissions(_path: &Path) -> io::Result<()> {
    Ok(())
}

fn github_client() -> io::Result<Client> {
    Client::builder()
        .user_agent(format!("ossify/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|error| io::Error::other(error.to_string()))
}

fn http_error(error: reqwest::Error) -> io::Error {
    io::Error::other(error.to_string())
}
