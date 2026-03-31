use std::collections::{BTreeSet, HashMap};
use std::env;
use std::fs;
use std::io;
use std::io::Cursor;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use flate2::read::GzDecoder;
use ignore::WalkBuilder;
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use reqwest::blocking::Client;
use serde::Deserialize;
use serde::Serialize;
use serde_yaml::{Mapping, Value};
use tar::Archive;
use zip::ZipArchive;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DoctorSeverity {
    Error,
    Warning,
    Info,
}

impl DoctorSeverity {
    fn penalty(self) -> u16 {
        match self {
            Self::Error => 20,
            Self::Warning => 10,
            Self::Info => 4,
        }
    }

    fn rank(self) -> u8 {
        match self {
            Self::Error => 0,
            Self::Warning => 1,
            Self::Info => 2,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DoctorFinding {
    pub severity: DoctorSeverity,
    pub code: String,
    pub message: String,
    pub file: Option<PathBuf>,
    pub help: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DocsDoctorReport {
    pub target: PathBuf,
    pub score: u8,
    pub markdown_files: usize,
    pub local_links_checked: usize,
    pub findings: Vec<DoctorFinding>,
}

impl DocsDoctorReport {
    pub fn error_count(&self) -> usize {
        self.findings
            .iter()
            .filter(|finding| finding.severity == DoctorSeverity::Error)
            .count()
    }

    pub fn warning_count(&self) -> usize {
        self.findings
            .iter()
            .filter(|finding| finding.severity == DoctorSeverity::Warning)
            .count()
    }

    pub fn info_count(&self) -> usize {
        self.findings
            .iter()
            .filter(|finding| finding.severity == DoctorSeverity::Info)
            .count()
    }

    pub fn summary(&self) -> String {
        match (self.error_count(), self.warning_count()) {
            (0, 0) => String::from("Documentation surface looks healthy."),
            (0, warnings) => format!("Documentation surface has {warnings} warning(s)."),
            (errors, 0) => format!("Documentation surface has {errors} error(s)."),
            (errors, warnings) => {
                format!("Documentation surface has {errors} error(s) and {warnings} warning(s).")
            }
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkflowDoctorReport {
    pub target: PathBuf,
    pub score: Option<u8>,
    pub workflow_files: usize,
    pub engine: String,
    pub engine_available: bool,
    pub findings: Vec<DoctorFinding>,
}

impl WorkflowDoctorReport {
    pub fn error_count(&self) -> usize {
        self.findings
            .iter()
            .filter(|finding| finding.severity == DoctorSeverity::Error)
            .count()
    }

    pub fn warning_count(&self) -> usize {
        self.findings
            .iter()
            .filter(|finding| finding.severity == DoctorSeverity::Warning)
            .count()
    }

    pub fn info_count(&self) -> usize {
        self.findings
            .iter()
            .filter(|finding| finding.severity == DoctorSeverity::Info)
            .count()
    }

    pub fn summary(&self) -> String {
        if !self.engine_available {
            if self
                .findings
                .iter()
                .any(|finding| finding.code == "workflow.engine-bootstrap-failed")
            {
                return String::from(
                    "Workflow doctor could not bootstrap actionlint automatically.",
                );
            }
            return String::from("Workflow doctor needs actionlint to run external checks.");
        }
        if self.workflow_files == 0 {
            return String::from("No GitHub Actions workflow files were found.");
        }
        match (self.error_count(), self.warning_count(), self.info_count()) {
            (0, 0, 0) => String::from(
                "Workflow surface looks healthy under actionlint and ossify hygiene checks.",
            ),
            (0, 0, infos) => {
                format!(
                    "Workflow surface passes actionlint but still has {infos} hygiene signal(s)."
                )
            }
            (0, warnings, infos) => {
                let mut parts = Vec::new();
                if warnings > 0 {
                    parts.push(format!("{warnings} warning(s)"));
                }
                if infos > 0 {
                    parts.push(format!("{infos} info signal(s)"));
                }
                format!(
                    "Workflow surface passes actionlint but still has {}.",
                    parts.join(" and ")
                )
            }
            (errors, _, _) => format!("Workflow surface has {errors} actionlint error(s)."),
        }
    }
}

#[derive(Debug, Clone)]
struct MarkdownAnalysis {
    path: PathBuf,
    headings: BTreeSet<String>,
    raw_headings: Vec<String>,
    links: Vec<String>,
    code_blocks: usize,
}

const ACTIONLINT_REPO: &str = "rhysd/actionlint";

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
}

pub fn doctor_docs(root: &Path) -> io::Result<DocsDoctorReport> {
    let target = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let markdown_files = collect_markdown_files(&target)?;
    let mut analyses = Vec::new();
    let mut headings_by_path = HashMap::new();

    for path in &markdown_files {
        let contents = fs::read_to_string(path)?;
        let analysis = analyze_markdown(path, &contents);
        headings_by_path.insert(path.clone(), analysis.headings.clone());
        analyses.push(analysis);
    }

    let mut findings = Vec::new();
    let mut local_links_checked = 0usize;
    let readme_path = target.join("README.md");
    let readme = analyses
        .iter()
        .find(|analysis| analysis.path == readme_path);

    if readme.is_none() {
        findings.push(DoctorFinding {
            severity: DoctorSeverity::Error,
            code: String::from("readme.missing"),
            message: String::from("README.md is missing."),
            file: Some(readme_path.clone()),
            help: Some(String::from(
                "Add a root README so the project has a clear entry point for install and usage.",
            )),
        });
    }

    if let Some(readme) = readme {
        if !has_heading(&readme.raw_headings, &["install", "installation"]) {
            findings.push(DoctorFinding {
                severity: DoctorSeverity::Warning,
                code: String::from("readme.install"),
                message: String::from("README.md has no obvious install section."),
                file: Some(readme.path.clone()),
                help: Some(String::from(
                    "Add an Install or Installation section with one copy-pasteable command.",
                )),
            });
        }

        if !has_heading(
            &readme.raw_headings,
            &["usage", "quickstart", "examples", "getting started"],
        ) {
            findings.push(DoctorFinding {
                severity: DoctorSeverity::Warning,
                code: String::from("readme.usage"),
                message: String::from("README.md has no obvious usage or examples section."),
                file: Some(readme.path.clone()),
                help: Some(String::from(
                    "Add a Usage, Examples, or Quickstart section so readers can try the tool immediately.",
                )),
            });
        }

        if readme.code_blocks == 0 {
            findings.push(DoctorFinding {
                severity: DoctorSeverity::Info,
                code: String::from("readme.codeblocks"),
                message: String::from("README.md has no fenced code blocks."),
                file: Some(readme.path.clone()),
                help: Some(String::from(
                    "Add at least one copy-pasteable command example to make the README more actionable.",
                )),
            });
        }
    }

    for analysis in &analyses {
        for destination in &analysis.links {
            let Some(link) = parse_local_link(destination) else {
                continue;
            };
            local_links_checked += 1;

            let target_path = if let Some(relative_path) = link.path.as_deref() {
                normalize_path(
                    analysis
                        .path
                        .parent()
                        .unwrap_or(target.as_path())
                        .join(relative_path),
                )
            } else {
                analysis.path.clone()
            };

            if !target_path.exists() {
                findings.push(DoctorFinding {
                    severity: DoctorSeverity::Error,
                    code: String::from("docs.broken-link"),
                    message: format!("Broken local link `{destination}`."),
                    file: Some(analysis.path.clone()),
                    help: Some(String::from(
                        "Create the missing file or update the link destination so local docs stay navigable.",
                    )),
                });
                continue;
            }

            if let Some(anchor) = link.anchor.as_deref() {
                if is_markdown_file(&target_path) {
                    let slug = slugify_heading(anchor);
                    let headings = headings_by_path.get(&target_path);
                    let has_anchor = headings
                        .map(|values| values.contains(&slug))
                        .unwrap_or(false);

                    if !has_anchor {
                        findings.push(DoctorFinding {
                            severity: DoctorSeverity::Error,
                            code: String::from("docs.missing-anchor"),
                            message: format!(
                                "Link `{destination}` points to an anchor that does not exist."
                            ),
                            file: Some(analysis.path.clone()),
                            help: Some(String::from(
                                "Update the fragment or add the matching heading in the target Markdown file.",
                            )),
                        });
                    }
                }
            }
        }
    }

    findings.sort_by(|left, right| {
        left.severity
            .rank()
            .cmp(&right.severity.rank())
            .then_with(|| left.file.cmp(&right.file))
            .then_with(|| left.code.cmp(&right.code))
    });

    let penalty = findings
        .iter()
        .map(|finding| finding.severity.penalty())
        .sum::<u16>();
    let score = 100u16.saturating_sub(penalty).min(100) as u8;

    Ok(DocsDoctorReport {
        target,
        score,
        markdown_files: markdown_files.len(),
        local_links_checked,
        findings,
    })
}

pub fn doctor_workflow(root: &Path) -> io::Result<WorkflowDoctorReport> {
    let target = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let workflow_files = collect_workflow_files(&target)?;
    if workflow_files.is_empty() {
        return Ok(WorkflowDoctorReport {
            target,
            score: Some(100),
            workflow_files: 0,
            engine: String::from("actionlint"),
            engine_available: true,
            findings: Vec::new(),
        });
    }

    let mut actionlint_program =
        resolve_engine_binary("actionlint").unwrap_or_else(|| PathBuf::from("actionlint"));

    let output = match run_actionlint(&actionlint_program, &target) {
        Ok(output) => output,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            if !should_auto_install_engines() {
                return Ok(workflow_engine_missing_report(
                    &target,
                    workflow_files.len(),
                ));
            }

            actionlint_program = match ensure_actionlint_binary() {
                Ok(path) => path,
                Err(error) => {
                    return Ok(workflow_engine_bootstrap_failed_report(
                        &target,
                        workflow_files.len(),
                        &error,
                    ));
                }
            };

            match run_actionlint(&actionlint_program, &target) {
                Ok(output) => output,
                Err(error) => {
                    return Ok(workflow_engine_bootstrap_failed_report(
                        &target,
                        workflow_files.len(),
                        &error,
                    ));
                }
            }
        }
        Err(error) => return Err(error),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed = parse_actionlint_errors(stdout.trim()).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("failed to parse actionlint output: {error}"),
        )
    })?;
    let mut findings = parsed
        .into_iter()
        .map(|entry| {
            let kind = entry.kind.clone().unwrap_or_else(|| String::from("lint"));
            DoctorFinding {
                severity: DoctorSeverity::Error,
                code: format!("workflow.{kind}"),
                message: format_actionlint_message(&entry),
                file: (!entry.filepath.is_empty())
                    .then(|| normalize_path(target.join(entry.filepath))),
                help: None,
            }
        })
        .collect::<Vec<_>>();

    findings.extend(workflow_hygiene_findings(&target, &workflow_files)?);
    findings.sort_by(|left, right| {
        left.severity
            .rank()
            .cmp(&right.severity.rank())
            .then_with(|| left.file.cmp(&right.file))
            .then_with(|| left.code.cmp(&right.code))
    });

    let score = workflow_score(&findings);

    Ok(WorkflowDoctorReport {
        target,
        score: Some(score),
        workflow_files: workflow_files.len(),
        engine: String::from("actionlint"),
        engine_available: true,
        findings,
    })
}

fn run_actionlint(actionlint_program: &Path, target: &Path) -> io::Result<std::process::Output> {
    Command::new(actionlint_program)
        .arg("-format")
        .arg("{{json .}}")
        .current_dir(target)
        .output()
}

fn workflow_engine_missing_report(target: &Path, workflow_files: usize) -> WorkflowDoctorReport {
    WorkflowDoctorReport {
        target: target.to_path_buf(),
        score: None,
        workflow_files,
        engine: String::from("actionlint"),
        engine_available: false,
        findings: vec![DoctorFinding {
            severity: DoctorSeverity::Info,
            code: String::from("workflow.engine-missing"),
            message: String::from(
                "actionlint is not installed, so external workflow checks could not run.",
            ),
            file: None,
            help: Some(String::from(
                "Run the public ossify installer or set OSSIFY_ACTIONLINT to an existing actionlint binary, then rerun `ossify doctor workflow .`.",
            )),
        }],
    }
}

fn workflow_engine_bootstrap_failed_report(
    target: &Path,
    workflow_files: usize,
    error: &io::Error,
) -> WorkflowDoctorReport {
    WorkflowDoctorReport {
        target: target.to_path_buf(),
        score: None,
        workflow_files,
        engine: String::from("actionlint"),
        engine_available: false,
        findings: vec![DoctorFinding {
            severity: DoctorSeverity::Warning,
            code: String::from("workflow.engine-bootstrap-failed"),
            message: String::from(
                "actionlint was missing and automatic bootstrap did not complete.",
            ),
            file: None,
            help: Some(format!(
                "Bootstrap error: {error}. Rerun the public ossify installer or set OSSIFY_ACTIONLINT to an existing actionlint binary, then retry `ossify doctor workflow .`."
            )),
        }],
    }
}

fn workflow_hygiene_findings(
    _root: &Path,
    workflow_files: &[PathBuf],
) -> io::Result<Vec<DoctorFinding>> {
    let mut findings = Vec::new();

    for path in workflow_files {
        let contents = fs::read_to_string(path)?;
        let parsed: Value = match serde_yaml::from_str(&contents) {
            Ok(value) => value,
            Err(_) => continue,
        };

        let Some(document) = parsed.as_mapping() else {
            continue;
        };

        if !document.contains_key(Value::String(String::from("permissions"))) {
            findings.push(DoctorFinding {
                severity: DoctorSeverity::Warning,
                code: String::from("workflow.permissions.missing"),
                message: String::from(
                    "Workflow does not declare explicit GITHUB_TOKEN permissions.",
                ),
                file: Some(path.clone()),
                help: Some(String::from(
                    "Add a top-level `permissions:` block with the narrowest access your jobs need, usually `contents: read` for CI.",
                )),
            });
        }

        if !document.contains_key(Value::String(String::from("concurrency"))) {
            findings.push(DoctorFinding {
                severity: DoctorSeverity::Info,
                code: String::from("workflow.concurrency.missing"),
                message: String::from(
                    "Workflow has no concurrency guard, so superseded runs can continue burning minutes.",
                ),
                file: Some(path.clone()),
                help: Some(String::from(
                    "Add `concurrency` when duplicate runs should cancel in progress, especially for CI and release-related workflows.",
                )),
            });
        }

        let mut timeout_missing_jobs = Vec::new();
        if let Some(jobs) = document
            .get(Value::String(String::from("jobs")))
            .and_then(Value::as_mapping)
        {
            for (job_name, job_value) in jobs {
                let Some(job_id) = job_name.as_str() else {
                    continue;
                };
                let Some(job) = job_value.as_mapping() else {
                    continue;
                };

                if !job.contains_key(Value::String(String::from("timeout-minutes"))) {
                    timeout_missing_jobs.push(job_id.to_owned());
                }
            }
        }

        if !timeout_missing_jobs.is_empty() {
            timeout_missing_jobs.sort();
            findings.push(DoctorFinding {
                severity: DoctorSeverity::Info,
                code: String::from("workflow.timeout.missing"),
                message: format!(
                    "missing `timeout-minutes` on jobs: {}",
                    timeout_missing_jobs.join(", ")
                ),
                file: Some(path.clone()),
                help: Some(String::from(
                    "Add `timeout-minutes:` to long-running jobs so failures fail fast instead of hanging.",
                )),
            });
        }

        let mutable_refs = collect_mutable_action_refs(document);
        if !mutable_refs.is_empty() {
            findings.push(DoctorFinding {
                severity: DoctorSeverity::Info,
                code: String::from("workflow.actions.unpinned"),
                message: format!(
                    "Workflow uses mutable action refs instead of full commit SHAs: {}.",
                    mutable_refs.join(", ")
                ),
                file: Some(path.clone()),
                help: Some(String::from(
                    "Pin action refs to full commit SHAs for stronger supply-chain reproducibility, then optionally document the friendly tag in comments.",
                )),
            });
        }
    }
    Ok(findings)
}

fn collect_mutable_action_refs(document: &Mapping) -> Vec<String> {
    let mut refs = BTreeSet::new();
    collect_mutable_action_refs_from_value(&Value::Mapping(document.clone()), &mut refs);
    refs.into_iter().collect()
}

fn collect_mutable_action_refs_from_value(value: &Value, refs: &mut BTreeSet<String>) {
    match value {
        Value::Mapping(mapping) => {
            for (key, child) in mapping {
                if key.as_str() == Some("uses") {
                    if let Some(action_ref) = child.as_str() {
                        if is_mutable_action_ref(action_ref)
                            && !is_official_github_action_ref(action_ref)
                        {
                            refs.insert(action_ref.to_owned());
                        }
                    }
                }
                collect_mutable_action_refs_from_value(child, refs);
            }
        }
        Value::Sequence(values) => {
            for child in values {
                collect_mutable_action_refs_from_value(child, refs);
            }
        }
        _ => {}
    }
}

fn is_mutable_action_ref(action_ref: &str) -> bool {
    if action_ref.starts_with("./") || action_ref.starts_with("docker://") {
        return false;
    }

    let Some((_, reference)) = action_ref.rsplit_once('@') else {
        return false;
    };
    !(reference.len() == 40
        && reference
            .chars()
            .all(|character| character.is_ascii_hexdigit()))
}

fn is_official_github_action_ref(action_ref: &str) -> bool {
    action_ref.starts_with("actions/")
}

fn workflow_penalty(finding: &DoctorFinding) -> u16 {
    match finding.code.as_str() {
        "workflow.permissions.missing" => 8,
        "workflow.actions.unpinned" => 3,
        "workflow.concurrency.missing" | "workflow.timeout.missing" => 2,
        "workflow.engine-bootstrap-failed" => 12,
        "workflow.engine-missing" => 8,
        _ => finding.severity.penalty(),
    }
}

fn workflow_score(findings: &[DoctorFinding]) -> u8 {
    let penalty = findings.iter().map(workflow_penalty).sum::<u16>();
    let mut score = 100u16.saturating_sub(penalty).min(100) as u8;

    if findings
        .iter()
        .any(|finding| finding.code == "workflow.syntax-check")
    {
        score = score.min(49);
    } else if findings
        .iter()
        .any(|finding| finding.severity == DoctorSeverity::Error)
    {
        score = score.min(69);
    }

    score
}

fn resolve_engine_binary(engine: &str) -> Option<PathBuf> {
    let env_name = format!("OSSIFY_{}", engine.replace('-', "_").to_ascii_uppercase());
    if let Some(path) = env::var_os(&env_name) {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }

    let executable = if cfg!(windows) {
        format!("{engine}.exe")
    } else {
        engine.to_owned()
    };

    for dir in managed_engine_dirs() {
        let candidate = dir.join(&executable);
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    None
}

fn managed_engine_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    if let Some(path) = env::var_os("OSSIFY_TOOLS_DIR") {
        dirs.push(PathBuf::from(path));
    }

    if let Ok(current_exe) = env::current_exe() {
        if let Some(bin_dir) = current_exe.parent() {
            if let Some(root) = bin_dir.parent() {
                dirs.push(root.join("tools").join("bin"));
            }
        }
    }

    if cfg!(windows) {
        if let Some(local_app_data) = env::var_os("LOCALAPPDATA") {
            dirs.push(
                PathBuf::from(local_app_data)
                    .join("Programs")
                    .join("ossify")
                    .join("tools")
                    .join("bin"),
            );
        }
    } else {
        if let Some(xdg_data_home) = env::var_os("XDG_DATA_HOME") {
            dirs.push(
                PathBuf::from(xdg_data_home)
                    .join("ossify")
                    .join("tools")
                    .join("bin"),
            );
        }
        if let Some(home) = env::var_os("HOME") {
            dirs.push(
                PathBuf::from(home)
                    .join(".local")
                    .join("share")
                    .join("ossify")
                    .join("tools")
                    .join("bin"),
            );
        }
    }

    let mut unique = Vec::new();
    let mut seen = BTreeSet::new();
    for dir in dirs {
        let key = dir.to_string_lossy().into_owned();
        if seen.insert(key) {
            unique.push(dir);
        }
    }
    unique
}

fn should_auto_install_engines() -> bool {
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

fn ensure_actionlint_binary() -> io::Result<PathBuf> {
    if let Some(path) = resolve_engine_binary("actionlint") {
        return Ok(path);
    }

    let installed = install_managed_actionlint()?;
    if installed.is_file() {
        return Ok(installed);
    }

    resolve_engine_binary("actionlint").ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "actionlint bootstrap finished but the managed binary could not be resolved",
        )
    })
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

    let tools_dir = primary_managed_tools_dir()?;
    let destination = tools_dir.join(binary_name);

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

fn primary_managed_tools_dir() -> io::Result<PathBuf> {
    let tools_dir = managed_engine_dirs()
        .into_iter()
        .next()
        .ok_or_else(|| io::Error::other("could not resolve a managed tools directory"))?;
    fs::create_dir_all(&tools_dir)?;
    Ok(tools_dir)
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

fn collect_markdown_files(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut builder = WalkBuilder::new(root);
    builder.hidden(false);
    builder.git_ignore(true);
    builder.git_global(true);
    builder.git_exclude(true);

    let mut files = Vec::new();
    for entry in builder.build() {
        let entry = entry.map_err(|error| io::Error::other(error.to_string()))?;
        if !entry
            .file_type()
            .map(|file_type| file_type.is_file())
            .unwrap_or(false)
        {
            continue;
        }

        let path = entry.into_path();
        if is_markdown_file(&path) {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

fn collect_workflow_files(root: &Path) -> io::Result<Vec<PathBuf>> {
    let workflows_root = root.join(".github/workflows");
    if !workflows_root.is_dir() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    let mut builder = WalkBuilder::new(&workflows_root);
    builder.hidden(false);
    builder.git_ignore(false);
    builder.git_global(false);
    builder.git_exclude(false);

    for entry in builder.build() {
        let entry = entry.map_err(|error| io::Error::other(error.to_string()))?;
        if !entry
            .file_type()
            .map(|file_type| file_type.is_file())
            .unwrap_or(false)
        {
            continue;
        }

        let path = entry.into_path();
        if matches!(
            path.extension().and_then(|value| value.to_str()),
            Some("yml" | "yaml")
        ) {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

fn is_markdown_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|value| value.to_str()),
        Some("md" | "markdown")
    )
}

fn analyze_markdown(path: &Path, contents: &str) -> MarkdownAnalysis {
    let mut headings = BTreeSet::new();
    let mut raw_headings = Vec::new();
    let mut links = Vec::new();
    let mut code_blocks = 0usize;
    let mut current_heading = None::<String>;

    let parser = Parser::new_ext(contents, Options::all());
    for event in parser {
        match event {
            Event::Start(Tag::Heading { .. }) => current_heading = Some(String::new()),
            Event::End(TagEnd::Heading(_)) => {
                if let Some(heading) = current_heading.take() {
                    let heading = heading.trim().to_owned();
                    if !heading.is_empty() {
                        raw_headings.push(heading.clone());
                        headings.insert(slugify_heading(&heading));
                    }
                }
            }
            Event::Start(Tag::CodeBlock(_)) => code_blocks += 1,
            Event::Start(Tag::Link { dest_url, .. })
            | Event::Start(Tag::Image { dest_url, .. }) => {
                links.push(dest_url.to_string());
            }
            Event::Text(text) | Event::Code(text) => {
                if let Some(current) = current_heading.as_mut() {
                    current.push_str(&text);
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if let Some(current) = current_heading.as_mut() {
                    current.push(' ');
                }
            }
            _ => {}
        }
    }

    MarkdownAnalysis {
        path: path.to_path_buf(),
        headings,
        raw_headings,
        links,
        code_blocks,
    }
}

fn has_heading(headings: &[String], candidates: &[&str]) -> bool {
    headings.iter().any(|heading| {
        let normalized = heading.to_lowercase();
        candidates
            .iter()
            .any(|candidate| normalized.contains(candidate))
    })
}

#[derive(Debug, Clone)]
struct LocalLink {
    path: Option<String>,
    anchor: Option<String>,
}

fn parse_local_link(destination: &str) -> Option<LocalLink> {
    if destination.is_empty()
        || destination.starts_with("http://")
        || destination.starts_with("https://")
        || destination.starts_with("mailto:")
        || destination.starts_with("tel:")
        || destination.starts_with("data:")
    {
        return None;
    }

    let (path, anchor) = match destination.split_once('#') {
        Some((path, anchor)) => (path, Some(anchor)),
        None => (destination, None),
    };

    Some(LocalLink {
        path: (!path.is_empty()).then(|| path.to_owned()),
        anchor: anchor.filter(|value| !value.is_empty()).map(str::to_owned),
    })
}

fn normalize_path(path: PathBuf) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                let _ = normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

fn slugify_heading(value: &str) -> String {
    let mut slug = String::with_capacity(value.len());
    let mut previous_dash = false;

    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            previous_dash = false;
        } else if !previous_dash {
            slug.push('-');
            previous_dash = true;
        }
    }

    slug.trim_matches('-').to_owned()
}

#[derive(Debug, Clone, Deserialize)]
struct ActionlintError {
    #[serde(default)]
    filepath: String,
    message: String,
    #[serde(default)]
    line: usize,
    #[serde(default)]
    column: usize,
    #[serde(default)]
    kind: Option<String>,
}

fn parse_actionlint_errors(input: &str) -> Result<Vec<ActionlintError>, serde_json::Error> {
    if input.trim().is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str(input)
}

fn format_actionlint_message(entry: &ActionlintError) -> String {
    if entry.line > 0 && entry.column > 0 {
        format!(
            "{} (line {}, col {})",
            entry.message, entry.line, entry.column
        )
    } else {
        entry.message.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_TEMP_ID: AtomicUsize = AtomicUsize::new(0);

    fn temp_repo(name: &str) -> PathBuf {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("{name}-{id}"));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create temp directory");
        path
    }

    #[test]
    fn detects_broken_local_markdown_links() {
        let root = temp_repo("ossify-docs-doctor-broken-link");
        fs::write(
            root.join("README.md"),
            "# Demo\n\n## Install\n\n```bash\ncargo run\n```\n\n## Usage\n\nSee [Guide](docs/guide.md).\n",
        )
        .expect("write readme");

        let report = doctor_docs(&root).expect("doctor docs");
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.code == "docs.broken-link"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn detects_missing_markdown_anchor() {
        let root = temp_repo("ossify-docs-doctor-anchor");
        fs::create_dir_all(root.join("docs")).expect("create docs");
        fs::write(
            root.join("README.md"),
            "# Demo\n\n## Install\n\n```bash\ncargo run\n```\n\n## Usage\n\nSee [Guide](docs/guide.md#missing-anchor).\n",
        )
        .expect("write readme");
        fs::write(root.join("docs/guide.md"), "# Guide\n\n## Real Heading\n").expect("write guide");

        let report = doctor_docs(&root).expect("doctor docs");
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.code == "docs.missing-anchor"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn flags_readme_quality_gaps() {
        let root = temp_repo("ossify-docs-doctor-readme");
        fs::write(root.join("README.md"), "# Demo\n\nShort description.\n").expect("write readme");

        let report = doctor_docs(&root).expect("doctor docs");
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.code == "readme.install"));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.code == "readme.usage"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn parses_actionlint_json_output() {
        let errors = parse_actionlint_errors(
            r#"[{"message":"unexpected key","filepath":".github/workflows/ci.yml","line":5,"column":11,"kind":"syntax-check"}]"#,
        )
        .expect("parse actionlint output");

        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].filepath, ".github/workflows/ci.yml");
        assert_eq!(errors[0].line, 5);
        assert_eq!(errors[0].kind.as_deref(), Some("syntax-check"));
    }

    #[test]
    fn workflow_doctor_handles_missing_engine() {
        let root = temp_repo("ossify-workflow-doctor-no-engine");
        fs::create_dir_all(root.join(".github/workflows")).expect("create workflows");
        fs::write(
            root.join(".github/workflows/ci.yml"),
            "name: CI\non: push\njobs:\n  test:\n    runs-on: ubuntu-latest\n    steps:\n      - run: echo ok\n",
        )
        .expect("write workflow");

        let report = doctor_workflow(&root).expect("workflow doctor");
        if !report.engine_available {
            assert!(report.score.is_none());
            assert!(report
                .findings
                .iter()
                .any(|finding| finding.code == "workflow.engine-missing"));
        }

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn workflow_hygiene_finds_missing_guardrails() {
        let root = temp_repo("ossify-workflow-doctor-hygiene");
        fs::create_dir_all(root.join(".github/workflows")).expect("create workflows");
        let workflow = root.join(".github/workflows/ci.yml");
        fs::write(
            &workflow,
            "name: CI\non: push\njobs:\n  test:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v4\n      - uses: Swatinem/rust-cache@v2\n      - run: cargo test\n",
        )
        .expect("write workflow");

        let findings = workflow_hygiene_findings(&root, std::slice::from_ref(&workflow))
            .expect("workflow hygiene findings");

        assert!(findings
            .iter()
            .any(|finding| finding.code == "workflow.permissions.missing"));
        assert!(findings
            .iter()
            .any(|finding| finding.code == "workflow.concurrency.missing"));
        assert!(findings
            .iter()
            .any(|finding| finding.code == "workflow.timeout.missing"));
        assert!(findings
            .iter()
            .any(|finding| finding.code == "workflow.actions.unpinned"));
        assert_eq!(
            findings
                .iter()
                .filter(|finding| finding.code == "workflow.timeout.missing")
                .count(),
            1
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn official_actions_tags_do_not_trigger_unpinned_finding() {
        let root = temp_repo("ossify-workflow-doctor-official-actions");
        fs::create_dir_all(root.join(".github/workflows")).expect("create workflows");
        let workflow = root.join(".github/workflows/pages.yml");
        fs::write(
            &workflow,
            "name: Pages\npermissions:\n  contents: read\nconcurrency:\n  group: pages\njobs:\n  deploy:\n    timeout-minutes: 15\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v4\n      - uses: actions/upload-pages-artifact@v3\n",
        )
        .expect("write workflow");

        let findings = workflow_hygiene_findings(&root, std::slice::from_ref(&workflow))
            .expect("workflow hygiene findings");

        assert!(!findings
            .iter()
            .any(|finding| finding.code == "workflow.actions.unpinned"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn syntax_errors_cap_workflow_score_below_fifty() {
        let findings = vec![DoctorFinding {
            severity: DoctorSeverity::Error,
            code: String::from("workflow.syntax-check"),
            message: String::from("could not parse as YAML"),
            file: None,
            help: None,
        }];

        assert_eq!(workflow_score(&findings), 49);
    }

    #[test]
    fn non_syntax_actionlint_errors_cap_workflow_score_below_seventy() {
        let findings = vec![DoctorFinding {
            severity: DoctorSeverity::Error,
            code: String::from("workflow.expression"),
            message: String::from("invalid expression"),
            file: None,
            help: None,
        }];

        assert_eq!(workflow_score(&findings), 69);
    }
}
