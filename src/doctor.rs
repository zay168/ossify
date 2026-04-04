use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use ignore::WalkBuilder;
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use serde::Deserialize;
use serde::Serialize;
use serde_yaml::{Mapping, Value};

use crate::cli::EcosystemArg;
use crate::engines::{run_tool, ManagedEngineError, ManagedEngineStatus, ManagedTool};
use crate::rust_deps::{score_rust_deps_findings, RustAdvisoryClass, RustDepsScoringOutcome};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DoctorDomain {
    Docs,
    Workflow,
    Deps,
    Release,
}

impl DoctorDomain {
    pub fn label(self) -> &'static str {
        match self {
            Self::Docs => "docs",
            Self::Workflow => "workflow",
            Self::Deps => "deps",
            Self::Release => "release",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DoctorEcosystem {
    Auto,
    Rust,
    Node,
    Python,
}

impl DoctorEcosystem {
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Rust => "rust",
            Self::Node => "node",
            Self::Python => "python",
        }
    }
}

impl From<EcosystemArg> for DoctorEcosystem {
    fn from(value: EcosystemArg) -> Self {
        match value {
            EcosystemArg::Auto => Self::Auto,
            EcosystemArg::Rust => Self::Rust,
            EcosystemArg::Node => Self::Node,
            EcosystemArg::Python => Self::Python,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EngineSource {
    OssifyNative,
    AbsorbedPolicy,
    ManagedTool,
}

#[derive(Debug, Clone, Serialize)]
pub struct DoctorFinding {
    pub domain: DoctorDomain,
    pub ecosystem: Option<DoctorEcosystem>,
    pub severity: DoctorSeverity,
    pub code: String,
    pub message: String,
    pub file: Option<PathBuf>,
    pub help: Option<String>,
    pub evidence: Vec<String>,
    pub fix_hint: Option<String>,
    pub engine: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DoctorDomainScore {
    pub domain: DoctorDomain,
    pub score: Option<u8>,
    pub cap: Option<u8>,
    pub cap_reason: Option<String>,
    pub cap_code: Option<String>,
    pub engine: String,
    pub engine_source: EngineSource,
    pub ecosystems: Vec<DoctorEcosystem>,
    pub error_count: usize,
    pub warning_count: usize,
    pub info_count: usize,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EcosystemDoctorScore {
    pub ecosystem: DoctorEcosystem,
    pub score: u8,
    pub cap: Option<u8>,
    pub cap_reason: Option<String>,
    pub cap_code: Option<String>,
    pub engine: String,
    pub engine_source: EngineSource,
    pub engine_status: ManagedEngineStatus,
    pub engine_detail: Option<String>,
    pub finding_count: usize,
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

    pub fn domain_score(&self) -> DoctorDomainScore {
        DoctorDomainScore {
            domain: DoctorDomain::Docs,
            score: Some(self.score),
            cap: None,
            cap_reason: None,
            cap_code: None,
            engine: String::from("ossify markdown policy"),
            engine_source: EngineSource::OssifyNative,
            ecosystems: Vec::new(),
            error_count: self.error_count(),
            warning_count: self.warning_count(),
            info_count: self.info_count(),
            summary: self.summary(),
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

    pub fn domain_score(&self) -> DoctorDomainScore {
        DoctorDomainScore {
            domain: DoctorDomain::Workflow,
            score: self.score,
            cap: self
                .findings
                .iter()
                .find_map(|finding| match finding.code.as_str() {
                    "workflow.syntax-check" => Some(49),
                    _ if finding.severity == DoctorSeverity::Error => Some(69),
                    _ => None,
                }),
            cap_reason: self
                .findings
                .iter()
                .find_map(|finding| match finding.code.as_str() {
                    "workflow.syntax-check" => Some(String::from(
                        "workflow syntax error capped workflow score at 49/100",
                    )),
                    _ if finding.severity == DoctorSeverity::Error => Some(String::from(
                        "actionlint runtime errors cap workflow score below 70/100",
                    )),
                    _ => None,
                }),
            cap_code: self
                .findings
                .iter()
                .find_map(|finding| match finding.code.as_str() {
                    "workflow.syntax-check" => Some(String::from("workflow.syntax-check")),
                    _ if finding.severity == DoctorSeverity::Error => {
                        Some(String::from("workflow.actionlint.error"))
                    }
                    _ => None,
                }),
            engine: self.engine.clone(),
            engine_source: EngineSource::ManagedTool,
            ecosystems: Vec::new(),
            error_count: self.error_count(),
            warning_count: self.warning_count(),
            info_count: self.info_count(),
            summary: self.summary(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DepsDoctorReport {
    pub target: PathBuf,
    pub requested_ecosystem: DoctorEcosystem,
    pub ecosystems: Vec<EcosystemDoctorScore>,
    pub domain: DoctorDomainScore,
    pub findings: Vec<DoctorFinding>,
}

impl DepsDoctorReport {
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

    pub fn summary(&self) -> &str {
        &self.domain.summary
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ReleaseDoctorReport {
    pub target: PathBuf,
    pub requested_ecosystem: DoctorEcosystem,
    pub ecosystems: Vec<EcosystemDoctorScore>,
    pub domain: DoctorDomainScore,
    pub findings: Vec<DoctorFinding>,
}

impl ReleaseDoctorReport {
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

    pub fn summary(&self) -> &str {
        &self.domain.summary
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

#[derive(Debug)]
struct EcosystemScan {
    ecosystem: DoctorEcosystem,
    findings: Vec<DoctorFinding>,
    engine: String,
    engine_source: EngineSource,
    engine_status: ManagedEngineStatus,
    engine_detail: Option<String>,
}

fn doctor_finding(
    domain: DoctorDomain,
    ecosystem: Option<DoctorEcosystem>,
    severity: DoctorSeverity,
    code: impl Into<String>,
    message: impl Into<String>,
    file: Option<PathBuf>,
    help: Option<String>,
) -> DoctorFinding {
    DoctorFinding {
        domain,
        ecosystem,
        severity,
        code: code.into(),
        message: message.into(),
        file,
        help,
        evidence: Vec::new(),
        fix_hint: None,
        engine: None,
    }
}

#[allow(clippy::too_many_arguments)]
fn doctor_finding_with_detail(
    domain: DoctorDomain,
    ecosystem: Option<DoctorEcosystem>,
    severity: DoctorSeverity,
    code: impl Into<String>,
    message: impl Into<String>,
    file: Option<PathBuf>,
    help: Option<String>,
    evidence: Vec<String>,
    fix_hint: Option<String>,
    engine: Option<String>,
) -> DoctorFinding {
    DoctorFinding {
        domain,
        ecosystem,
        severity,
        code: code.into(),
        message: message.into(),
        file,
        help,
        evidence,
        fix_hint,
        engine,
    }
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
        findings.push(doctor_finding(
            DoctorDomain::Docs,
            None,
            DoctorSeverity::Error,
            "readme.missing",
            "README.md is missing.",
            Some(readme_path.clone()),
            Some(String::from(
                "Add a root README so the project has a clear entry point for install and usage.",
            )),
        ));
    }

    if let Some(readme) = readme {
        if !has_heading(&readme.raw_headings, &["install", "installation"]) {
            findings.push(doctor_finding(
                DoctorDomain::Docs,
                None,
                DoctorSeverity::Warning,
                "readme.install",
                "README.md has no obvious install section.",
                Some(readme.path.clone()),
                Some(String::from(
                    "Add an Install or Installation section with one copy-pasteable command.",
                )),
            ));
        }

        if !has_heading(
            &readme.raw_headings,
            &["usage", "quickstart", "examples", "getting started"],
        ) {
            findings.push(doctor_finding(
                DoctorDomain::Docs,
                None,
                DoctorSeverity::Warning,
                "readme.usage",
                "README.md has no obvious usage or examples section.",
                Some(readme.path.clone()),
                Some(String::from(
                    "Add a Usage, Examples, or Quickstart section so readers can try the tool immediately.",
                )),
            ));
        }

        if readme.code_blocks == 0 {
            findings.push(doctor_finding(
                DoctorDomain::Docs,
                None,
                DoctorSeverity::Info,
                "readme.codeblocks",
                "README.md has no fenced code blocks.",
                Some(readme.path.clone()),
                Some(String::from(
                    "Add at least one copy-pasteable command example to make the README more actionable.",
                )),
            ));
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
                findings.push(doctor_finding(
                    DoctorDomain::Docs,
                    None,
                    DoctorSeverity::Error,
                    "docs.broken-link",
                    format!("Broken local link `{destination}`."),
                    Some(analysis.path.clone()),
                    Some(String::from(
                        "Create the missing file or update the link destination so local docs stay navigable.",
                    )),
                ));
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
                        findings.push(doctor_finding(
                            DoctorDomain::Docs,
                            None,
                            DoctorSeverity::Error,
                            "docs.missing-anchor",
                            format!(
                                "Link `{destination}` points to an anchor that does not exist."
                            ),
                            Some(analysis.path.clone()),
                            Some(String::from(
                                "Update the fragment or add the matching heading in the target Markdown file.",
                            )),
                        ));
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

    let output = match run_tool(ManagedTool::Actionlint, &target, &["-format", "{{json .}}"]) {
        Ok(output) => output,
        Err(error) => {
            return Ok(workflow_engine_error_report(
                &target,
                workflow_files.len(),
                &error,
            ))
        }
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
            doctor_finding_with_detail(
                DoctorDomain::Workflow,
                None,
                DoctorSeverity::Error,
                format!("workflow.{kind}"),
                format_actionlint_message(&entry),
                (!entry.filepath.is_empty()).then(|| normalize_path(target.join(entry.filepath))),
                None,
                Vec::new(),
                None,
                Some(String::from("actionlint")),
            )
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
            findings.push(doctor_finding_with_detail(
                DoctorDomain::Workflow,
                None,
                DoctorSeverity::Warning,
                "workflow.permissions.missing",
                "Workflow does not declare explicit GITHUB_TOKEN permissions.",
                Some(path.clone()),
                Some(String::from(
                    "Add a top-level `permissions:` block with the narrowest access your jobs need, usually `contents: read` for CI.",
                )),
                Vec::new(),
                Some(String::from(
                    "Declare explicit top-level permissions and keep them narrow.",
                )),
                Some(String::from("ossify workflow policy")),
            ));
        }

        if !document.contains_key(Value::String(String::from("concurrency"))) {
            findings.push(doctor_finding_with_detail(
                DoctorDomain::Workflow,
                None,
                DoctorSeverity::Info,
                "workflow.concurrency.missing",
                "Workflow has no concurrency guard, so superseded runs can continue burning minutes.",
                Some(path.clone()),
                Some(String::from(
                    "Add `concurrency` when duplicate runs should cancel in progress, especially for CI and release-related workflows.",
                )),
                Vec::new(),
                Some(String::from(
                    "Add concurrency cancellation for superseded runs.",
                )),
                Some(String::from("ossify workflow policy")),
            ));
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
            findings.push(doctor_finding_with_detail(
                DoctorDomain::Workflow,
                None,
                DoctorSeverity::Info,
                "workflow.timeout.missing",
                format!(
                    "missing `timeout-minutes` on jobs: {}",
                    timeout_missing_jobs.join(", ")
                ),
                Some(path.clone()),
                Some(String::from(
                    "Add `timeout-minutes:` to long-running jobs so failures fail fast instead of hanging.",
                )),
                timeout_missing_jobs.clone(),
                Some(String::from(
                    "Add timeout-minutes to every job in the workflow.",
                )),
                Some(String::from("ossify workflow policy")),
            ));
        }

        let mutable_refs = collect_mutable_action_refs(document);
        if !mutable_refs.is_empty() {
            findings.push(doctor_finding_with_detail(
                DoctorDomain::Workflow,
                None,
                DoctorSeverity::Info,
                "workflow.actions.unpinned",
                format!(
                    "Workflow uses mutable action refs instead of full commit SHAs: {}.",
                    mutable_refs.join(", ")
                ),
                Some(path.clone()),
                Some(String::from(
                    "Pin action refs to full commit SHAs for stronger supply-chain reproducibility, then optionally document the friendly tag in comments.",
                )),
                mutable_refs,
                Some(String::from(
                    "Pin third-party GitHub Actions to immutable SHAs.",
                )),
                Some(String::from("ossify workflow policy")),
            ));
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

pub fn doctor_deps(
    root: &Path,
    requested_ecosystem: DoctorEcosystem,
) -> io::Result<DepsDoctorReport> {
    let target = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let ecosystems = requested_or_detected_ecosystems(&target, requested_ecosystem);

    if ecosystems.is_empty() {
        let domain = DoctorDomainScore {
            domain: DoctorDomain::Deps,
            score: Some(100),
            cap: None,
            cap_reason: None,
            cap_code: None,
            engine: String::from("cargo-deny + audit-ci + pip-audit semantics"),
            engine_source: EngineSource::AbsorbedPolicy,
            ecosystems: Vec::new(),
            error_count: 0,
            warning_count: 0,
            info_count: 0,
            summary: String::from("No supported dependency ecosystems were detected."),
        };

        return Ok(DepsDoctorReport {
            target,
            requested_ecosystem,
            ecosystems: Vec::new(),
            domain,
            findings: Vec::new(),
        });
    }

    let scans = ecosystems
        .into_iter()
        .map(|ecosystem| scan_deps_ecosystem(&target, ecosystem))
        .collect::<io::Result<Vec<_>>>()?;

    let mut findings = Vec::new();
    let mut ecosystem_scores = Vec::new();
    for scan in scans {
        let scoring = deps_scoring_for_ecosystem(scan.ecosystem, &scan.findings);

        ecosystem_scores.push(EcosystemDoctorScore {
            ecosystem: scan.ecosystem,
            score: scoring.score,
            cap: scoring.cap,
            cap_reason: scoring.cap_reason.clone(),
            cap_code: scoring.cap_code.clone(),
            engine: scan.engine.clone(),
            engine_source: scan.engine_source,
            engine_status: scan.engine_status,
            engine_detail: scan.engine_detail.clone(),
            finding_count: scan.findings.len(),
        });
        findings.extend(scan.findings);
    }

    sort_doctor_findings(&mut findings);
    let mut score = average_scores(&ecosystem_scores);
    let cap = ecosystem_scores.iter().filter_map(|entry| entry.cap).min();
    if let Some(cap) = cap {
        score = Some(score.unwrap_or(cap).min(cap));
    }

    let domain = DoctorDomainScore {
        domain: DoctorDomain::Deps,
        score,
        cap,
        cap_reason: ecosystem_scores
            .iter()
            .filter_map(|entry| entry.cap_reason.as_ref().map(|reason| (entry.cap, reason)))
            .min_by_key(|(cap, _)| cap.unwrap_or(u8::MAX))
            .map(|(_, reason)| reason.clone()),
        cap_code: ecosystem_scores
            .iter()
            .filter_map(|entry| entry.cap_code.as_ref().map(|code| (entry.cap, code)))
            .min_by_key(|(cap, _)| cap.unwrap_or(u8::MAX))
            .map(|(_, code)| code.clone()),
        engine: String::from("cargo-deny + audit-ci + pip-audit semantics"),
        engine_source: aggregate_engine_source(&ecosystem_scores),
        ecosystems: ecosystem_scores
            .iter()
            .map(|entry| entry.ecosystem)
            .collect(),
        error_count: findings
            .iter()
            .filter(|finding| finding.severity == DoctorSeverity::Error)
            .count(),
        warning_count: findings
            .iter()
            .filter(|finding| finding.severity == DoctorSeverity::Warning)
            .count(),
        info_count: findings
            .iter()
            .filter(|finding| finding.severity == DoctorSeverity::Info)
            .count(),
        summary: deps_summary(&findings, &ecosystem_scores),
    };

    Ok(DepsDoctorReport {
        target,
        requested_ecosystem,
        ecosystems: ecosystem_scores,
        domain,
        findings,
    })
}

pub fn doctor_release(
    root: &Path,
    requested_ecosystem: DoctorEcosystem,
) -> io::Result<ReleaseDoctorReport> {
    let target = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let ecosystems = requested_or_detected_ecosystems(&target, requested_ecosystem);

    if ecosystems.is_empty() {
        let domain = DoctorDomainScore {
            domain: DoctorDomain::Release,
            score: Some(100),
            cap: None,
            cap_reason: None,
            cap_code: None,
            engine: String::from("release-plz + git-cliff + cargo-dist + release-please semantics"),
            engine_source: EngineSource::AbsorbedPolicy,
            ecosystems: Vec::new(),
            error_count: 0,
            warning_count: 0,
            info_count: 0,
            summary: String::from("No supported release ecosystems were detected."),
        };

        return Ok(ReleaseDoctorReport {
            target,
            requested_ecosystem,
            ecosystems: Vec::new(),
            domain,
            findings: Vec::new(),
        });
    }

    let scans = ecosystems
        .into_iter()
        .map(|ecosystem| scan_release_ecosystem(&target, ecosystem))
        .collect::<io::Result<Vec<_>>>()?;

    let mut findings = Vec::new();
    let mut ecosystem_scores = Vec::new();
    for scan in scans {
        let cap = release_score_cap(&scan.findings);
        let mut score = release_score(&scan.findings);
        if let Some(cap) = cap {
            score = score.min(cap);
        }

        ecosystem_scores.push(EcosystemDoctorScore {
            ecosystem: scan.ecosystem,
            score,
            cap,
            cap_reason: cap.map(|cap| {
                format!(
                    "release {} checks capped score at {cap}/100",
                    scan.ecosystem.label()
                )
            }),
            cap_code: cap.map(|_| format!("release.{}.cap", scan.ecosystem.label())),
            engine: scan.engine.clone(),
            engine_source: scan.engine_source,
            engine_status: scan.engine_status,
            engine_detail: scan.engine_detail.clone(),
            finding_count: scan.findings.len(),
        });
        findings.extend(scan.findings);
    }

    sort_doctor_findings(&mut findings);
    let mut score = average_scores(&ecosystem_scores);
    let cap = ecosystem_scores.iter().filter_map(|entry| entry.cap).min();
    if let Some(cap) = cap {
        score = Some(score.unwrap_or(cap).min(cap));
    }

    let domain = DoctorDomainScore {
        domain: DoctorDomain::Release,
        score,
        cap,
        cap_reason: ecosystem_scores
            .iter()
            .filter_map(|entry| entry.cap_reason.as_ref().map(|reason| (entry.cap, reason)))
            .min_by_key(|(cap, _)| cap.unwrap_or(u8::MAX))
            .map(|(_, reason)| reason.clone()),
        cap_code: ecosystem_scores
            .iter()
            .filter_map(|entry| entry.cap_code.as_ref().map(|code| (entry.cap, code)))
            .min_by_key(|(cap, _)| cap.unwrap_or(u8::MAX))
            .map(|(_, code)| code.clone()),
        engine: String::from("release-plz + git-cliff + cargo-dist + release-please semantics"),
        engine_source: aggregate_engine_source(&ecosystem_scores),
        ecosystems: ecosystem_scores
            .iter()
            .map(|entry| entry.ecosystem)
            .collect(),
        error_count: findings
            .iter()
            .filter(|finding| finding.severity == DoctorSeverity::Error)
            .count(),
        warning_count: findings
            .iter()
            .filter(|finding| finding.severity == DoctorSeverity::Warning)
            .count(),
        info_count: findings
            .iter()
            .filter(|finding| finding.severity == DoctorSeverity::Info)
            .count(),
        summary: release_summary(&findings, &ecosystem_scores),
    };

    Ok(ReleaseDoctorReport {
        target,
        requested_ecosystem,
        ecosystems: ecosystem_scores,
        domain,
        findings,
    })
}

pub fn audit_domain_scores(root: &Path) -> io::Result<Vec<DoctorDomainScore>> {
    let docs = doctor_docs(root)?.domain_score();
    let workflow = doctor_workflow(root)?;
    let deps = doctor_deps(root, DoctorEcosystem::Auto)?;
    let release = doctor_release(root, DoctorEcosystem::Auto)?;

    let mut scores = vec![docs];
    if workflow.workflow_files > 0 || !workflow.findings.is_empty() {
        scores.push(workflow.domain_score());
    }
    if !deps.ecosystems.is_empty() {
        scores.push(deps.domain);
    }
    if !release.ecosystems.is_empty() {
        scores.push(release.domain);
    }

    Ok(scores)
}

fn requested_or_detected_ecosystems(
    root: &Path,
    requested: DoctorEcosystem,
) -> Vec<DoctorEcosystem> {
    if requested != DoctorEcosystem::Auto {
        return vec![requested];
    }

    let mut ecosystems = Vec::new();
    if root.join("Cargo.toml").is_file() {
        ecosystems.push(DoctorEcosystem::Rust);
    }
    if root.join("package.json").is_file() {
        ecosystems.push(DoctorEcosystem::Node);
    }
    if root.join("pyproject.toml").is_file() || has_requirements_files(root) {
        ecosystems.push(DoctorEcosystem::Python);
    }
    ecosystems
}

fn sort_doctor_findings(findings: &mut [DoctorFinding]) {
    findings.sort_by(|left, right| {
        left.severity
            .rank()
            .cmp(&right.severity.rank())
            .then_with(|| left.ecosystem.cmp(&right.ecosystem))
            .then_with(|| left.file.cmp(&right.file))
            .then_with(|| left.code.cmp(&right.code))
    });
}

fn average_scores(scores: &[EcosystemDoctorScore]) -> Option<u8> {
    if scores.is_empty() {
        return None;
    }
    let total = scores
        .iter()
        .map(|entry| u16::from(entry.score))
        .sum::<u16>();
    Some((total / scores.len() as u16) as u8)
}

fn aggregate_engine_source(scores: &[EcosystemDoctorScore]) -> EngineSource {
    if scores
        .iter()
        .any(|entry| entry.engine_source == EngineSource::ManagedTool)
    {
        EngineSource::ManagedTool
    } else {
        EngineSource::AbsorbedPolicy
    }
}

fn workflow_engine_error_report(
    target: &Path,
    workflow_files: usize,
    error: &ManagedEngineError,
) -> WorkflowDoctorReport {
    let (code, message) = match error.status {
        ManagedEngineStatus::HeuristicFallback => (
            "workflow.engine-missing",
            "actionlint is not available, so external workflow checks could not run.",
        ),
        ManagedEngineStatus::RuntimeMissing => (
            "workflow.engine-runtime-missing",
            "actionlint could not be bootstrapped because a required runtime or platform path was missing.",
        ),
        ManagedEngineStatus::BootstrapFailed => (
            "workflow.engine-bootstrap-failed",
            "actionlint was missing and automatic bootstrap did not complete.",
        ),
        ManagedEngineStatus::ExecutionFailed => (
            "workflow.engine-execution-failed",
            "actionlint is available but failed during execution.",
        ),
        ManagedEngineStatus::ParseFailed | ManagedEngineStatus::Managed => (
            "workflow.engine-missing",
            "actionlint is not available, so external workflow checks could not run.",
        ),
    };

    WorkflowDoctorReport {
        target: target.to_path_buf(),
        score: None,
        workflow_files,
        engine: String::from("actionlint"),
        engine_available: false,
        findings: vec![doctor_finding_with_detail(
            DoctorDomain::Workflow,
            None,
            if matches!(error.status, ManagedEngineStatus::HeuristicFallback) {
                DoctorSeverity::Info
            } else {
                DoctorSeverity::Warning
            },
            code,
            message,
            None,
            Some(error.message.clone()),
            Vec::new(),
            Some(String::from(
                "Let ossify manage the workflow engine automatically, or point `OSSIFY_ACTIONLINT` at a working binary.",
            )),
            Some(String::from("actionlint")),
        )],
    }
}

fn scan_deps_ecosystem(root: &Path, ecosystem: DoctorEcosystem) -> io::Result<EcosystemScan> {
    let heuristic_findings = match ecosystem {
        DoctorEcosystem::Rust => analyze_rust_deps(root)?,
        DoctorEcosystem::Node => analyze_node_deps(root)?,
        DoctorEcosystem::Python => analyze_python_deps(root)?,
        DoctorEcosystem::Auto => Vec::new(),
    };

    let managed = match ecosystem {
        DoctorEcosystem::Rust => run_rust_deps_engine(root),
        DoctorEcosystem::Node => run_node_deps_engine(root),
        DoctorEcosystem::Python => run_python_deps_engine(root),
        DoctorEcosystem::Auto => Ok(Vec::new()),
    };

    match managed {
        Ok(mut engine_findings) => {
            engine_findings.extend(heuristic_findings);
            Ok(EcosystemScan {
                ecosystem,
                findings: engine_findings,
                engine: ecosystem_deps_engine(ecosystem).to_owned(),
                engine_source: EngineSource::ManagedTool,
                engine_status: ManagedEngineStatus::Managed,
                engine_detail: None,
            })
        }
        Err(error) => {
            let mut findings = heuristic_findings;
            findings.push(managed_engine_fallback_finding(
                DoctorDomain::Deps,
                ecosystem,
                &error,
                Some(String::from(
                    "ossify kept the absorbed policy fallback active for this ecosystem.",
                )),
            ));
            Ok(EcosystemScan {
                ecosystem,
                findings,
                engine: ecosystem_deps_engine(ecosystem).to_owned(),
                engine_source: EngineSource::AbsorbedPolicy,
                engine_status: error.status,
                engine_detail: Some(error.message),
            })
        }
    }
}

fn scan_release_ecosystem(root: &Path, ecosystem: DoctorEcosystem) -> io::Result<EcosystemScan> {
    let heuristic_findings = match ecosystem {
        DoctorEcosystem::Rust => analyze_rust_release(root)?,
        DoctorEcosystem::Node => analyze_node_release(root)?,
        DoctorEcosystem::Python => analyze_python_release(root)?,
        DoctorEcosystem::Auto => Vec::new(),
    };

    let managed = match ecosystem {
        DoctorEcosystem::Rust => verify_rust_release_engines(root),
        DoctorEcosystem::Node => verify_release_please_engine(root, DoctorEcosystem::Node),
        DoctorEcosystem::Python => verify_release_please_engine(root, DoctorEcosystem::Python),
        DoctorEcosystem::Auto => Ok(()),
    };

    match managed {
        Ok(()) => Ok(EcosystemScan {
            ecosystem,
            findings: heuristic_findings,
            engine: ecosystem_release_engine(ecosystem).to_owned(),
            engine_source: EngineSource::ManagedTool,
            engine_status: ManagedEngineStatus::Managed,
            engine_detail: None,
        }),
        Err(error) => {
            let mut findings = heuristic_findings;
            findings.push(managed_engine_fallback_finding(
                DoctorDomain::Release,
                ecosystem,
                &error,
                Some(String::from(
                    "ossify kept the absorbed release heuristics active for this ecosystem.",
                )),
            ));
            Ok(EcosystemScan {
                ecosystem,
                findings,
                engine: ecosystem_release_engine(ecosystem).to_owned(),
                engine_source: EngineSource::AbsorbedPolicy,
                engine_status: error.status,
                engine_detail: Some(error.message),
            })
        }
    }
}

fn managed_engine_fallback_finding(
    domain: DoctorDomain,
    ecosystem: DoctorEcosystem,
    error: &ManagedEngineError,
    help: Option<String>,
) -> DoctorFinding {
    let severity = match error.status {
        ManagedEngineStatus::HeuristicFallback => DoctorSeverity::Info,
        ManagedEngineStatus::RuntimeMissing
        | ManagedEngineStatus::BootstrapFailed
        | ManagedEngineStatus::ExecutionFailed
        | ManagedEngineStatus::ParseFailed => DoctorSeverity::Warning,
        ManagedEngineStatus::Managed => DoctorSeverity::Info,
    };
    let suffix = match error.status {
        ManagedEngineStatus::HeuristicFallback => "engine-missing",
        ManagedEngineStatus::RuntimeMissing => "engine-runtime-missing",
        ManagedEngineStatus::BootstrapFailed => "engine-bootstrap-failed",
        ManagedEngineStatus::ExecutionFailed => "engine-execution-failed",
        ManagedEngineStatus::ParseFailed => "engine-parse-failed",
        ManagedEngineStatus::Managed => "engine-managed",
    };
    let domain_label = domain.label();
    doctor_finding_with_detail(
        domain,
        Some(ecosystem),
        severity,
        format!("{domain_label}.{}.{}", ecosystem.label(), suffix),
        format!(
            "Managed {} checks could not run cleanly, so ossify used degraded fallback heuristics.",
            error.tool.display_name()
        ),
        None,
        help.or_else(|| Some(error.message.clone())),
        Vec::new(),
        Some(format!(
            "Restore the managed {} path to upgrade this ecosystem back to high-confidence engine-backed checks.",
            error.tool.display_name()
        )),
        Some(error.tool.display_name().to_owned()),
    )
}

fn run_rust_deps_engine(root: &Path) -> Result<Vec<DoctorFinding>, ManagedEngineError> {
    let mut args = vec![
        "-f",
        "json",
        "--manifest-path",
        "Cargo.toml",
        "check",
        "advisories",
    ];
    if root.join("deny.toml").is_file() {
        args.extend(["bans", "licenses", "sources"]);
    }
    let output = run_tool(ManagedTool::CargoDeny, root, &args)?;

    parse_cargo_deny_output(root, &output).map_err(|message| ManagedEngineError {
        tool: ManagedTool::CargoDeny,
        status: ManagedEngineStatus::ParseFailed,
        message,
    })
}

fn run_node_deps_engine(root: &Path) -> Result<Vec<DoctorFinding>, ManagedEngineError> {
    let output = run_tool(
        ManagedTool::AuditCi,
        root,
        &[
            "--directory",
            ".",
            "--output-format",
            "json",
            "--report-type",
            "full",
        ],
    )?;

    parse_audit_ci_output(root, &output).map_err(|message| ManagedEngineError {
        tool: ManagedTool::AuditCi,
        status: ManagedEngineStatus::ParseFailed,
        message,
    })
}

fn run_python_deps_engine(root: &Path) -> Result<Vec<DoctorFinding>, ManagedEngineError> {
    let args = python_audit_args(root);
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    let output = run_tool(ManagedTool::PipAudit, root, &refs)?;
    parse_pip_audit_output(root, &output).map_err(|message| ManagedEngineError {
        tool: ManagedTool::PipAudit,
        status: ManagedEngineStatus::ParseFailed,
        message,
    })
}

fn verify_rust_release_engines(root: &Path) -> Result<(), ManagedEngineError> {
    run_tool(ManagedTool::ReleasePlz, root, &["--version"])?;
    run_tool(ManagedTool::GitCliff, root, &["--version"])?;
    run_tool(ManagedTool::CargoDist, root, &["--version"])?;
    Ok(())
}

fn verify_release_please_engine(
    root: &Path,
    ecosystem: DoctorEcosystem,
) -> Result<(), ManagedEngineError> {
    let release_type = ecosystem.label();
    run_tool(
        ManagedTool::ReleasePlease,
        root,
        &[
            "release-pr",
            "--dry-run",
            "--repo-url",
            "https://github.com/example/example",
            "--release-type",
            release_type,
            "--path",
            ".",
        ],
    )
    .map(|_| ())
}

fn parse_cargo_deny_output(
    root: &Path,
    output: &std::process::Output,
) -> Result<Vec<DoctorFinding>, String> {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}\n{stderr}");
    let mut findings = Vec::new();
    let mut parsed_any = false;

    for line in combined.lines().filter(|line| !line.trim().is_empty()) {
        let value: serde_json::Value = serde_json::from_str(line)
            .map_err(|error| format!("invalid cargo-deny JSON line: {error}"))?;
        parsed_any = true;
        let fields = value
            .get("fields")
            .and_then(serde_json::Value::as_object)
            .cloned()
            .unwrap_or_default();
        let level = fields
            .get("level")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_ascii_lowercase();
        let diagnostic_severity = fields
            .get("severity")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_ascii_lowercase();
        let diagnostic_code = fields
            .get("code")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_ascii_lowercase();
        let message = fields
            .get("message")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_owned();
        if message.is_empty() {
            continue;
        }

        if message.contains("unable to find a config path") {
            continue;
        }

        if message.contains("failed to open advisory database")
            || message.contains("does not appear to be a git repository")
            || message.contains("failed to fetch")
        {
            return Err(format!(
                "cargo-deny could not complete its advisory checks: {message}"
            ));
        }

        let (severity, code, fix_hint, evidence) = if let Some(class) =
            cargo_deny_advisory_class(&fields, &diagnostic_code, &message)
        {
            let evidence = cargo_deny_advisory_evidence(&fields, class);
            let severity = match class {
                RustAdvisoryClass::Critical
                | RustAdvisoryClass::High
                | RustAdvisoryClass::Medium
                | RustAdvisoryClass::Unsound
                | RustAdvisoryClass::Reported => DoctorSeverity::Error,
                RustAdvisoryClass::Low
                | RustAdvisoryClass::Unmaintained
                | RustAdvisoryClass::Yanked => DoctorSeverity::Warning,
                RustAdvisoryClass::Informational => DoctorSeverity::Info,
            };

            (
                severity,
                class.code(),
                Some(String::from(
                    "Review cargo-deny output and resolve, replace, or explicitly ignore the reported advisory set.",
                )),
                evidence,
            )
        } else if message.contains("license") {
            (
                DoctorSeverity::Warning,
                "deps.rust.license.reported",
                Some(String::from(
                    "Tighten your deny.toml license policy or replace the offending crate.",
                )),
                Vec::new(),
            )
        } else if message.contains("source") {
            (
                DoctorSeverity::Warning,
                "deps.rust.source.reported",
                Some(String::from(
                    "Review your dependency source allowlist and remove unexpected registries or git sources.",
                )),
                Vec::new(),
            )
        } else if diagnostic_code == "duplicate"
            || message.contains("ban")
            || message.contains("duplicate")
        {
            (
                DoctorSeverity::Warning,
                "deps.rust.bans.reported",
                Some(String::from(
                    "Review duplicate or banned crates and align them with your dependency policy.",
                )),
                Vec::new(),
            )
        } else if level == "warn"
            || level == "error"
            || diagnostic_severity == "warn"
            || diagnostic_severity == "error"
        {
            (
                DoctorSeverity::Info,
                "deps.rust.engine.signal",
                Some(String::from(
                    "Inspect cargo-deny output directly if you need the raw upstream diagnostic context.",
                )),
                Vec::new(),
            )
        } else {
            continue;
        };

        findings.push(doctor_finding_with_detail(
            DoctorDomain::Deps,
            Some(DoctorEcosystem::Rust),
            severity,
            code,
            message,
            Some(root.join("Cargo.toml")),
            None,
            evidence,
            fix_hint,
            Some(String::from("cargo-deny")),
        ));
    }

    if !parsed_any && !combined.trim().is_empty() {
        return Err(String::from(
            "cargo-deny returned output, but ossify could not parse the JSON stream.",
        ));
    }

    if !output.status.success() && findings.is_empty() {
        return Err(String::from(
            "cargo-deny completed with a non-zero status but did not emit actionable dependency findings.",
        ));
    }

    Ok(dedup_findings(findings))
}

fn cargo_deny_advisory_class(
    fields: &serde_json::Map<String, serde_json::Value>,
    diagnostic_code: &str,
    message: &str,
) -> Option<RustAdvisoryClass> {
    let advisory = fields
        .get("advisory")
        .and_then(serde_json::Value::as_object);
    if advisory.is_none()
        && diagnostic_code != "unmaintained"
        && diagnostic_code != "yanked"
        && diagnostic_code != "unsound"
        && !message.contains("advisory")
        && !message.contains("vulnerability")
    {
        return None;
    }

    let informational = advisory
        .and_then(|advisory| advisory.get("informational"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();

    let normalized_message = message.to_ascii_lowercase();
    if informational == "unmaintained" || diagnostic_code == "unmaintained" {
        return Some(RustAdvisoryClass::Unmaintained);
    }
    if informational == "yanked"
        || diagnostic_code == "yanked"
        || normalized_message.contains("yanked")
    {
        return Some(RustAdvisoryClass::Yanked);
    }
    if informational == "unsound"
        || diagnostic_code == "unsound"
        || normalized_message.contains("unsound")
    {
        return Some(RustAdvisoryClass::Unsound);
    }
    if informational == "notice"
        || informational == "informational"
        || diagnostic_code == "notice"
        || normalized_message.contains("informational advisory")
    {
        return Some(RustAdvisoryClass::Informational);
    }

    if let Some(cvss) = advisory
        .and_then(|advisory| advisory.get("cvss"))
        .and_then(cvss_score)
    {
        return Some(match cvss {
            score if score >= 9.0 => RustAdvisoryClass::Critical,
            score if score >= 7.0 => RustAdvisoryClass::High,
            score if score >= 4.0 => RustAdvisoryClass::Medium,
            _ => RustAdvisoryClass::Low,
        });
    }

    if normalized_message.contains("critical") {
        return Some(RustAdvisoryClass::Critical);
    }
    if normalized_message.contains("high severity") || normalized_message.contains("high-severity")
    {
        return Some(RustAdvisoryClass::High);
    }
    if normalized_message.contains("medium severity")
        || normalized_message.contains("moderate")
        || normalized_message.contains("medium-severity")
    {
        return Some(RustAdvisoryClass::Medium);
    }
    if normalized_message.contains("low severity") || normalized_message.contains("low-severity") {
        return Some(RustAdvisoryClass::Low);
    }

    Some(RustAdvisoryClass::Reported)
}

fn cvss_score(value: &serde_json::Value) -> Option<f64> {
    match value {
        serde_json::Value::Number(number) => number.as_f64(),
        serde_json::Value::String(text) => text.parse::<f64>().ok(),
        serde_json::Value::Object(map) => map
            .get("score")
            .and_then(cvss_score)
            .or_else(|| map.get("base_score").and_then(cvss_score)),
        _ => None,
    }
}

fn cargo_deny_advisory_evidence(
    fields: &serde_json::Map<String, serde_json::Value>,
    class: RustAdvisoryClass,
) -> Vec<String> {
    let mut evidence = vec![format!("advisory.class={}", class.code())];
    if let Some(advisory) = fields
        .get("advisory")
        .and_then(serde_json::Value::as_object)
    {
        if let Some(id) = advisory.get("id").and_then(serde_json::Value::as_str) {
            evidence.push(format!("advisory.id={id}"));
        }
        if let Some(package) = advisory.get("package").and_then(serde_json::Value::as_str) {
            evidence.push(format!("crate={package}"));
        }
        if let Some(informational) = advisory
            .get("informational")
            .and_then(serde_json::Value::as_str)
        {
            evidence.push(format!("advisory.informational={informational}"));
        }
        if let Some(cvss) = advisory.get("cvss").and_then(cvss_score) {
            evidence.push(format!("advisory.cvss={cvss:.1}"));
        }
    }

    if let Some(version) = fields
        .get("graphs")
        .and_then(serde_json::Value::as_array)
        .and_then(|graphs| graphs.first())
        .and_then(|graph| graph.get("Krate"))
        .and_then(serde_json::Value::as_object)
        .and_then(|krate| krate.get("version"))
        .and_then(serde_json::Value::as_str)
    {
        evidence.push(format!("version={version}"));
    }

    evidence
}

fn parse_audit_ci_output(
    root: &Path,
    output: &std::process::Output,
) -> Result<Vec<DoctorFinding>, String> {
    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        return if output.status.success() {
            Ok(Vec::new())
        } else {
            Err(String::from("audit-ci exited non-zero without JSON output"))
        };
    }

    let value: serde_json::Value = serde_json::from_str(stdout.trim())
        .map_err(|error| format!("invalid audit-ci JSON output: {error}"))?;
    let vulnerabilities = value
        .get("metadata")
        .and_then(|entry| entry.get("vulnerabilities"))
        .and_then(serde_json::Value::as_object)
        .cloned()
        .unwrap_or_default();
    let dependencies = value
        .get("metadata")
        .and_then(|entry| entry.get("dependencies"))
        .and_then(serde_json::Value::as_object)
        .cloned()
        .unwrap_or_default();

    let counts = [
        (
            "critical",
            DoctorSeverity::Error,
            "deps.node.vulnerability.critical",
        ),
        (
            "high",
            DoctorSeverity::Error,
            "deps.node.vulnerability.high",
        ),
        (
            "moderate",
            DoctorSeverity::Warning,
            "deps.node.vulnerability.moderate",
        ),
        ("low", DoctorSeverity::Info, "deps.node.vulnerability.low"),
    ];

    let total_dependencies = dependencies
        .get("total")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);

    let mut findings = Vec::new();
    for (key, severity, code) in counts {
        let count = vulnerabilities
            .get(key)
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        if count == 0 {
            continue;
        }
        findings.push(doctor_finding_with_detail(
            DoctorDomain::Deps,
            Some(DoctorEcosystem::Node),
            severity,
            code,
            format!(
                "audit-ci reported {count} {key} dependency vulnerabilit{} across the audited Node graph.",
                if count == 1 { "y" } else { "ies" }
            ),
            Some(root.join("package.json")),
            None,
            vec![format!("dependencies.total={total_dependencies}")],
            Some(String::from(
                "Run `audit-ci --report-type full` locally and upgrade, replace, or explicitly allowlist the flagged packages.",
            )),
            Some(String::from("audit-ci")),
        ));
    }

    if !output.status.success() && findings.is_empty() {
        return Err(String::from(
            "audit-ci completed with a non-zero status but did not emit any vulnerability counts.",
        ));
    }

    Ok(findings)
}

fn parse_pip_audit_output(
    root: &Path,
    output: &std::process::Output,
) -> Result<Vec<DoctorFinding>, String> {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}\n{stderr}");
    if stdout.trim().is_empty() {
        return if output.status.success() {
            Ok(Vec::new())
        } else {
            Err(String::from(
                "pip-audit exited non-zero without JSON output",
            ))
        };
    }

    let mut deserializer = serde_json::Deserializer::from_str(&combined);
    let value = serde_json::Value::deserialize(&mut deserializer)
        .map_err(|error| format!("invalid pip-audit JSON output: {error}"))?;
    let packages = value
        .get("dependencies")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .or_else(|| value.as_array().cloned())
        .unwrap_or_default();
    let mut vuln_count = 0usize;
    let mut evidence = Vec::new();

    for package in packages {
        let name = package
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("package");
        let vulns = package
            .get("vulns")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        vuln_count += vulns.len();
        for vuln in vulns.iter().take(2) {
            let id = vuln
                .get("id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown-vuln");
            let fix_versions = vuln
                .get("fix_versions")
                .and_then(serde_json::Value::as_array)
                .map(|versions| {
                    versions
                        .iter()
                        .filter_map(serde_json::Value::as_str)
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();
            if fix_versions.is_empty() {
                evidence.push(format!("{name}: {id}"));
            } else {
                evidence.push(format!("{name}: {id} -> {fix_versions}"));
            }
        }
    }

    if vuln_count == 0 {
        if !output.status.success() {
            return Err(String::from(
                "pip-audit completed with a non-zero status but did not emit any vulnerability records.",
            ));
        }
        return Ok(Vec::new());
    }

    Ok(vec![doctor_finding_with_detail(
        DoctorDomain::Deps,
        Some(DoctorEcosystem::Python),
        DoctorSeverity::Error,
        "deps.python.vulnerability.reported",
        format!(
            "pip-audit reported {vuln_count} known vulnerabilit{} in the Python dependency graph.",
            if vuln_count == 1 { "y" } else { "ies" }
        ),
        python_primary_file(root),
        None,
        evidence,
        Some(String::from(
            "Run `pip-audit -f json` locally and upgrade or constrain the vulnerable packages.",
        )),
        Some(String::from("pip-audit")),
    )])
}

fn python_audit_args(root: &Path) -> Vec<String> {
    let mut args = vec![
        String::from("--format"),
        String::from("json"),
        String::from("--progress-spinner"),
        String::from("off"),
    ];

    let requirements = collect_requirements_files(root).unwrap_or_default();
    if !requirements.is_empty() {
        for requirement in requirements {
            if let Some(name) = requirement.file_name().and_then(|entry| entry.to_str()) {
                args.push(String::from("--requirement"));
                args.push(name.to_owned());
            }
        }
    } else {
        if root.join("pyproject.toml").is_file() {
            args.push(String::from("--locked"));
        }
        args.push(String::from("."));
    }

    args
}

fn dedup_findings(findings: Vec<DoctorFinding>) -> Vec<DoctorFinding> {
    let mut ordered = Vec::new();
    let mut seen = BTreeSet::new();
    for finding in findings {
        let key = format!(
            "{}::{:?}::{:?}::{}::{}",
            finding.code,
            finding.severity,
            finding.ecosystem,
            finding
                .file
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_default(),
            finding.message
        );
        if seen.insert(key) {
            ordered.push(finding);
        }
    }
    ordered
}

fn ecosystem_deps_engine(ecosystem: DoctorEcosystem) -> &'static str {
    match ecosystem {
        DoctorEcosystem::Rust => "cargo-deny + ossify policy",
        DoctorEcosystem::Node => "audit-ci + ossify policy",
        DoctorEcosystem::Python => "pip-audit + ossify policy",
        DoctorEcosystem::Auto => "ossify policy",
    }
}

fn ecosystem_release_engine(ecosystem: DoctorEcosystem) -> &'static str {
    match ecosystem {
        DoctorEcosystem::Rust => "release-plz + git-cliff + cargo-dist + ossify policy",
        DoctorEcosystem::Node | DoctorEcosystem::Python => "release-please + ossify policy",
        DoctorEcosystem::Auto => "ossify policy",
    }
}

fn deps_summary(findings: &[DoctorFinding], ecosystems: &[EcosystemDoctorScore]) -> String {
    let mut summary = summarize_domain_findings(
        findings,
        "Dependency surface looks healthy.",
        "Dependency surface is carrying risk signals.",
    );
    if ecosystems
        .iter()
        .any(|entry| entry.engine_status != ManagedEngineStatus::Managed)
    {
        summary.push_str(" Some ecosystems are currently running in degraded fallback mode.");
    }
    summary
}

fn release_summary(findings: &[DoctorFinding], ecosystems: &[EcosystemDoctorScore]) -> String {
    let mut summary = summarize_domain_findings(
        findings,
        "Release surface looks healthy.",
        "Release surface is still missing maintainer-grade signals.",
    );
    if ecosystems
        .iter()
        .any(|entry| entry.engine_status != ManagedEngineStatus::Managed)
    {
        summary.push_str(" Some ecosystems are currently using degraded fallback release checks.");
    }
    summary
}

fn summarize_domain_findings(
    findings: &[DoctorFinding],
    empty_message: &str,
    non_empty_prefix: &str,
) -> String {
    let errors = findings
        .iter()
        .filter(|finding| finding.severity == DoctorSeverity::Error)
        .count();
    let warnings = findings
        .iter()
        .filter(|finding| finding.severity == DoctorSeverity::Warning)
        .count();
    let infos = findings
        .iter()
        .filter(|finding| finding.severity == DoctorSeverity::Info)
        .count();

    if errors == 0 && warnings == 0 && infos == 0 {
        return String::from(empty_message);
    }

    let mut parts = Vec::new();
    if errors > 0 {
        parts.push(format!("{errors} error(s)"));
    }
    if warnings > 0 {
        parts.push(format!("{warnings} warning(s)"));
    }
    if infos > 0 {
        parts.push(format!("{infos} info signal(s)"));
    }

    format!("{non_empty_prefix} {}", parts.join(" and "))
}

#[derive(Debug, Clone)]
struct DepsScoring {
    score: u8,
    cap: Option<u8>,
    cap_reason: Option<String>,
    cap_code: Option<String>,
}

fn deps_scoring_for_ecosystem(
    ecosystem: DoctorEcosystem,
    findings: &[DoctorFinding],
) -> DepsScoring {
    if ecosystem == DoctorEcosystem::Rust {
        let RustDepsScoringOutcome {
            score,
            cap,
            cap_reason,
            cap_code,
            ..
        } = score_rust_deps_findings(findings);
        return DepsScoring {
            score,
            cap,
            cap_reason,
            cap_code,
        };
    }

    let cap = deps_score_cap(findings);
    let mut score = deps_score(findings);
    if let Some(cap) = cap {
        score = score.min(cap);
    }

    DepsScoring {
        score,
        cap,
        cap_reason: cap
            .map(|cap| format!("{ecosystem:?} dependency findings capped score at {cap}/100")),
        cap_code: cap.map(|_| format!("deps.{}.cap", ecosystem.label())),
    }
}

fn generic_deps_penalty(finding: &DoctorFinding) -> u16 {
    match finding.code.as_str() {
        "deps.rust.lockfile.missing"
        | "deps.node.lockfile.missing"
        | "deps.python.lockfile.missing" => 10,
        code if code.contains("wildcard") => 8,
        code if code.contains("direct-source") => 8,
        code if code.contains("unpinned") => 6,
        code if code.contains("policy-missing") => 3,
        code if code.contains("license-missing") => 4,
        code if code.contains("vulnerability.critical") => 28,
        code if code.contains("vulnerability.high") => 16,
        _ => finding.severity.penalty(),
    }
}

fn deps_score(findings: &[DoctorFinding]) -> u8 {
    100u16
        .saturating_sub(findings.iter().map(generic_deps_penalty).sum::<u16>())
        .min(100) as u8
}

fn deps_score_cap(findings: &[DoctorFinding]) -> Option<u8> {
    if findings
        .iter()
        .any(|finding| finding.code.contains("vulnerability.critical"))
    {
        Some(39)
    } else if findings
        .iter()
        .any(|finding| finding.code.contains("vulnerability.high"))
    {
        Some(49)
    } else {
        None
    }
}

fn release_penalty(finding: &DoctorFinding) -> u16 {
    match finding.code.as_str() {
        code if code.contains("version.missing") => 14,
        code if code.contains("workflow.missing") => 12,
        code if code.contains("changelog.missing") => 8,
        code if code.contains("distribution.missing") => 6,
        code if code.contains("tag-signal.missing") => 6,
        _ => finding.severity.penalty(),
    }
}

fn release_score(findings: &[DoctorFinding]) -> u8 {
    100u16
        .saturating_sub(findings.iter().map(release_penalty).sum::<u16>())
        .min(100) as u8
}

fn release_score_cap(findings: &[DoctorFinding]) -> Option<u8> {
    if findings
        .iter()
        .any(|finding| finding.severity == DoctorSeverity::Error)
    {
        Some(59)
    } else {
        None
    }
}

fn analyze_rust_deps(root: &Path) -> io::Result<Vec<DoctorFinding>> {
    let manifest_path = root.join("Cargo.toml");
    if !manifest_path.is_file() {
        return Ok(vec![doctor_finding(
            DoctorDomain::Deps,
            Some(DoctorEcosystem::Rust),
            DoctorSeverity::Info,
            "deps.rust.not-detected",
            "No Cargo.toml was detected at the repository root.",
            None,
            Some(String::from(
                "Run `ossify doctor deps --ecosystem rust` from the Rust project root.",
            )),
        )]);
    }

    let manifest_text = fs::read_to_string(&manifest_path)?;
    let manifest: toml::Value = toml::from_str(&manifest_text)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
    let dependencies = collect_cargo_dependency_specs(&manifest);
    let has_lockfile = root.join("Cargo.lock").is_file();
    let has_policy = root.join("deny.toml").is_file();
    let has_license = cargo_package_string(&manifest, &["package", "license"])
        .or_else(|| cargo_package_string(&manifest, &["workspace", "package", "license"]))
        .is_some();

    let mut findings = Vec::new();
    if !dependencies.is_empty() && !has_lockfile {
        findings.push(doctor_finding_with_detail(
            DoctorDomain::Deps,
            Some(DoctorEcosystem::Rust),
            DoctorSeverity::Warning,
            "deps.rust.lockfile.missing",
            "Cargo dependencies exist but Cargo.lock is missing.",
            Some(manifest_path.clone()),
            Some(String::from(
                "Commit Cargo.lock for binaries and applications, and keep the resolved graph visible to maintainers.",
            )),
            vec![String::from("Cargo.lock")],
            Some(String::from("Generate and commit Cargo.lock.")),
            Some(String::from("cargo-deny semantics")),
        ));
    }
    if !has_license {
        findings.push(doctor_finding_with_detail(
            DoctorDomain::Deps,
            Some(DoctorEcosystem::Rust),
            DoctorSeverity::Warning,
            "deps.rust.license-missing",
            "Cargo package metadata does not declare a license expression.",
            Some(manifest_path.clone()),
            Some(String::from(
                "Set `package.license` so consumers and policy tooling can reason about the published crate.",
            )),
            vec![String::from("package.license")],
            Some(String::from("Add a valid SPDX license in Cargo.toml.")),
            Some(String::from("cargo-deny semantics")),
        ));
    }
    if !has_policy {
        findings.push(doctor_finding_with_detail(
            DoctorDomain::Deps,
            Some(DoctorEcosystem::Rust),
            DoctorSeverity::Info,
            "deps.rust.policy-missing",
            "No `deny.toml` policy file was found for cargo-deny-style checks.",
            Some(root.join("deny.toml")),
            Some(String::from(
                "A visible dependency policy makes advisories, licenses, bans, and source exceptions auditable.",
            )),
            vec![String::from("deny.toml")],
            Some(String::from("Add a deny.toml policy with explicit allowlists and ignores.")),
            Some(String::from("cargo-deny semantics")),
        ));
    }

    let mut wildcard = Vec::new();
    let mut direct_git = Vec::new();
    let mut local_path = Vec::new();
    let mut custom_registry = Vec::new();

    for (name, spec) in dependencies {
        match spec {
            CargoDepSpec::Version(version) => {
                if version.trim() == "*" {
                    wildcard.push(name);
                }
            }
            CargoDepSpec::Table(table) => {
                if table
                    .get("version")
                    .and_then(toml::Value::as_str)
                    .map(|value| value.trim() == "*")
                    .unwrap_or(false)
                {
                    wildcard.push(name.clone());
                }
                if table.contains_key("git") {
                    direct_git.push(name.clone());
                }
                if table.contains_key("path") {
                    local_path.push(name.clone());
                }
                if table.contains_key("registry") {
                    custom_registry.push(name);
                }
            }
        }
    }

    if !wildcard.is_empty() {
        findings.push(doctor_finding_with_detail(
            DoctorDomain::Deps,
            Some(DoctorEcosystem::Rust),
            DoctorSeverity::Warning,
            "deps.rust.version.wildcard",
            format!(
                "Wildcard Cargo dependency versions were found: {}.",
                wildcard.join(", ")
            ),
            Some(manifest_path.clone()),
            Some(String::from(
                "Prefer explicit version ranges so upgrades stay reviewable and repeatable.",
            )),
            wildcard,
            Some(String::from(
                "Replace `*` version requirements with explicit semver ranges.",
            )),
            Some(String::from("cargo-deny semantics")),
        ));
    }
    if !direct_git.is_empty() {
        findings.push(doctor_finding_with_detail(
            DoctorDomain::Deps,
            Some(DoctorEcosystem::Rust),
            DoctorSeverity::Warning,
            "deps.rust.source.direct-source",
            format!("Git-sourced Cargo dependencies were found: {}.", direct_git.join(", ")),
            Some(manifest_path.clone()),
            Some(String::from(
                "Git dependencies can be necessary, but they deserve explicit justification and review because they bypass crates.io release hygiene.",
            )),
            direct_git,
            Some(String::from(
                "Prefer crates.io releases where possible, or document why the git dependency is required.",
            )),
            Some(String::from("cargo-deny semantics")),
        ));
    }
    if !local_path.is_empty() {
        findings.push(doctor_finding_with_detail(
            DoctorDomain::Deps,
            Some(DoctorEcosystem::Rust),
            DoctorSeverity::Info,
            "deps.rust.source.path",
            format!("Local path Cargo dependencies were found: {}.", local_path.join(", ")),
            Some(manifest_path.clone()),
            Some(String::from(
                "Path dependencies are fine in a monorepo, but they are still worth surfacing as part of the dependency trust boundary.",
            )),
            local_path,
            Some(String::from(
                "Keep local path dependencies intentional and documented.",
            )),
            Some(String::from("cargo-deny semantics")),
        ));
    }
    if !custom_registry.is_empty() {
        findings.push(doctor_finding_with_detail(
            DoctorDomain::Deps,
            Some(DoctorEcosystem::Rust),
            DoctorSeverity::Info,
            "deps.rust.source.registry",
            format!(
                "Non-default Cargo registries were referenced: {}.",
                custom_registry.join(", ")
            ),
            Some(manifest_path),
            Some(String::from(
                "Alternative registries should stay visible so maintainers can review provenance and access assumptions.",
            )),
            custom_registry,
            Some(String::from(
                "Document the alternative registry and why it is trusted.",
            )),
            Some(String::from("cargo-deny semantics")),
        ));
    }

    Ok(findings)
}

fn analyze_node_deps(root: &Path) -> io::Result<Vec<DoctorFinding>> {
    let manifest_path = root.join("package.json");
    if !manifest_path.is_file() {
        return Ok(vec![doctor_finding(
            DoctorDomain::Deps,
            Some(DoctorEcosystem::Node),
            DoctorSeverity::Info,
            "deps.node.not-detected",
            "No package.json was detected at the repository root.",
            None,
            Some(String::from(
                "Run `ossify doctor deps --ecosystem node` from the Node project root.",
            )),
        )]);
    }

    let manifest_text = fs::read_to_string(&manifest_path)?;
    let manifest: serde_json::Value = serde_json::from_str(&manifest_text)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
    let dependencies = collect_node_dependency_specs(&manifest);
    let has_lockfile = [
        "package-lock.json",
        "npm-shrinkwrap.json",
        "pnpm-lock.yaml",
        "yarn.lock",
    ]
    .iter()
    .any(|name| root.join(name).is_file());
    let has_policy = [
        "audit-ci.json",
        "audit-ci.jsonc",
        ".audit-ci.json",
        ".audit-ci.jsonc",
    ]
    .iter()
    .any(|name| root.join(name).is_file());
    let has_license = manifest
        .get("license")
        .and_then(serde_json::Value::as_str)
        .is_some();

    let mut findings = Vec::new();
    if !dependencies.is_empty() && !has_lockfile {
        findings.push(doctor_finding_with_detail(
            DoctorDomain::Deps,
            Some(DoctorEcosystem::Node),
            DoctorSeverity::Warning,
            "deps.node.lockfile.missing",
            "package.json declares dependencies but no lockfile was found.",
            Some(manifest_path.clone()),
            Some(String::from(
                "Keep the resolved npm graph visible with a checked-in lockfile so reviews and CI are reproducible.",
            )),
            vec![String::from("package-lock.json | pnpm-lock.yaml | yarn.lock")],
            Some(String::from("Commit the lockfile used by your package manager.")),
            Some(String::from("audit-ci semantics")),
        ));
    }
    if !has_license {
        findings.push(doctor_finding_with_detail(
            DoctorDomain::Deps,
            Some(DoctorEcosystem::Node),
            DoctorSeverity::Warning,
            "deps.node.license-missing",
            "package.json does not declare a license field.",
            Some(manifest_path.clone()),
            Some(String::from(
                "A visible license helps downstream consumers and dependency policy checks reason about the package surface.",
            )),
            vec![String::from("license")],
            Some(String::from("Add a license field to package.json.")),
            Some(String::from("audit-ci semantics")),
        ));
    }
    if !has_policy {
        findings.push(doctor_finding_with_detail(
            DoctorDomain::Deps,
            Some(DoctorEcosystem::Node),
            DoctorSeverity::Info,
            "deps.node.policy-missing",
            "No visible audit-ci policy file was found.",
            Some(root.join("audit-ci.jsonc")),
            Some(String::from(
                "A repository-owned audit policy makes dependency ignores and gating thresholds reviewable.",
            )),
            vec![String::from("audit-ci.jsonc")],
            Some(String::from("Add an audit-ci policy file or document how npm audit is gated.")),
            Some(String::from("audit-ci semantics")),
        ));
    }

    let mut wildcard = Vec::new();
    let mut direct_source = Vec::new();
    let mut local_source = Vec::new();
    for (name, version) in dependencies {
        let normalized = version.trim().to_ascii_lowercase();
        if matches!(normalized.as_str(), "*" | "latest") {
            wildcard.push(name);
        } else if normalized.starts_with("git+")
            || normalized.starts_with("github:")
            || normalized.starts_with("http://")
            || normalized.starts_with("https://")
        {
            direct_source.push(name);
        } else if normalized.starts_with("file:") || normalized.starts_with("link:") {
            local_source.push(name);
        }
    }

    if !wildcard.is_empty() {
        findings.push(doctor_finding_with_detail(
            DoctorDomain::Deps,
            Some(DoctorEcosystem::Node),
            DoctorSeverity::Warning,
            "deps.node.version.wildcard",
            format!(
                "Wildcard or `latest` npm dependency specs were found: {}.",
                wildcard.join(", ")
            ),
            Some(manifest_path.clone()),
            Some(String::from(
                "Prefer explicit semver ranges over `*` or `latest` so upgrades stay reviewable.",
            )),
            wildcard,
            Some(String::from(
                "Replace `*` or `latest` with explicit semver ranges.",
            )),
            Some(String::from("audit-ci semantics")),
        ));
    }
    if !direct_source.is_empty() {
        findings.push(doctor_finding_with_detail(
            DoctorDomain::Deps,
            Some(DoctorEcosystem::Node),
            DoctorSeverity::Warning,
            "deps.node.source.direct-source",
            format!("Direct git or URL npm dependencies were found: {}.", direct_source.join(", ")),
            Some(manifest_path.clone()),
            Some(String::from(
                "Direct-source npm dependencies deserve explicit maintainer review because they bypass the usual registry release path.",
            )),
            direct_source,
            Some(String::from(
                "Prefer published registry packages, or document why the direct source is required.",
            )),
            Some(String::from("audit-ci semantics")),
        ));
    }
    if !local_source.is_empty() {
        findings.push(doctor_finding_with_detail(
            DoctorDomain::Deps,
            Some(DoctorEcosystem::Node),
            DoctorSeverity::Info,
            "deps.node.source.local",
            format!("Local npm dependency specs were found: {}.", local_source.join(", ")),
            Some(manifest_path),
            Some(String::from(
                "Local workspace or file dependencies are legitimate, but they still change the trust boundary and should stay visible.",
            )),
            local_source,
            Some(String::from(
                "Keep local dependency links intentional and documented.",
            )),
            Some(String::from("audit-ci semantics")),
        ));
    }

    Ok(findings)
}

fn analyze_python_deps(root: &Path) -> io::Result<Vec<DoctorFinding>> {
    let pyproject_path = root.join("pyproject.toml");
    let requirements_files = collect_requirements_files(root)?;

    if !pyproject_path.is_file() && requirements_files.is_empty() {
        return Ok(vec![doctor_finding(
            DoctorDomain::Deps,
            Some(DoctorEcosystem::Python),
            DoctorSeverity::Info,
            "deps.python.not-detected",
            "No pyproject.toml or requirements files were detected at the repository root.",
            None,
            Some(String::from(
                "Run `ossify doctor deps --ecosystem python` from the Python project root.",
            )),
        )]);
    }

    let mut findings = Vec::new();
    let has_lockfile = [
        "uv.lock",
        "poetry.lock",
        "Pipfile.lock",
        "pdm.lock",
        "requirements.lock",
    ]
    .iter()
    .any(|name| root.join(name).is_file());

    let mut requirements = Vec::new();
    for file in &requirements_files {
        requirements.extend(parse_requirements(file)?);
    }

    let mut pyproject_license = None::<String>;
    let mut pyproject_dependencies = Vec::<String>::new();
    if pyproject_path.is_file() {
        let pyproject_text = fs::read_to_string(&pyproject_path)?;
        let pyproject: toml::Value = toml::from_str(&pyproject_text)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
        pyproject_license = python_license_value(&pyproject);
        pyproject_dependencies = python_pyproject_dependencies(&pyproject);
    }

    let has_deps = !requirements.is_empty() || !pyproject_dependencies.is_empty();
    if has_deps && !has_lockfile {
        findings.push(doctor_finding_with_detail(
            DoctorDomain::Deps,
            Some(DoctorEcosystem::Python),
            DoctorSeverity::Warning,
            "deps.python.lockfile.missing",
            "Python dependencies are declared but no lockfile was found.",
            Some(pyproject_path.clone()),
            Some(String::from(
                "Visible lockfiles make Python environments and audits more reproducible across machines and CI.",
            )),
            vec![String::from("uv.lock | poetry.lock | Pipfile.lock | pdm.lock")],
            Some(String::from("Commit the lockfile used by your Python workflow.")),
            Some(String::from("pip-audit semantics")),
        ));
    }
    if pyproject_path.is_file() && pyproject_license.is_none() {
        findings.push(doctor_finding_with_detail(
            DoctorDomain::Deps,
            Some(DoctorEcosystem::Python),
            DoctorSeverity::Warning,
            "deps.python.license-missing",
            "pyproject.toml does not declare a visible license signal.",
            Some(pyproject_path.clone()),
            Some(String::from(
                "Set a visible project license so consumers and policy tooling can reason about the package surface.",
            )),
            vec![String::from("project.license")],
            Some(String::from("Declare the project license in pyproject.toml.")),
            Some(String::from("pip-audit semantics")),
        ));
    }
    if !workflow_mentions(
        root,
        &["pip-audit", "python -m pip_audit", "python -m pip-audit"],
    )? {
        findings.push(doctor_finding_with_detail(
            DoctorDomain::Deps,
            Some(DoctorEcosystem::Python),
            DoctorSeverity::Info,
            "deps.python.policy-missing",
            "No visible pip-audit-style dependency gate was found in the repository workflows.",
            Some(root.join(".github/workflows")),
            Some(String::from(
                "A visible dependency audit step makes Python advisory handling easier to review over time.",
            )),
            vec![String::from("pip-audit")],
            Some(String::from("Add a pip-audit step or document the equivalent dependency gate.")),
            Some(String::from("pip-audit semantics")),
        ));
    }

    let mut unpinned = Vec::new();
    let mut direct_source = Vec::new();
    let mut editable = Vec::new();
    for requirement in requirements.iter().chain(pyproject_dependencies.iter()) {
        let normalized = requirement.trim().to_ascii_lowercase();
        if normalized.starts_with("-e ") || normalized.starts_with("--editable") {
            editable.push(requirement.clone());
        } else if normalized.starts_with("git+")
            || normalized.starts_with("http://")
            || normalized.starts_with("https://")
        {
            direct_source.push(requirement.clone());
        } else if !normalized.contains("==") {
            unpinned.push(requirement.clone());
        }
    }

    if !unpinned.is_empty() {
        findings.push(doctor_finding_with_detail(
            DoctorDomain::Deps,
            Some(DoctorEcosystem::Python),
            DoctorSeverity::Info,
            "deps.python.version.unpinned",
            format!(
                "Python dependency specs without exact pins were found: {}.",
                join_preview(&unpinned)
            ),
            Some(first_existing_path(std::slice::from_ref(&pyproject_path), &requirements_files)),
            Some(String::from(
                "Exact pins are not always required, but surfacing looser Python dependency specs makes the supply surface more reviewable.",
            )),
            unpinned,
            Some(String::from(
                "Consider lockfiles or exact pins where reproducibility matters.",
            )),
            Some(String::from("pip-audit semantics")),
        ));
    }
    if !direct_source.is_empty() {
        findings.push(doctor_finding_with_detail(
            DoctorDomain::Deps,
            Some(DoctorEcosystem::Python),
            DoctorSeverity::Warning,
            "deps.python.source.direct-source",
            format!(
                "Direct URL or VCS Python dependencies were found: {}.",
                join_preview(&direct_source)
            ),
            Some(first_existing_path(std::slice::from_ref(&pyproject_path), &requirements_files)),
            Some(String::from(
                "Direct-source Python dependencies deserve explicit review because they bypass the normal release path on package indexes.",
            )),
            direct_source,
            Some(String::from(
                "Prefer released packages where possible, or document why the direct source is required.",
            )),
            Some(String::from("pip-audit semantics")),
        ));
    }
    if !editable.is_empty() {
        findings.push(doctor_finding_with_detail(
            DoctorDomain::Deps,
            Some(DoctorEcosystem::Python),
            DoctorSeverity::Info,
            "deps.python.source.editable",
            format!(
                "Editable Python dependency installs were found: {}.",
                join_preview(&editable)
            ),
            Some(first_existing_path(&[pyproject_path], &requirements_files)),
            Some(String::from(
                "Editable installs are common in dev workflows, but they still widen the effective dependency boundary and should stay visible.",
            )),
            editable,
            Some(String::from(
                "Keep editable dependency usage intentional and documented.",
            )),
            Some(String::from("pip-audit semantics")),
        ));
    }

    Ok(findings)
}

fn analyze_rust_release(root: &Path) -> io::Result<Vec<DoctorFinding>> {
    let manifest_path = root.join("Cargo.toml");
    if !manifest_path.is_file() {
        return Ok(vec![doctor_finding(
            DoctorDomain::Release,
            Some(DoctorEcosystem::Rust),
            DoctorSeverity::Info,
            "release.rust.not-detected",
            "No Cargo.toml was detected at the repository root.",
            None,
            Some(String::from(
                "Run `ossify doctor release --ecosystem rust` from the Rust project root.",
            )),
        )]);
    }

    let manifest_text = fs::read_to_string(&manifest_path)?;
    let manifest: toml::Value = toml::from_str(&manifest_text)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
    let changelog = root.join("CHANGELOG.md");
    let version = cargo_package_string(&manifest, &["package", "version"])
        .or_else(|| cargo_package_string(&manifest, &["workspace", "package", "version"]));
    let has_release_flow = workflow_mentions(
        root,
        &[
            "cargo publish",
            "release-plz",
            "cargo-dist",
            "git-cliff",
            "softprops/action-gh-release",
            "gh release",
        ],
    )?;
    let has_dist_signal = manifest_text.contains("workspace.metadata.dist")
        || manifest_text.contains("package.metadata.dist")
        || workflow_mentions(root, &["cargo-dist", "dist build", "dist plan"])?;
    let has_tags = visible_git_tag_count(root)? > 0;

    let mut findings = Vec::new();
    if version.is_none() {
        findings.push(doctor_finding_with_detail(
            DoctorDomain::Release,
            Some(DoctorEcosystem::Rust),
            DoctorSeverity::Error,
            "release.rust.version.missing",
            "Cargo package metadata does not expose a visible release version.",
            Some(manifest_path.clone()),
            Some(String::from(
                "A release-oriented crate should expose a version field so changelogs, tags, and publishing flows can stay coherent.",
            )),
            vec![String::from("package.version")],
            Some(String::from("Add a version field to Cargo.toml.")),
            Some(String::from("release-plz semantics")),
        ));
    }
    if !changelog.is_file() {
        findings.push(doctor_finding_with_detail(
            DoctorDomain::Release,
            Some(DoctorEcosystem::Rust),
            DoctorSeverity::Warning,
            "release.rust.changelog.missing",
            "No CHANGELOG.md was found for Rust release notes.",
            Some(changelog),
            Some(String::from(
                "A visible changelog makes upgrades and release cadence easier to trust before reading commits.",
            )),
            vec![String::from("CHANGELOG.md")],
            Some(String::from("Add and maintain CHANGELOG.md.")),
            Some(String::from("git-cliff semantics")),
        ));
    }
    if !has_release_flow {
        findings.push(doctor_finding_with_detail(
            DoctorDomain::Release,
            Some(DoctorEcosystem::Rust),
            DoctorSeverity::Error,
            "release.rust.workflow.missing",
            "No Rust release automation signal was detected in GitHub workflows.",
            Some(root.join(".github/workflows")),
            Some(String::from(
                "Release automation keeps version bumps, changelog generation, and publishing more repeatable for maintainers.",
            )),
            vec![String::from("release workflow")],
            Some(String::from(
                "Add a release workflow using release-plz, cargo publish, or an equivalent shipping flow.",
            )),
            Some(String::from("release-plz semantics")),
        ));
    }
    if !has_dist_signal {
        findings.push(doctor_finding_with_detail(
            DoctorDomain::Release,
            Some(DoctorEcosystem::Rust),
            DoctorSeverity::Info,
            "release.rust.distribution.missing",
            "No visible cargo-dist-style distribution signal was detected.",
            Some(manifest_path),
            Some(String::from(
                "If the project ships binaries, visible packaging or installer automation makes the distribution path easier to trust.",
            )),
            vec![String::from("cargo-dist")],
            Some(String::from(
                "Add cargo-dist or document how release artifacts and installers are produced.",
            )),
            Some(String::from("cargo-dist semantics")),
        ));
    }
    if !has_tags {
        findings.push(doctor_finding_with_detail(
            DoctorDomain::Release,
            Some(DoctorEcosystem::Rust),
            DoctorSeverity::Info,
            "release.rust.tag-signal.missing",
            "No visible Git tag signal was detected in the repository history.",
            Some(root.join(".git")),
            Some(String::from(
                "Tags are not mandatory, but they make release history far easier to browse and verify.",
            )),
            vec![String::from("git tags")],
            Some(String::from("Publish version tags for real releases.")),
            Some(String::from("release-plz semantics")),
        ));
    }

    Ok(findings)
}

fn analyze_node_release(root: &Path) -> io::Result<Vec<DoctorFinding>> {
    let manifest_path = root.join("package.json");
    if !manifest_path.is_file() {
        return Ok(vec![doctor_finding(
            DoctorDomain::Release,
            Some(DoctorEcosystem::Node),
            DoctorSeverity::Info,
            "release.node.not-detected",
            "No package.json was detected at the repository root.",
            None,
            Some(String::from(
                "Run `ossify doctor release --ecosystem node` from the Node project root.",
            )),
        )]);
    }

    let manifest_text = fs::read_to_string(&manifest_path)?;
    let manifest: serde_json::Value = serde_json::from_str(&manifest_text)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
    let is_private = manifest
        .get("private")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let version = manifest.get("version").and_then(serde_json::Value::as_str);
    let changelog = root.join("CHANGELOG.md");
    let has_release_flow = workflow_mentions(
        root,
        &[
            "npm publish",
            "pnpm publish",
            "yarn npm publish",
            "release-please",
            "changesets",
            "semantic-release",
        ],
    )?;
    let has_tags = visible_git_tag_count(root)? > 0;

    let mut findings = Vec::new();
    if !is_private && version.is_none() {
        findings.push(doctor_finding_with_detail(
            DoctorDomain::Release,
            Some(DoctorEcosystem::Node),
            DoctorSeverity::Error,
            "release.node.version.missing",
            "package.json does not expose a version for a publishable package.",
            Some(manifest_path.clone()),
            Some(String::from(
                "Versioning needs to be explicit if the package is meant to be published or consumed as a release surface.",
            )),
            vec![String::from("version")],
            Some(String::from("Add a version field to package.json.")),
            Some(String::from("release-please semantics")),
        ));
    }
    if !changelog.is_file() {
        findings.push(doctor_finding_with_detail(
            DoctorDomain::Release,
            Some(DoctorEcosystem::Node),
            DoctorSeverity::Warning,
            "release.node.changelog.missing",
            "No CHANGELOG.md was found for Node release notes.",
            Some(changelog),
            Some(String::from(
                "A changelog helps consumers evaluate upgrades without diffing commits or package metadata by hand.",
            )),
            vec![String::from("CHANGELOG.md")],
            Some(String::from("Add and maintain CHANGELOG.md.")),
            Some(String::from("release-please semantics")),
        ));
    }
    if !is_private && !has_release_flow {
        findings.push(doctor_finding_with_detail(
            DoctorDomain::Release,
            Some(DoctorEcosystem::Node),
            DoctorSeverity::Error,
            "release.node.workflow.missing",
            "No Node release automation signal was detected in GitHub workflows.",
            Some(root.join(".github/workflows")),
            Some(String::from(
                "A publishable Node package should expose how versioning and release publication are handled.",
            )),
            vec![String::from("release workflow")],
            Some(String::from(
                "Add release automation with release-please, changesets, semantic-release, or a documented npm publish flow.",
            )),
            Some(String::from("release-please semantics")),
        ));
    }
    if !is_private && !has_tags {
        findings.push(doctor_finding_with_detail(
            DoctorDomain::Release,
            Some(DoctorEcosystem::Node),
            DoctorSeverity::Info,
            "release.node.tag-signal.missing",
            "No visible Git tag signal was detected for Node releases.",
            Some(root.join(".git")),
            Some(String::from(
                "Tags are not mandatory, but they make release history and rollback points much easier to inspect.",
            )),
            vec![String::from("git tags")],
            Some(String::from("Tag published or announced releases.")),
            Some(String::from("release-please semantics")),
        ));
    }

    Ok(findings)
}

fn analyze_python_release(root: &Path) -> io::Result<Vec<DoctorFinding>> {
    let pyproject_path = root.join("pyproject.toml");
    if !pyproject_path.is_file() && !has_requirements_files(root) {
        return Ok(vec![doctor_finding(
            DoctorDomain::Release,
            Some(DoctorEcosystem::Python),
            DoctorSeverity::Info,
            "release.python.not-detected",
            "No pyproject.toml or requirements files were detected at the repository root.",
            None,
            Some(String::from(
                "Run `ossify doctor release --ecosystem python` from the Python project root.",
            )),
        )]);
    }

    let mut version = None::<String>;
    if pyproject_path.is_file() {
        let pyproject_text = fs::read_to_string(&pyproject_path)?;
        let pyproject: toml::Value = toml::from_str(&pyproject_text)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
        version = python_version_value(&pyproject);
    }
    let changelog = root.join("CHANGELOG.md");
    let has_release_flow = workflow_mentions(
        root,
        &[
            "pypi",
            "twine upload",
            "python -m build",
            "release-please",
            "python-semantic-release",
        ],
    )?;
    let has_tags = visible_git_tag_count(root)? > 0;

    let mut findings = Vec::new();
    if version.is_none() {
        findings.push(doctor_finding_with_detail(
            DoctorDomain::Release,
            Some(DoctorEcosystem::Python),
            DoctorSeverity::Error,
            "release.python.version.missing",
            "No visible Python release version signal was detected in pyproject.toml.",
            Some(pyproject_path.clone()),
            Some(String::from(
                "Release-oriented Python projects should expose either a version or a visible dynamic versioning strategy.",
            )),
            vec![String::from("project.version | tool.poetry.version")],
            Some(String::from(
                "Add a version field or make the dynamic versioning strategy explicit in the repo.",
            )),
            Some(String::from("release-please semantics")),
        ));
    }
    if !changelog.is_file() {
        findings.push(doctor_finding_with_detail(
            DoctorDomain::Release,
            Some(DoctorEcosystem::Python),
            DoctorSeverity::Warning,
            "release.python.changelog.missing",
            "No CHANGELOG.md was found for Python release notes.",
            Some(changelog),
            Some(String::from(
                "A visible changelog makes it much easier to audit package evolution before upgrading or publishing.",
            )),
            vec![String::from("CHANGELOG.md")],
            Some(String::from("Add and maintain CHANGELOG.md.")),
            Some(String::from("release-please semantics")),
        ));
    }
    if !has_release_flow {
        findings.push(doctor_finding_with_detail(
            DoctorDomain::Release,
            Some(DoctorEcosystem::Python),
            DoctorSeverity::Error,
            "release.python.workflow.missing",
            "No Python release automation signal was detected in GitHub workflows.",
            Some(root.join(".github/workflows")),
            Some(String::from(
                "Release automation makes Python build and publication behavior visible and repeatable for maintainers.",
            )),
            vec![String::from("release workflow")],
            Some(String::from(
                "Add release automation for build and publication, or document the release process clearly.",
            )),
            Some(String::from("release-please semantics")),
        ));
    }
    if !has_tags {
        findings.push(doctor_finding_with_detail(
            DoctorDomain::Release,
            Some(DoctorEcosystem::Python),
            DoctorSeverity::Info,
            "release.python.tag-signal.missing",
            "No visible Git tag signal was detected for Python releases.",
            Some(root.join(".git")),
            Some(String::from(
                "Tags help maintainers and consumers map published versions back to repository history.",
            )),
            vec![String::from("git tags")],
            Some(String::from("Tag published or announced releases.")),
            Some(String::from("release-please semantics")),
        ));
    }

    Ok(findings)
}

#[derive(Debug, Clone)]
enum CargoDepSpec {
    Version(String),
    Table(toml::Table),
}

fn collect_cargo_dependency_specs(manifest: &toml::Value) -> BTreeMap<String, CargoDepSpec> {
    let mut specs = BTreeMap::new();
    for path in [
        ["dependencies"].as_slice(),
        ["dev-dependencies"].as_slice(),
        ["build-dependencies"].as_slice(),
        ["workspace", "dependencies"].as_slice(),
    ] {
        let Some(table) = toml_table_at(manifest, path) else {
            continue;
        };

        for (name, value) in table {
            let spec = match value {
                toml::Value::String(version) => CargoDepSpec::Version(version.clone()),
                toml::Value::Table(table) => CargoDepSpec::Table(table.clone()),
                _ => continue,
            };
            specs.entry(name.clone()).or_insert(spec);
        }
    }

    specs
}

fn cargo_package_string(manifest: &toml::Value, path: &[&str]) -> Option<String> {
    let mut current = manifest;
    for segment in path {
        current = current.get(*segment)?;
    }
    current.as_str().map(str::to_owned)
}

fn toml_table_at<'a>(value: &'a toml::Value, path: &[&str]) -> Option<&'a toml::Table> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }
    current.as_table()
}

fn collect_node_dependency_specs(manifest: &serde_json::Value) -> BTreeMap<String, String> {
    let mut specs = BTreeMap::new();
    for section in [
        "dependencies",
        "devDependencies",
        "peerDependencies",
        "optionalDependencies",
    ] {
        let Some(table) = manifest.get(section).and_then(serde_json::Value::as_object) else {
            continue;
        };
        for (name, value) in table {
            if let Some(version) = value.as_str() {
                specs
                    .entry(name.clone())
                    .or_insert_with(|| version.to_owned());
            }
        }
    }
    specs
}

fn has_requirements_files(root: &Path) -> bool {
    collect_requirements_files(root)
        .map(|files| !files.is_empty())
        .unwrap_or(false)
}

fn collect_requirements_files(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut builder = WalkBuilder::new(root);
    builder.hidden(false);
    builder.git_ignore(true);
    builder.git_global(true);
    builder.git_exclude(true);

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
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        let normalized = name.to_ascii_lowercase();
        if normalized.starts_with("requirements")
            && (normalized.ends_with(".txt") || normalized.ends_with(".in"))
        {
            files.push(path);
        }
    }

    files.sort();
    Ok(files)
}

fn python_primary_file(root: &Path) -> Option<PathBuf> {
    collect_requirements_files(root)
        .ok()
        .and_then(|files| files.into_iter().next())
        .or_else(|| {
            let pyproject = root.join("pyproject.toml");
            pyproject.is_file().then_some(pyproject)
        })
}

fn parse_requirements(path: &Path) -> io::Result<Vec<String>> {
    let contents = fs::read_to_string(path)?;
    let mut requirements = Vec::new();
    let mut current = String::new();

    for raw_line in contents.lines() {
        let mut line = raw_line.trim().to_owned();
        if let Some((prefix, _)) = line.split_once(" #") {
            line = prefix.trim().to_owned();
        }
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if line.ends_with('\\') {
            current.push_str(line.trim_end_matches('\\').trim_end());
            continue;
        }

        if !current.is_empty() {
            current.push_str(&line);
            line = current.clone();
            current.clear();
        }

        if line.starts_with("-r ")
            || line.starts_with("--requirement ")
            || line.starts_with("-c ")
            || line.starts_with("--constraint ")
        {
            continue;
        }

        requirements.push(line);
    }

    if !current.is_empty() {
        requirements.push(current);
    }

    Ok(requirements)
}

fn python_license_value(pyproject: &toml::Value) -> Option<String> {
    if let Some(license) = cargo_package_string(pyproject, &["project", "license"]) {
        return Some(license);
    }

    if let Some(table) = toml_table_at(pyproject, &["project", "license"]) {
        if let Some(text) = table.get("text").and_then(toml::Value::as_str) {
            return Some(text.to_owned());
        }
        if let Some(file) = table.get("file").and_then(toml::Value::as_str) {
            return Some(file.to_owned());
        }
    }

    cargo_package_string(pyproject, &["tool", "poetry", "license"])
}

fn python_version_value(pyproject: &toml::Value) -> Option<String> {
    if let Some(version) = cargo_package_string(pyproject, &["project", "version"]) {
        return Some(version);
    }
    if let Some(version) = cargo_package_string(pyproject, &["tool", "poetry", "version"]) {
        return Some(version);
    }

    let dynamic_version = pyproject
        .get("project")
        .and_then(|value| value.get("dynamic"))
        .and_then(toml::Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(toml::Value::as_str)
                .any(|value| value == "version")
        })
        .unwrap_or(false);

    if dynamic_version
        || toml_table_at(pyproject, &["tool", "setuptools_scm"]).is_some()
        || toml_table_at(pyproject, &["tool", "hatch", "version"]).is_some()
    {
        Some(String::from("dynamic"))
    } else {
        None
    }
}

fn python_pyproject_dependencies(pyproject: &toml::Value) -> Vec<String> {
    let mut dependencies = Vec::new();

    if let Some(values) = pyproject
        .get("project")
        .and_then(|value| value.get("dependencies"))
        .and_then(toml::Value::as_array)
    {
        dependencies.extend(
            values
                .iter()
                .filter_map(toml::Value::as_str)
                .map(str::to_owned),
        );
    }

    if let Some(table) = toml_table_at(pyproject, &["project", "optional-dependencies"]) {
        for value in table.values() {
            if let Some(values) = value.as_array() {
                dependencies.extend(
                    values
                        .iter()
                        .filter_map(toml::Value::as_str)
                        .map(str::to_owned),
                );
            }
        }
    }

    if let Some(table) = toml_table_at(pyproject, &["tool", "poetry", "dependencies"]) {
        for (name, value) in table {
            if name == "python" {
                continue;
            }
            match value {
                toml::Value::String(version) => dependencies.push(format!("{name}{version}")),
                toml::Value::Table(table) => {
                    if let Some(version) = table.get("version").and_then(toml::Value::as_str) {
                        dependencies.push(format!("{name}{version}"));
                    } else if table.contains_key("git") || table.contains_key("path") {
                        dependencies.push(name.clone());
                    }
                }
                _ => {}
            }
        }
    }

    dependencies
}

fn workflow_mentions(root: &Path, needles: &[&str]) -> io::Result<bool> {
    let workflows = collect_workflow_files(root)?;
    if workflows.is_empty() {
        return Ok(false);
    }

    let needles = needles
        .iter()
        .map(|needle| needle.to_ascii_lowercase())
        .collect::<Vec<_>>();
    for workflow in workflows {
        let contents = fs::read_to_string(&workflow)?.to_ascii_lowercase();
        if needles.iter().any(|needle| contents.contains(needle)) {
            return Ok(true);
        }
    }

    Ok(false)
}

fn visible_git_tag_count(root: &Path) -> io::Result<usize> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["tag", "--list"])
        .output();

    match output {
        Ok(output) if output.status.success() => Ok(String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count()),
        Ok(_) => Ok(0),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(0),
        Err(error) => Err(error),
    }
}

fn join_preview(values: &[String]) -> String {
    const MAX_ITEMS: usize = 4;
    if values.len() <= MAX_ITEMS {
        return values.join(", ");
    }

    let mut preview = values.iter().take(MAX_ITEMS).cloned().collect::<Vec<_>>();
    preview.push(format!("+{} more", values.len() - MAX_ITEMS));
    preview.join(", ")
}

fn first_existing_path(primary: &[PathBuf], secondary: &[PathBuf]) -> PathBuf {
    primary
        .iter()
        .chain(secondary.iter())
        .find(|path| path.exists())
        .cloned()
        .or_else(|| primary.first().cloned())
        .or_else(|| secondary.first().cloned())
        .unwrap_or_else(|| PathBuf::from("."))
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
    #[cfg(unix)]
    use std::os::unix::process::ExitStatusExt;
    #[cfg(windows)]
    use std::os::windows::process::ExitStatusExt;
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
        let findings = vec![doctor_finding(
            DoctorDomain::Workflow,
            None,
            DoctorSeverity::Error,
            "workflow.syntax-check",
            "could not parse as YAML",
            None,
            None,
        )];

        assert_eq!(workflow_score(&findings), 49);
    }

    #[test]
    fn non_syntax_actionlint_errors_cap_workflow_score_below_seventy() {
        let findings = vec![doctor_finding(
            DoctorDomain::Workflow,
            None,
            DoctorSeverity::Error,
            "workflow.expression",
            "invalid expression",
            None,
            None,
        )];

        assert_eq!(workflow_score(&findings), 69);
    }

    #[test]
    fn deps_doctor_flags_node_lockfile_and_direct_sources() {
        let root = temp_repo("ossify-deps-doctor-node");
        fs::write(
            root.join("package.json"),
            r#"{
  "name": "demo-node",
  "version": "0.1.0",
  "dependencies": {
    "left-pad": "*",
    "direct-url": "https://example.com/direct.tgz"
  }
}"#,
        )
        .expect("write package.json");

        let report = doctor_deps(&root, DoctorEcosystem::Node).expect("doctor deps");

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.code == "deps.node.lockfile.missing"));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.code == "deps.node.version.wildcard"));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.code == "deps.node.source.direct-source"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn release_doctor_flags_rust_release_gaps() {
        let root = temp_repo("ossify-release-doctor-rust");
        fs::create_dir_all(root.join("src")).expect("create src");
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nlicense = \"MIT\"\n",
        )
        .expect("write Cargo.toml");
        fs::write(root.join("src/main.rs"), "fn main() {}\n").expect("write main.rs");

        let report = doctor_release(&root, DoctorEcosystem::Rust).expect("doctor release");

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.code == "release.rust.workflow.missing"));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.code == "release.rust.changelog.missing"));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.code == "release.rust.tag-signal.missing"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn deps_summary_mentions_degraded_mode_when_any_engine_falls_back() {
        let summary = deps_summary(
            &[],
            &[EcosystemDoctorScore {
                ecosystem: DoctorEcosystem::Node,
                score: 100,
                cap: None,
                cap_reason: None,
                cap_code: None,
                engine: String::from("audit-ci + ossify policy"),
                engine_source: EngineSource::AbsorbedPolicy,
                engine_status: ManagedEngineStatus::RuntimeMissing,
                engine_detail: Some(String::from("node missing")),
                finding_count: 0,
            }],
        );

        assert!(summary.contains("degraded fallback mode"));
    }

    #[test]
    fn python_audit_args_prefer_requirements_files() {
        let root = temp_repo("ossify-python-audit-args");
        fs::write(root.join("requirements.txt"), "urllib3==1.26.18\n").expect("write requirements");

        let args = python_audit_args(&root);

        assert!(args.iter().any(|arg| arg == "--requirement"));
        assert!(args.iter().any(|arg| arg == "requirements.txt"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn aggregate_engine_source_prefers_managed_when_any_ecosystem_is_managed() {
        let scores = vec![
            EcosystemDoctorScore {
                ecosystem: DoctorEcosystem::Rust,
                score: 49,
                cap: Some(49),
                cap_reason: Some(String::from("high Rust advisory capped score at 49/100")),
                cap_code: Some(String::from("deps.rust.advisory.reported")),
                engine: String::from("cargo-deny + ossify policy"),
                engine_source: EngineSource::ManagedTool,
                engine_status: ManagedEngineStatus::Managed,
                engine_detail: None,
                finding_count: 1,
            },
            EcosystemDoctorScore {
                ecosystem: DoctorEcosystem::Python,
                score: 77,
                cap: None,
                cap_reason: None,
                cap_code: None,
                engine: String::from("pip-audit + ossify policy"),
                engine_source: EngineSource::AbsorbedPolicy,
                engine_status: ManagedEngineStatus::ParseFailed,
                engine_detail: Some(String::from("parse failed")),
                finding_count: 3,
            },
        ];

        assert_eq!(aggregate_engine_source(&scores), EngineSource::ManagedTool);
    }

    #[test]
    fn cargo_deny_advisories_normalize_to_specific_rust_classes() {
        let root = temp_repo("ossify-cargo-deny-classes");
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n",
        )
        .expect("write Cargo.toml");

        let cases = [
            (
                RustAdvisoryClass::Critical,
                advisory_output("vulnerability", None, Some(9.8), "critical issue"),
            ),
            (
                RustAdvisoryClass::High,
                advisory_output("vulnerability", None, Some(8.2), "high issue"),
            ),
            (
                RustAdvisoryClass::Medium,
                advisory_output("vulnerability", None, Some(5.4), "medium issue"),
            ),
            (
                RustAdvisoryClass::Low,
                advisory_output("vulnerability", None, Some(2.3), "low issue"),
            ),
            (
                RustAdvisoryClass::Unmaintained,
                advisory_output(
                    "unmaintained",
                    Some("unmaintained"),
                    None,
                    "crate is unmaintained",
                ),
            ),
            (
                RustAdvisoryClass::Yanked,
                advisory_output("yanked", Some("yanked"), None, "crate was yanked"),
            ),
            (
                RustAdvisoryClass::Unsound,
                advisory_output("unsound", Some("unsound"), None, "crate is unsound"),
            ),
            (
                RustAdvisoryClass::Informational,
                advisory_output("notice", Some("notice"), None, "informational advisory"),
            ),
            (
                RustAdvisoryClass::Reported,
                advisory_output("vulnerability", None, None, "generic advisory"),
            ),
        ];

        for (class, output) in cases {
            let findings =
                parse_cargo_deny_output(&root, &output).expect("parse cargo-deny output");
            assert!(
                findings.iter().any(|finding| finding.code == class.code()),
                "expected {:?} to normalize to {} but saw {:?}",
                class,
                class.code(),
                findings
                    .iter()
                    .map(|finding| finding.code.clone())
                    .collect::<Vec<_>>()
            );
        }

        let _ = fs::remove_dir_all(&root);
    }

    fn advisory_output(
        code: &str,
        informational: Option<&str>,
        cvss: Option<f64>,
        message: &str,
    ) -> std::process::Output {
        let advisory = serde_json::json!({
            "id": "RUSTSEC-2099-0001",
            "package": "demo-crate",
            "informational": informational,
            "cvss": cvss,
        });
        let line = serde_json::json!({
            "type": "diagnostic",
            "fields": {
                "code": code,
                "level": "error",
                "severity": "error",
                "message": message,
                "advisory": advisory,
                "graphs": [
                    {
                        "Krate": {
                            "name": "demo-crate",
                            "version": "1.2.3"
                        }
                    }
                ]
            }
        });

        std::process::Output {
            status: std::process::ExitStatus::from_raw(1),
            stdout: format!("{line}\n").into_bytes(),
            stderr: Vec::new(),
        }
    }
}
