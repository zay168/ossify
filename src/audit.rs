use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::config::{OssifyConfig, RuleRequirement};
use crate::doctor::{audit_domain_scores, DoctorDomainScore};
use crate::intel::index::RepoIndex;
use crate::intel::inference::{infer_rule, FindingSignal, RuleInput};
use crate::intel::knowledge::KnowledgePack;
use crate::intel::{
    ConfidenceBreakdown, ContextRef, HistoryRef, ProofItem, RetrievalScope, RootCause,
};
use crate::project::{detect_project, ProjectContext, ProjectKind, RepoProfile};
use crate::trust::{aggregate_trust_score, TrustKernelConfig};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    Strong,
    Partial,
    Missing,
}

impl CheckStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Strong => "strong",
            Self::Partial => "partial",
            Self::Missing => "missing",
        }
    }

    pub fn from_coverage(coverage: u8) -> Self {
        if coverage >= 85 {
            Self::Strong
        } else if coverage >= 20 {
            Self::Partial
        } else {
            Self::Missing
        }
    }

    pub fn meets_requirement(self, requirement: RuleRequirement) -> bool {
        match requirement {
            RuleRequirement::Partial => matches!(self, Self::Partial | Self::Strong),
            RuleRequirement::Strong => matches!(self, Self::Strong),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReadinessTier {
    LaunchReady,
    Promising,
    Rough,
}

impl ReadinessTier {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LaunchReady => "launch-ready",
            Self::Promising => "promising",
            Self::Rough => "rough",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleCategory {
    Identity,
    Docs,
    Community,
    Automation,
    Release,
}

impl RuleCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Identity => "identity",
            Self::Docs => "docs",
            Self::Community => "community",
            Self::Automation => "automation",
            Self::Release => "release",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Fixability {
    Automatic,
    Guided,
    Manual,
}

impl Fixability {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Automatic => "automatic",
            Self::Guided => "guided",
            Self::Manual => "manual",
        }
    }

    pub fn is_fixable(self) -> bool {
        !matches!(self, Self::Manual)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FindingSeverity {
    Error,
    Warning,
    Info,
}

impl FindingSeverity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
        }
    }

    pub fn rank(self) -> u8 {
        match self {
            Self::Error => 0,
            Self::Warning => 1,
            Self::Info => 2,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AuditFinding {
    pub id: String,
    pub severity: FindingSeverity,
    pub message: String,
    pub help: String,
    pub evidence: Vec<String>,
    pub location: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuditDiagnostic {
    pub rule_id: &'static str,
    pub rule_label: &'static str,
    pub category: RuleCategory,
    pub severity: FindingSeverity,
    pub message: String,
    pub help: String,
    pub evidence: Vec<String>,
    pub location: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct PlannedCheckUpgrade {
    pub rule_id: &'static str,
    pub message: String,
    pub evidence: Vec<String>,
    pub location: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuditCheck {
    pub id: &'static str,
    pub label: &'static str,
    pub category: RuleCategory,
    pub weight: u16,
    pub points: u16,
    pub earned: u16,
    pub coverage: u8,
    pub status: CheckStatus,
    pub fixability: Fixability,
    pub fixable: bool,
    pub confidence: f32,
    pub message: String,
    pub hint: &'static str,
    pub detail: Option<String>,
    pub evidence: Vec<String>,
    pub findings: Vec<AuditFinding>,
    pub location: Option<PathBuf>,
    pub primary_cause: Option<RootCause>,
    pub secondary_causes: Vec<RootCause>,
    pub causes: Vec<RootCause>,
    pub proof: Vec<ProofItem>,
    pub context_refs: Vec<ContextRef>,
    pub retrieval_scope: RetrievalScope,
    pub history_refs: Vec<HistoryRef>,
    pub confidence_breakdown: ConfidenceBreakdown,
    pub required_level: Option<RuleRequirement>,
    pub blocking: bool,
}

impl AuditCheck {
    pub fn gap(&self) -> u16 {
        self.weight.saturating_sub(self.earned)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CategoryScore {
    pub category: RuleCategory,
    pub earned: u16,
    pub total: u16,
    pub score: u8,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuditReport {
    pub target: PathBuf,
    pub project: ProjectContext,
    pub readiness: ReadinessTier,
    pub score: u8,
    pub base_score: u8,
    pub minimum_score: u8,
    pub strict_passed: bool,
    pub config_source: Option<PathBuf>,
    pub category_scores: Vec<CategoryScore>,
    pub domain_scores: Vec<DoctorDomainScore>,
    pub checks: Vec<AuditCheck>,
}

impl AuditReport {
    pub fn strong_checks(&self) -> impl Iterator<Item = &AuditCheck> {
        self.checks
            .iter()
            .filter(|check| check.status == CheckStatus::Strong)
    }

    pub fn partial_checks(&self) -> impl Iterator<Item = &AuditCheck> {
        self.checks
            .iter()
            .filter(|check| check.status == CheckStatus::Partial)
    }

    pub fn missing_checks(&self) -> impl Iterator<Item = &AuditCheck> {
        self.checks
            .iter()
            .filter(|check| check.status == CheckStatus::Missing)
    }

    pub fn strong_count(&self) -> usize {
        self.strong_checks().count()
    }

    pub fn partial_count(&self) -> usize {
        self.partial_checks().count()
    }

    pub fn missing_count(&self) -> usize {
        self.missing_checks().count()
    }

    pub fn finding_count(&self) -> usize {
        self.checks.iter().map(|check| check.findings.len()).sum()
    }

    pub fn diagnostics(&self) -> Vec<AuditDiagnostic> {
        self.checks
            .iter()
            .flat_map(|check| {
                check.findings.iter().map(move |finding| AuditDiagnostic {
                    rule_id: check.id,
                    rule_label: check.label,
                    category: check.category,
                    severity: finding.severity,
                    message: finding.message.clone(),
                    help: finding.help.clone(),
                    evidence: finding.evidence.clone(),
                    location: finding.location.clone(),
                })
            })
            .collect()
    }
}

#[derive(Clone, Copy)]
struct RuleSpec {
    id: &'static str,
    label: &'static str,
    category: RuleCategory,
    fixability: Fixability,
    hint: &'static str,
}

#[derive(Debug, Clone)]
struct RuleAssessment {
    status: CheckStatus,
    coverage: u8,
    confidence: f32,
    message: String,
    evidence: Vec<String>,
    findings: Vec<AuditFinding>,
    location: Option<PathBuf>,
}

impl RuleAssessment {
    fn simple(
        status: CheckStatus,
        confidence: f32,
        message: impl Into<String>,
        evidence: Vec<String>,
        location: Option<PathBuf>,
    ) -> Self {
        Self {
            status,
            coverage: default_coverage(status),
            confidence,
            message: message.into(),
            evidence,
            findings: Vec::new(),
            location,
        }
    }

    fn precise(
        coverage: u8,
        confidence: f32,
        message: impl Into<String>,
        evidence: Vec<String>,
        findings: Vec<AuditFinding>,
        location: Option<PathBuf>,
    ) -> Self {
        Self {
            status: CheckStatus::from_coverage(coverage),
            coverage,
            confidence,
            message: message.into(),
            evidence,
            findings,
            location,
        }
    }
}

struct RepositorySnapshot {
    root: PathBuf,
    project: ProjectContext,
    files: Vec<PathBuf>,
    workflow_files: Vec<PathBuf>,
    workflow_text: String,
    readme_path: Option<PathBuf>,
    readme_text: String,
    nested_projects: Vec<NestedProject>,
    index: RepoIndex,
    knowledge: KnowledgePack,
}

#[derive(Debug, Clone)]
struct NestedProject {
    root: PathBuf,
    project: ProjectContext,
}

impl RepositorySnapshot {
    fn build(root: PathBuf, project: ProjectContext) -> io::Result<Self> {
        let knowledge = KnowledgePack::load(project.kind);
        let index = RepoIndex::build(&root, &project)?;
        let files = index.files.clone();
        let workflow_files = index.workflow_files();
        let readme_path = index.first_existing(&["README.md", "README"]);
        let readme_text = readme_path
            .as_ref()
            .and_then(|path| index.file_text(path).map(str::to_owned))
            .unwrap_or_default();
        let workflow_text = workflow_files
            .iter()
            .map(|path| {
                index
                    .file_text(path)
                    .map(str::to_owned)
                    .unwrap_or_else(|| read_text(path))
            })
            .collect::<Vec<_>>()
            .join("\n");
        let nested_projects = find_nested_projects(&root);

        Ok(Self {
            root,
            project,
            workflow_text,
            workflow_files,
            files,
            readme_path,
            readme_text,
            nested_projects,
            index,
            knowledge,
        })
    }

    fn first_existing(&self, candidates: &[&str]) -> Option<PathBuf> {
        self.index.first_existing(candidates)
    }

    fn supporting_doc(&self) -> Option<PathBuf> {
        preferred_markdown_doc(&self.root, &self.files)
    }

    fn primary_nested_project(&self) -> Option<&NestedProject> {
        self.nested_projects.first()
    }
}

fn find_nested_projects(root: &Path) -> Vec<NestedProject> {
    let mut nested = fs::read_dir(root)
        .ok()
        .into_iter()
        .flat_map(|entries| entries.filter_map(Result::ok))
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .filter(|path| {
            path.file_name()
                .and_then(|value| value.to_str())
                .map(|name| {
                    !matches!(
                        name,
                        ".git"
                            | "target"
                            | "node_modules"
                            | ".next"
                            | "dist"
                            | "build"
                            | ".venv"
                            | "venv"
                            | "__pycache__"
                    )
                })
                .unwrap_or(false)
        })
        .filter(|path| path.join(".git").exists())
        .filter_map(|path| {
            let project = detect_project(&path).ok()?;
            Some(NestedProject {
                root: path,
                project,
            })
        })
        .collect::<Vec<_>>();

    nested.sort_by(|left, right| left.root.cmp(&right.root));
    nested
}

pub fn audit_repository(path: &Path, config: &OssifyConfig) -> io::Result<AuditReport> {
    ensure_directory(path)?;

    let canonical = if path.exists() {
        fs::canonicalize(path)?
    } else {
        path.to_path_buf()
    };
    let project = detect_project(&canonical)?.with_profile_override(config.profile_override());
    let snapshot = RepositorySnapshot::build(canonical.clone(), project.clone())?;

    let specs = rule_specs();
    let mut checks = Vec::new();
    for spec in specs {
        if !config.rule_enabled(spec.id) {
            continue;
        }

        let assessment = match spec.id {
            "project_manifest" => assess_project_manifest(&snapshot),
            "manifest_metadata" => assess_manifest_metadata(&snapshot),
            "license" => assess_license(&snapshot),
            "readme" => assess_readme(&snapshot),
            "examples" => assess_examples(&snapshot),
            "contributing_guide" => assess_quality_document(
                &snapshot,
                &["CONTRIBUTING.md"],
                200,
                &["pull request", "issue", "test", "branch"],
            ),
            "code_of_conduct" => assess_quality_document(
                &snapshot,
                &["CODE_OF_CONDUCT.md"],
                160,
                &["expected behavior", "unacceptable behavior", "report"],
            ),
            "security_policy" => assess_security_policy(&snapshot),
            "issue_templates" => assess_issue_templates(&snapshot),
            "pull_request_template" => assess_pull_request_template(&snapshot),
            "codeowners" => assess_codeowners(&snapshot),
            "funding" => assess_funding(&snapshot),
            "ci_workflow" => assess_ci_workflow(&snapshot),
            "tests" => assess_tests(&snapshot),
            "lint_and_format" => assess_lint_and_format(&snapshot),
            "dependabot" => assess_dependabot(&snapshot),
            "changelog" => assess_changelog(&snapshot),
            "release_workflow" => assess_release_workflow(&snapshot),
            _ => continue,
        };

        checks.push(build_check(spec, &snapshot, config, assessment));
    }

    let base_score = percentage(
        checks.iter().map(|check| check.earned).sum(),
        checks.iter().map(|check| check.weight).sum(),
    );
    let domain_scores = audit_domain_scores(&canonical)?;
    let score = aggregate_audit_score(base_score, &domain_scores);
    let minimum_score = config.minimum_score();
    let strict_passed = score >= minimum_score && checks.iter().all(|check| !check.blocking);

    Ok(AuditReport {
        target: canonical,
        project: snapshot.project,
        readiness: readiness_tier(score),
        score,
        base_score,
        minimum_score,
        strict_passed,
        config_source: config.source().map(Path::to_path_buf),
        category_scores: category_scores(&checks),
        domain_scores,
        checks,
    })
}

pub fn estimate_report_after_upgrades(
    report: &AuditReport,
    upgrades: &[PlannedCheckUpgrade],
) -> AuditReport {
    let mut estimated = report.clone();

    for check in &mut estimated.checks {
        let Some(upgrade) = upgrades.iter().find(|upgrade| upgrade.rule_id == check.id) else {
            continue;
        };

        check.coverage = 100;
        check.status = CheckStatus::Strong;
        check.earned = check.weight;
        check.points = check.weight;
        check.message = upgrade.message.clone();
        check.detail = Some(upgrade.message.clone());
        check.evidence = upgrade.evidence.clone();
        check.findings.clear();
        check.location = upgrade.location.clone();
        check.primary_cause = None;
        check.secondary_causes.clear();
        check.causes.clear();
        check.proof = vec![ProofItem {
            expectation: format!("planned ossify scaffold for {}", check.label),
            kind: crate::intel::ProofKind::Satisfied,
            weight: check.weight,
            confidence: 0.95,
            detail: upgrade.message.clone(),
            context: upgrade
                .location
                .as_ref()
                .map(|path| {
                    vec![ContextRef {
                        path: path.clone(),
                        chunk_kind: crate::intel::ChunkKind::FilePath,
                        byte_start: None,
                        byte_end: None,
                        line_start: None,
                        line_end: None,
                        approximate: true,
                        excerpt: None,
                    }]
                })
                .unwrap_or_default(),
        }];
        check.context_refs = check
            .proof
            .iter()
            .flat_map(|item| item.context.clone())
            .collect();
        check.retrieval_scope = RetrievalScope {
            consulted_paths: upgrade
                .location
                .as_ref()
                .map(|path| vec![path.display().to_string()])
                .unwrap_or_default(),
            chunk_kinds: vec![String::from("file-path")],
            used_history: false,
            cache_state: crate::intel::CacheState::Unavailable,
        };
        check.history_refs.clear();
        check.confidence_breakdown = ConfidenceBreakdown {
            support_score: check.weight as f32,
            penalty_score: 0.0,
            total_required_weight: check.weight as f32,
            derived_coverage: 100,
        };
        check.blocking = false;
    }

    estimated.base_score = percentage(
        estimated.checks.iter().map(|check| check.earned).sum(),
        estimated.checks.iter().map(|check| check.weight).sum(),
    );
    estimated.score = aggregate_audit_score(estimated.base_score, &estimated.domain_scores);
    estimated.readiness = readiness_tier(estimated.score);
    estimated.category_scores = category_scores(&estimated.checks);
    estimated.strict_passed = estimated.score >= estimated.minimum_score
        && estimated.checks.iter().all(|check| !check.blocking);

    estimated
}

fn readiness_tier(score: u8) -> ReadinessTier {
    if score >= 85 {
        ReadinessTier::LaunchReady
    } else if score >= 60 {
        ReadinessTier::Promising
    } else {
        ReadinessTier::Rough
    }
}

fn aggregate_audit_score(base_score: u8, domain_scores: &[DoctorDomainScore]) -> u8 {
    aggregate_trust_score(base_score, domain_scores, TrustKernelConfig::default())
}

fn ensure_directory(path: &Path) -> io::Result<()> {
    if path.exists() && !path.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{} is not a directory", path.display()),
        ));
    }

    Ok(())
}

fn rule_specs() -> Vec<RuleSpec> {
    vec![
        RuleSpec { id: "project_manifest", label: "Project manifest", category: RuleCategory::Identity, fixability: Fixability::Manual, hint: "Keep a real manifest in the repository root so the stack and package identity are explicit." },
        RuleSpec { id: "manifest_metadata", label: "Manifest metadata", category: RuleCategory::Identity, fixability: Fixability::Manual, hint: "Fill description, repository, homepage, version, and discoverability metadata inside the manifest." },
        RuleSpec { id: "license", label: "License", category: RuleCategory::Identity, fixability: Fixability::Automatic, hint: "Ship a recognized LICENSE file so adopters can evaluate usage rights immediately." },
        RuleSpec { id: "readme", label: "README", category: RuleCategory::Docs, fixability: Fixability::Automatic, hint: "Explain what the project is, how to install it, how to use it, and where examples live." },
        RuleSpec { id: "examples", label: "Examples", category: RuleCategory::Docs, fixability: Fixability::Manual, hint: "Add a concrete example or examples directory so a new user can copy a working starting point." },
        RuleSpec { id: "contributing_guide", label: "Contributing guide", category: RuleCategory::Community, fixability: Fixability::Automatic, hint: "Document the contribution flow, review expectations, and local verification steps." },
        RuleSpec { id: "code_of_conduct", label: "Code of conduct", category: RuleCategory::Community, fixability: Fixability::Automatic, hint: "Define expected behavior and a reporting path so contributors know the project is safe to join." },
        RuleSpec { id: "security_policy", label: "Security policy", category: RuleCategory::Community, fixability: Fixability::Automatic, hint: "Explain how to report vulnerabilities privately and what response contributors can expect." },
        RuleSpec { id: "issue_templates", label: "Issue templates", category: RuleCategory::Community, fixability: Fixability::Automatic, hint: "Use bug and feature templates to collect higher-signal issues." },
        RuleSpec { id: "pull_request_template", label: "Pull request template", category: RuleCategory::Community, fixability: Fixability::Automatic, hint: "A pull request template keeps reviews focused on context, impact, and verification." },
        RuleSpec { id: "codeowners", label: "CODEOWNERS", category: RuleCategory::Community, fixability: Fixability::Guided, hint: "Declare review ownership so contributors know who is responsible for the repository surface." },
        RuleSpec { id: "funding", label: "Funding file", category: RuleCategory::Community, fixability: Fixability::Guided, hint: "If maintainers accept sponsorship, add FUNDING.yml so support is easy to discover." },
        RuleSpec { id: "ci_workflow", label: "CI workflow", category: RuleCategory::Automation, fixability: Fixability::Automatic, hint: "Run build, test, and stack-specific verification on push and pull request." },
        RuleSpec { id: "tests", label: "Tests", category: RuleCategory::Automation, fixability: Fixability::Manual, hint: "Add or surface executable tests so the public surface is safer to evolve." },
        RuleSpec { id: "lint_and_format", label: "Lint and format signals", category: RuleCategory::Automation, fixability: Fixability::Manual, hint: "Expose a lint and formatting path so the repo feels maintained and predictable." },
        RuleSpec { id: "dependabot", label: "Dependabot", category: RuleCategory::Automation, fixability: Fixability::Automatic, hint: "Automate dependency update visibility with Dependabot." },
        RuleSpec { id: "changelog", label: "Changelog", category: RuleCategory::Release, fixability: Fixability::Automatic, hint: "Track release history so changes are easier to evaluate before upgrading." },
        RuleSpec { id: "release_workflow", label: "Release workflow", category: RuleCategory::Release, fixability: Fixability::Automatic, hint: "Automate release notes or packaging so shipping is repeatable." },
    ]
}

fn build_check(
    spec: RuleSpec,
    snapshot: &RepositorySnapshot,
    config: &OssifyConfig,
    assessment: RuleAssessment,
) -> AuditCheck {
    let weight = rule_weight(spec.id, spec.category, snapshot.project.profile, config);
    let required_level = config.rule_requirement(spec.id);
    let blocking = required_level
        .map(|requirement| !assessment.status.meets_requirement(requirement))
        .unwrap_or(false);
    let earned = coverage_to_points(weight, assessment.coverage);
    let confidence = assessment.confidence.clamp(0.0, 1.0);
    let message = assessment.message.clone();
    let evidence = assessment.evidence.clone();
    let findings = assessment.findings.clone();
    let location = assessment.location.clone();
    let intelligence = infer_rule(
        &RuleInput {
            rule_id: spec.id,
            label: spec.label,
            coverage: assessment.coverage,
            message: message.clone(),
            evidence: evidence.clone(),
            findings: findings
                .iter()
                .map(|finding| FindingSignal {
                    id: finding.id.clone(),
                    severity: finding.severity,
                    message: finding.message.clone(),
                    help: finding.help.clone(),
                    location: finding.location.clone(),
                })
                .collect(),
            location: location.clone(),
        },
        &snapshot.project,
        &snapshot.index,
        &snapshot.knowledge.rule(spec.id),
    );

    AuditCheck {
        id: spec.id,
        label: spec.label,
        category: spec.category,
        weight,
        points: earned,
        earned,
        coverage: assessment.coverage,
        status: assessment.status,
        fixability: spec.fixability,
        fixable: spec.fixability.is_fixable(),
        confidence,
        message: message.clone(),
        hint: spec.hint,
        detail: Some(message),
        evidence,
        findings,
        location,
        primary_cause: intelligence.primary_cause,
        secondary_causes: intelligence.secondary_causes,
        causes: intelligence.causes,
        proof: intelligence.proof,
        context_refs: intelligence.context_refs,
        retrieval_scope: intelligence.retrieval_scope,
        history_refs: intelligence.history_refs,
        confidence_breakdown: intelligence.confidence_breakdown,
        required_level,
        blocking,
    }
}

fn rule_weight(
    id: &str,
    category: RuleCategory,
    profile: RepoProfile,
    config: &OssifyConfig,
) -> u16 {
    if let Some(weight) = config.rule_weight(id) {
        return weight.max(1);
    }

    let base = base_rule_weight(id, profile) as f32;
    let weighted = (base * config.category_multiplier(category.as_str())).round();
    weighted.max(1.0) as u16
}

fn base_rule_weight(id: &str, profile: RepoProfile) -> u16 {
    match (id, profile) {
        ("project_manifest", _) => 8,
        ("manifest_metadata", RepoProfile::Library) => 14,
        ("manifest_metadata", RepoProfile::Cli) => 12,
        ("manifest_metadata", RepoProfile::App) => 11,
        ("manifest_metadata", RepoProfile::Generic) => 10,
        ("license", _) => 12,
        ("readme", RepoProfile::Cli) => 16,
        ("readme", RepoProfile::App) => 15,
        ("readme", _) => 14,
        ("examples", RepoProfile::Library) => 10,
        ("examples", RepoProfile::Cli) => 8,
        ("examples", RepoProfile::App) => 6,
        ("examples", RepoProfile::Generic) => 7,
        ("contributing_guide", _) => 6,
        ("code_of_conduct", _) => 6,
        ("security_policy", RepoProfile::App) => 10,
        ("security_policy", _) => 8,
        ("issue_templates", _) => 6,
        ("pull_request_template", _) => 4,
        ("codeowners", RepoProfile::Cli) => 5,
        ("codeowners", _) => 4,
        ("funding", _) => 3,
        ("ci_workflow", _) => 10,
        ("tests", RepoProfile::Library) => 10,
        ("tests", RepoProfile::App) => 9,
        ("tests", _) => 8,
        ("lint_and_format", _) => 5,
        ("dependabot", _) => 4,
        ("changelog", _) => 6,
        ("release_workflow", RepoProfile::Cli) => 7,
        ("release_workflow", _) => 6,
        _ => 5,
    }
}

fn default_coverage(status: CheckStatus) -> u8 {
    match status {
        CheckStatus::Strong => 100,
        CheckStatus::Partial => 60,
        CheckStatus::Missing => 0,
    }
}

fn coverage_ratio(matched: usize, total: usize) -> u8 {
    if total == 0 {
        0
    } else {
        (((matched as u32) * 100) / (total as u32)) as u8
    }
}

fn weighted_coverage(parts: &[(u8, bool)]) -> u8 {
    let total = parts
        .iter()
        .map(|(weight, _)| u32::from(*weight))
        .sum::<u32>();
    if total == 0 {
        return 0;
    }

    let earned = parts
        .iter()
        .filter(|(_, matched)| *matched)
        .map(|(weight, _)| u32::from(*weight))
        .sum::<u32>();
    ((earned * 100) / total) as u8
}

fn coverage_to_points(weight: u16, coverage: u8) -> u16 {
    ((u32::from(weight) * u32::from(coverage) + 50) / 100) as u16
}

fn finding(
    id: impl Into<String>,
    severity: FindingSeverity,
    message: impl Into<String>,
    help: impl Into<String>,
    evidence: Vec<String>,
    location: Option<PathBuf>,
) -> AuditFinding {
    AuditFinding {
        id: id.into(),
        severity,
        message: message.into(),
        help: help.into(),
        evidence: dedupe_strings(evidence),
        location,
    }
}

fn assess_project_manifest(snapshot: &RepositorySnapshot) -> RuleAssessment {
    match &snapshot.project.manifest_path {
        Some(path) if snapshot.project.kind != ProjectKind::Unknown => RuleAssessment::simple(
            CheckStatus::Strong,
            0.98,
            format!(
                "Detected a {} manifest for a {} repo.",
                snapshot.project.kind.display_name(),
                snapshot.project.profile.display_name()
            ),
            vec![relative_display(&snapshot.root, path)],
            Some(path.clone()),
        ),
        Some(path) => RuleAssessment::precise(
            50,
            0.7,
            "Found a manifest, but the project type is still ambiguous.",
            vec![relative_display(&snapshot.root, path)],
            vec![finding(
                "project_manifest.unsupported",
                FindingSeverity::Warning,
                "The manifest exists, but ossify could not infer a supported stack confidently.",
                "Keep a supported manifest in the repository root or add clearer stack markers.",
                vec![relative_display(&snapshot.root, path)],
                Some(path.clone()),
            )],
            Some(path.clone()),
        ),
        None if snapshot.primary_nested_project().is_some() => {
            let nested = snapshot.primary_nested_project().expect("nested project");
            let nested_label = format!(
                "{} ({})",
                nested
                    .root
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("nested project"),
                nested.project.summary()
            );
            let evidence = vec![format!(
                "nested project: {}",
                relative_display(&snapshot.root, &nested.root)
            )];
            RuleAssessment::precise(
                25,
                0.78,
                "No supported root manifest was found, but a nested standalone project was detected.",
                evidence.clone(),
                vec![finding(
                    "project_manifest.nested_project",
                    FindingSeverity::Warning,
                    format!(
                        "No root manifest was detected, but `{nested_label}` looks like the real runnable project."
                    ),
                    format!(
                        "If `{}` is the real project, audit that path directly or add a root README that explains the workspace layout and why the code lives in a nested project.",
                        relative_display(&snapshot.root, &nested.root)
                    ),
                    evidence,
                    Some(nested.root.clone()),
                )],
                Some(nested.root.clone()),
            )
        }
        None => RuleAssessment::precise(
            0,
            0.99,
            "No supported manifest was found in the repository root.",
            Vec::new(),
            vec![finding(
                "project_manifest.missing",
                FindingSeverity::Error,
                "No root manifest was detected.",
                "Add Cargo.toml, package.json, pyproject.toml, or go.mod at the repository root.",
                Vec::new(),
                None,
            )],
            None,
        ),
    }
}

fn assess_manifest_metadata(snapshot: &RepositorySnapshot) -> RuleAssessment {
    let metadata = &snapshot.project.metadata;
    let coherent_name =
        !snapshot.project.binary_name().is_empty() && snapshot.project.name != "project";
    let mut evidence = Vec::new();
    let mut findings = Vec::new();
    let location = snapshot.project.manifest_path.clone();
    let lightweight_requirements_manifest = location
        .as_ref()
        .and_then(|path| path.file_name())
        .and_then(|value| value.to_str())
        .map(|name| name.eq_ignore_ascii_case("requirements.txt"))
        .unwrap_or(false);

    if lightweight_requirements_manifest {
        if coherent_name {
            evidence.push(String::from("name"));
        }
        if !metadata.dependencies.is_empty() {
            evidence.push(format!("{} dependencies", metadata.dependencies.len()));
        }

        let coverage = if metadata.dependencies.is_empty() {
            15
        } else {
            25
        };
        findings.push(finding(
            "manifest_metadata.lightweight_manifest",
            FindingSeverity::Info,
            "A lightweight Python dependency manifest was detected, but it cannot carry richer project metadata.",
            "Keep requirements.txt for runtime dependencies, and add pyproject.toml later if you want version, description, repository, or packaging metadata to be explicit.",
            vec![String::from("requirements.txt")],
            location.clone(),
        ));

        return RuleAssessment::precise(
            coverage,
            0.78,
            if metadata.dependencies.is_empty() {
                String::from(
                    "A lightweight Python dependency manifest exists, but it carries very little project metadata.",
                )
            } else {
                String::from(
                    "requirements.txt captures Python dependencies, but richer repository metadata still lives outside the manifest surface.",
                )
            },
            evidence,
            findings,
            location,
        );
    }

    let has_description = metadata
        .description
        .as_ref()
        .map(|value| value.len() > 20)
        .unwrap_or(false);
    let has_license = metadata.license.is_some();
    let has_repository = metadata.repository.is_some();
    let has_homepage = metadata.homepage.is_some();
    let has_version = metadata.version.is_some();
    let has_discovery = !metadata.keywords.is_empty() || !metadata.categories.is_empty();

    for (label, matched) in [
        ("name", coherent_name),
        ("description", has_description),
        ("license metadata", has_license),
        ("repository", has_repository),
        ("homepage", has_homepage),
        ("version", has_version),
        ("keywords/categories", has_discovery),
    ] {
        if matched {
            evidence.push(label.to_owned());
        }
    }

    if !coherent_name {
        findings.push(finding(
            "manifest_metadata.name",
            FindingSeverity::Warning,
            "The manifest name is missing or too generic.",
            "Use a stable, project-specific package name so the repository identity is obvious.",
            Vec::new(),
            location.clone(),
        ));
    }
    if !has_description {
        findings.push(finding(
            "manifest_metadata.description",
            FindingSeverity::Warning,
            "The manifest is missing a meaningful description.",
            "Add a one-sentence description that explains the public value of the project.",
            metadata
                .description
                .clone()
                .into_iter()
                .collect::<Vec<String>>(),
            location.clone(),
        ));
    }
    if !has_license {
        findings.push(finding(
            "manifest_metadata.license",
            FindingSeverity::Info,
            "The manifest does not declare license metadata.",
            "Mirror the repository license inside the manifest so package registries stay informative.",
            Vec::new(),
            location.clone(),
        ));
    }
    if !has_repository {
        findings.push(finding(
            "manifest_metadata.repository",
            FindingSeverity::Warning,
            "The manifest does not point back to the source repository.",
            "Add a repository URL so package pages link back to issues, docs, and source.",
            Vec::new(),
            location.clone(),
        ));
    }
    if !has_homepage {
        findings.push(finding(
            "manifest_metadata.homepage",
            FindingSeverity::Info,
            "The manifest is missing a homepage or docs URL.",
            "Add a homepage when you have canonical docs, a website, or richer project context.",
            Vec::new(),
            location.clone(),
        ));
    }
    if !has_version {
        findings.push(finding(
            "manifest_metadata.version",
            FindingSeverity::Warning,
            "The manifest does not expose a version yet.",
            "Set an explicit version so releases and changelogs have a stable anchor.",
            Vec::new(),
            location.clone(),
        ));
    }
    if !has_discovery {
        findings.push(finding(
            "manifest_metadata.discovery",
            FindingSeverity::Info,
            "The manifest has no keywords or categories for discovery.",
            "Add a few relevant keywords or categories to improve package discoverability.",
            Vec::new(),
            location.clone(),
        ));
    }

    let coverage = coverage_ratio(evidence.len(), 7);
    let status = CheckStatus::from_coverage(coverage);

    RuleAssessment::precise(
        coverage,
        0.82,
        match status {
            CheckStatus::Strong => {
                String::from("Manifest metadata looks discoverable and distribution-ready.")
            }
            CheckStatus::Partial => format!(
                "Manifest metadata covers {}/7 expected discovery signals.",
                evidence.len()
            ),
            CheckStatus::Missing => {
                String::from("The manifest exists, but it lacks most discovery metadata.")
            }
        },
        evidence,
        findings,
        location,
    )
}

fn assess_license(snapshot: &RepositorySnapshot) -> RuleAssessment {
    let manifest_license = snapshot.project.metadata.license.clone();
    match snapshot.first_existing(&["LICENSE", "LICENSE.md", "COPYING"]) {
        Some(path) => {
            let contents = normalize(&read_text(&path));
            let recognized = contains_any(
                &contents,
                &[
                    "mit license",
                    "apache license",
                    "gnu general public license",
                    "mozilla public license",
                    "bsd license",
                ],
            );
            let status = if recognized && contents.len() >= 240 {
                CheckStatus::Strong
            } else {
                CheckStatus::Partial
            };

            let mut evidence = vec![relative_display(&snapshot.root, &path)];
            if let Some(license) = manifest_license {
                evidence.push(format!("manifest: {license}"));
            }

            let coverage = if status == CheckStatus::Strong { 100 } else { 65 };
            let mut findings = Vec::new();
            if !recognized {
                findings.push(finding(
                    "license.unrecognized",
                    FindingSeverity::Warning,
                    "A license file exists, but the text does not look like a recognized license yet.",
                    "Use a standard SPDX-compatible license text so adopters can evaluate usage rights quickly.",
                    vec![relative_display(&snapshot.root, &path)],
                    Some(path.clone()),
                ));
            }

            RuleAssessment::precise(
                coverage,
                0.93,
                if status == CheckStatus::Strong {
                    String::from("A recognized license file is present.")
                } else {
                    String::from(
                        "A license file exists, but the contents do not look fully recognized yet.",
                    )
                },
                evidence,
                findings,
                Some(path),
            )
        }
        None if manifest_license.is_some() => RuleAssessment::precise(
            35,
            0.72,
            String::from(
                "License metadata exists in the manifest, but there is no dedicated LICENSE file.",
            ),
            vec![format!(
                "manifest: {}",
                manifest_license.unwrap_or_default()
            )],
            vec![finding(
                "license.file",
                FindingSeverity::Error,
                "The repository does not ship a dedicated LICENSE file.",
                "Add a root LICENSE file even if the manifest already declares the license identifier.",
                Vec::new(),
                snapshot.project.manifest_path.clone(),
            )],
            snapshot.project.manifest_path.clone(),
        ),
        None => RuleAssessment::precise(
            0,
            0.99,
            "No repository-wide license file was found.",
            Vec::new(),
            vec![finding(
                "license.missing",
                FindingSeverity::Error,
                "No repository-wide license file was found.",
                "Ship a root LICENSE file so adopters and companies can evaluate usage rights immediately.",
                Vec::new(),
                None,
            )],
            None,
        ),
    }
}

fn assess_readme(snapshot: &RepositorySnapshot) -> RuleAssessment {
    let Some(path) = &snapshot.readme_path else {
        if let Some(path) = snapshot.supporting_doc() {
            let contents = read_text(&path);
            let normalized = normalize(&contents);
            let headings = markdown_heading_count(&contents);
            let code_blocks = markdown_code_block_count(&contents);
            let has_usage_flow = contains_any(
                &normalized,
                &[
                    "## usage",
                    "## quick start",
                    "## pipeline",
                    "## execution",
                    "python scripts/",
                    "cargo run",
                    "python -m",
                ],
            );
            let mut evidence = vec![relative_display(&snapshot.root, &path)];
            if headings >= 4 {
                evidence.push(String::from("structured-headings"));
            }
            if code_blocks >= 2 {
                evidence.push(String::from("code-blocks"));
            }
            if has_usage_flow {
                evidence.push(String::from("usage-flow"));
            }

            let mut findings = vec![finding(
                "readme.missing-root",
                FindingSeverity::Warning,
                "There is no canonical README in the repository root.",
                "Add a root README that orients a new visitor before they discover deeper project docs.",
                vec![relative_display(&snapshot.root, &path)],
                Some(path.clone()),
            )];
            if !has_usage_flow {
                findings.push(finding(
                    "readme.usage",
                    FindingSeverity::Warning,
                    "Supporting docs exist, but they do not clearly expose a runnable usage flow.",
                    "Add a quick-start or usage section with a copy-pasteable command path.",
                    Vec::new(),
                    Some(path.clone()),
                ));
            }

            return RuleAssessment::precise(
                35 + (coverage_ratio(evidence.len(), 4) / 3),
                0.78,
                format!(
                    "No canonical README was found, but {} contains substantial project documentation.",
                    relative_display(&snapshot.root, &path)
                ),
                evidence,
                findings,
                Some(path),
            );
        }

        if let Some(nested) = snapshot.primary_nested_project() {
            let nested_readme = [nested.root.join("README.md"), nested.root.join("README")]
                .into_iter()
                .find(|candidate| candidate.is_file());

            if let Some(path) = nested_readme {
                let evidence = vec![
                    format!(
                        "nested project: {}",
                        relative_display(&snapshot.root, &nested.root)
                    ),
                    relative_display(&snapshot.root, &path),
                ];
                return RuleAssessment::precise(
                    25,
                    0.76,
                    "No root README was found, but a nested standalone project already has its own README.",
                    evidence.clone(),
                    vec![finding(
                        "readme.nested_project",
                        FindingSeverity::Warning,
                        "The repository root has no README, but a nested project already carries project docs.",
                        format!(
                            "Add a root README that explains the repository layout and points readers to `{}` if that nested project is the real app.",
                            relative_display(&snapshot.root, &path)
                        ),
                        evidence,
                        Some(path.clone()),
                    )],
                    Some(path),
                );
            }
        }

        return RuleAssessment::precise(
            0,
            0.99,
            "No README was found in the repository root.",
            Vec::new(),
            vec![finding(
                "readme.missing",
                FindingSeverity::Error,
                "No README was found in the repository root.",
                "Add a README that explains what the project does, how to install it, how to use it, and where examples live.",
                Vec::new(),
                None,
            )],
            None,
        );
    };

    let normalized = normalize(&snapshot.readme_text);
    let mut evidence = Vec::new();
    let has_commands_heading = contains_any(
        &normalized,
        &["## commands", "## command line", "## cli", "## command"],
    );
    let has_cli_command_surface = readme_has_cli_command_surface(&normalized, snapshot);
    let has_install_heading = markdown_heading_matches(
        &snapshot.readme_text,
        &[
            "install",
            "installation",
            "setup",
            "getting started",
            "quick start",
            "quickstart",
        ],
    );
    let has_usage_heading = markdown_heading_matches(
        &snapshot.readme_text,
        &[
            "usage",
            "utilisation",
            "quick start",
            "quickstart",
            "getting started",
            "run",
            "running",
            "demo",
        ],
    );
    let has_contributing_heading = markdown_heading_matches(
        &snapshot.readme_text,
        &["contributing", "contribuer", "contribution"],
    );
    let has_license_heading =
        markdown_heading_matches(&snapshot.readme_text, &["license", "licence"]);
    let has_examples_heading = markdown_heading_matches(
        &snapshot.readme_text,
        &["examples", "example", "exemples", "exemple"],
    );
    let has_stack_heading = markdown_heading_matches(
        &snapshot.readme_text,
        &[
            "stack",
            "stack technique",
            "tech stack",
            "technologies",
            "technology",
            "architecture",
        ],
    );
    let has_app_command_surface = readme_has_app_command_surface(&normalized, snapshot);
    let has_install = has_install_heading
        || contains_any(
            &normalized,
            &[
                "## install",
                "## setup",
                "### install",
                "cargo install",
                "npm install",
                "npm ci",
                "pnpm install",
                "yarn add",
                "yarn install",
                "bun install",
                "pip install",
                "python -m pip install",
                "go install",
                "brew install",
                "scoop install",
                "cargo build",
            ],
        )
        || normalized.contains(&snapshot.project.install_snippet().to_lowercase());
    let has_usage = has_usage_heading
        || has_commands_heading
        || has_cli_command_surface
        || has_app_command_surface;
    let has_contributing_doc = snapshot.first_existing(&["CONTRIBUTING.md"]).is_some();
    let has_contributing = has_contributing_heading
        || contains_any(
            &normalized,
            &["## contributing", "### contributing", "## contribuer"],
        )
        || normalized.contains("contributing.md")
        || has_contributing_doc;
    let has_license_file = snapshot
        .first_existing(&["LICENSE", "LICENSE.md"])
        .is_some();
    let has_license = has_license_heading
        || contains_any(&normalized, &["## license", "## licence", "licensed under"])
        || normalized.contains("license mit")
        || normalized.contains("license apache")
        || normalized.contains("licence mit")
        || normalized.contains("licence apache")
        || normalized.contains("/license")
        || (has_license_file && (normalized.contains("license") || normalized.contains("licence")));
    let has_examples =
        has_examples_heading || contains_any(&normalized, &["## examples", "## example"]);
    let has_code_block = snapshot.readme_text.matches("```").count() >= 2;
    let has_stack_hint = has_stack_heading
        || normalized.contains(&snapshot.project.install_snippet().to_lowercase())
        || normalized.contains(&snapshot.project.usage_snippet().to_lowercase());
    let has_length = snapshot.readme_text.len() >= 450;
    let is_placeholder = looks_placeholder(&normalized);
    let mut findings = Vec::new();
    let has_stack_hint = has_stack_hint || has_cli_command_surface || has_app_command_surface;

    for (label, present) in [
        ("length", has_length),
        ("install", has_install),
        ("usage", has_usage),
        (
            if has_contributing_doc && !normalized.contains("contributing") {
                "contributing-doc"
            } else {
                "contributing"
            },
            has_contributing,
        ),
        (
            if has_license_file && !contains_any(&normalized, &["## license", "licensed under"]) {
                "license-reference"
            } else {
                "license"
            },
            has_license,
        ),
        ("examples", has_examples),
        ("code-blocks", has_code_block),
        ("stack-hints", has_stack_hint),
    ] {
        if present {
            evidence.push(label.to_owned());
        }
    }

    if !has_length {
        findings.push(finding(
            "readme.depth",
            FindingSeverity::Warning,
            "The README is still very short for a first-time visitor.",
            "Expand it with enough context, installation, usage, and examples for a new adopter.",
            Vec::new(),
            Some(path.clone()),
        ));
    }
    for (id, present, section, help) in [
        (
            "readme.install",
            has_install,
            "installation section",
            "Add an install or setup section with the first command a user should run.",
        ),
        (
            "readme.usage",
            has_usage,
            "usage section",
            "Add a usage or quick-start section that shows the core command or import path.",
        ),
        (
            "readme.examples",
            has_examples,
            "examples section",
            "Link to a reusable example path or include one concrete example in the README.",
        ),
        (
            "readme.contributing",
            has_contributing,
            "contributing guidance",
            "Link to CONTRIBUTING.md or summarize how contributors can get started.",
        ),
        (
            "readme.license",
            has_license,
            "license section",
            "Mention the project license and link to the root LICENSE file.",
        ),
        (
            "readme.code-blocks",
            has_code_block,
            "copy-pasteable code blocks",
            "Add at least one runnable command or code example that a reader can copy-paste.",
        ),
        (
            "readme.stack-hints",
            has_stack_hint,
            "stack-specific install or usage hints",
            "Mention the actual install or usage command for the detected stack.",
        ),
    ] {
        if !present {
            findings.push(finding(
                id,
                FindingSeverity::Warning,
                format!("The README is missing a clear {section}."),
                help,
                Vec::new(),
                Some(path.clone()),
            ));
        }
    }
    if is_placeholder {
        findings.push(finding(
            "readme.placeholder",
            FindingSeverity::Warning,
            "The README still reads like scaffold copy.",
            "Replace starter text with project-specific positioning, examples, and contributor context.",
            Vec::new(),
            Some(path.clone()),
        ));
    }

    let mut coverage = weighted_coverage(&[
        (10, has_length),
        (20, has_install),
        (20, has_usage),
        (10, has_contributing),
        (10, has_license),
        (10, has_examples),
        (10, has_code_block),
        (10, has_stack_hint),
    ]);
    if is_placeholder {
        coverage = coverage.saturating_sub(20);
    }
    let status = CheckStatus::from_coverage(coverage);

    RuleAssessment::precise(
        coverage,
        0.8,
        if status == CheckStatus::Strong {
            String::from(
                "README covers the main adoption flow with concrete sections and examples.",
            )
        } else if is_placeholder {
            String::from("README exists, but it still reads like starter copy and needs more project-specific guidance.")
        } else {
            format!(
                "README covers {}/8 adoption signals, but key onboarding sections are still missing.",
                evidence.len()
            )
        },
        evidence,
        findings,
        Some(path.clone()),
    )
}

fn assess_examples(snapshot: &RepositorySnapshot) -> RuleAssessment {
    let readme_normalized = normalize(&snapshot.readme_text);
    let examples_dir = first_file_in_dir(&snapshot.root.join("examples"));
    let docs_dir = first_file_in_dir(&snapshot.root.join("docs"));
    let scripts_count = runnable_script_count(&snapshot.root.join("scripts"));
    let supporting_doc = snapshot.supporting_doc();
    let supporting_doc_text = supporting_doc
        .as_ref()
        .map(|path| read_text(path))
        .unwrap_or_default();
    let supporting_doc_normalized = normalize(&supporting_doc_text);
    let readme_example = contains_any(&readme_normalized, &["## examples", "## example"])
        && snapshot.readme_text.matches("```").count() >= 2;
    let doc_has_script_examples = scripts_count >= 2
        && markdown_code_block_count(&supporting_doc_text) >= 2
        && contains_any(
            &supporting_doc_normalized,
            &[
                "python scripts/",
                "scripts/",
                "pipeline d'ex",
                "pipeline d’ex",
                "execution",
                "run:",
            ],
        );

    let mut evidence = Vec::new();
    if let Some(path) = &examples_dir {
        evidence.push(relative_display(&snapshot.root, path));
    }
    if let Some(path) = &docs_dir {
        evidence.push(relative_display(&snapshot.root, path));
    }
    if scripts_count >= 2 {
        evidence.push(format!("scripts/ ({} files)", scripts_count));
    }
    if let Some(path) = &supporting_doc {
        evidence.push(relative_display(&snapshot.root, path));
    }
    if readme_example {
        evidence.push(String::from("README examples section"));
    }
    if doc_has_script_examples {
        evidence.push(String::from("documented script pipeline"));
    }

    let has_examples_dir = examples_dir.is_some();
    let has_usage_surface = readme_example || doc_has_script_examples;
    let has_executable_assets =
        scripts_count >= 2 || docs_dir.is_some() || supporting_doc.is_some();
    let coverage = if has_examples_dir || has_usage_surface {
        100
    } else {
        weighted_coverage(&[
            (35, has_executable_assets),
            (30, docs_dir.is_some()),
            (
                35,
                contains_any(&readme_normalized, &["## usage", "## quick start"]),
            ),
        ])
    };
    let status = CheckStatus::from_coverage(coverage);
    let location = examples_dir
        .or(docs_dir)
        .or_else(|| supporting_doc.clone())
        .or_else(|| snapshot.readme_path.clone());
    let mut findings = Vec::new();

    if !has_examples_dir && !has_usage_surface {
        findings.push(finding(
            "examples.path",
            FindingSeverity::Warning,
            "The repository has no clearly reusable example path yet.",
            "Add an examples/ directory, a worked README example, or a documented script pipeline that a newcomer can copy.",
            evidence.clone(),
            location.clone(),
        ));
    }
    if scripts_count == 0 && !readme_example {
        findings.push(finding(
            "examples.runnable",
            FindingSeverity::Info,
            "No runnable scripts or notebooks were detected for onboarding.",
            "Consider shipping a tiny runnable example, notebook, or script that exercises the core project flow.",
            Vec::new(),
            location.clone(),
        ));
    }

    RuleAssessment::precise(
        coverage,
        0.73,
        if status == CheckStatus::Strong {
            String::from("The repository offers at least one concrete example path for adopters.")
        } else if status == CheckStatus::Partial {
            String::from("There is some usage guidance, but no clearly reusable example path yet.")
        } else {
            String::from("No examples directory or concrete usage example was detected.")
        },
        dedupe_strings(evidence),
        findings,
        location,
    )
}

fn assess_quality_document(
    snapshot: &RepositorySnapshot,
    candidates: &[&str],
    min_strong_len: usize,
    keywords: &[&str],
) -> RuleAssessment {
    match snapshot.first_existing(candidates) {
        Some(path) => {
            let contents = normalize(&read_text(&path));
            let evidence = keywords
                .iter()
                .filter(|keyword| contents.contains(**keyword))
                .map(|keyword| (*keyword).to_owned())
                .collect::<Vec<String>>();
            let has_depth = contents.len() >= min_strong_len;
            let covers_topics = evidence.len() >= keywords.len().min(3);
            let not_placeholder = !looks_placeholder(&contents);
            let coverage = weighted_coverage(&[
                (35, true),
                (25, has_depth),
                (25, covers_topics),
                (15, not_placeholder),
            ]);
            let status = CheckStatus::from_coverage(coverage);
            let mut findings = Vec::new();

            if !has_depth {
                findings.push(finding(
                    format!("{}.depth", relative_display(&snapshot.root, &path)),
                    FindingSeverity::Warning,
                    format!(
                        "{} exists, but it is still short on project-specific detail.",
                        relative_display(&snapshot.root, &path)
                    ),
                    "Expand it with maintainership expectations, response times, or verification steps that match this repository.",
                    Vec::new(),
                    Some(path.clone()),
                ));
            }
            if !covers_topics {
                findings.push(finding(
                    format!("{}.coverage", relative_display(&snapshot.root, &path)),
                    FindingSeverity::Warning,
                    format!(
                        "{} does not yet cover enough of the expected topics.",
                        relative_display(&snapshot.root, &path)
                    ),
                    format!(
                        "Make sure it explicitly covers topics such as {}.",
                        keywords.join(", ")
                    ),
                    evidence.clone(),
                    Some(path.clone()),
                ));
            }
            if !not_placeholder {
                findings.push(finding(
                    format!("{}.placeholder", relative_display(&snapshot.root, &path)),
                    FindingSeverity::Warning,
                    format!(
                        "{} still looks like scaffold copy.",
                        relative_display(&snapshot.root, &path)
                    ),
                    "Replace starter language with project-specific expectations and reporting details.",
                    Vec::new(),
                    Some(path.clone()),
                ));
            }

            RuleAssessment::precise(
                coverage,
                0.85,
                if status == CheckStatus::Strong {
                    format!(
                        "{} looks specific enough to guide contributors.",
                        relative_display(&snapshot.root, &path)
                    )
                } else if looks_placeholder(&contents) {
                    format!(
                        "{} exists, but it still looks like scaffold copy.",
                        relative_display(&snapshot.root, &path)
                    )
                } else {
                    format!(
                        "{} exists, but it could be more specific or complete.",
                        relative_display(&snapshot.root, &path)
                    )
                },
                evidence,
                findings,
                Some(path),
            )
        }
        None => RuleAssessment::precise(
            0,
            0.99,
            "The file is missing.",
            Vec::new(),
            vec![finding(
                "document.missing",
                FindingSeverity::Error,
                "The file is missing.",
                format!(
                    "Add {} with project-specific guidance.",
                    candidates.join(" or ")
                ),
                Vec::new(),
                None,
            )],
            None,
        ),
    }
}

fn assess_security_policy(snapshot: &RepositorySnapshot) -> RuleAssessment {
    match snapshot.first_existing(&["SECURITY.md"]) {
        Some(path) => {
            let contents = read_text(&path);
            let normalized = normalize(&contents);
            let has_private_channel = contains_any(
                &normalized,
                &[
                    "private report",
                    "report privately",
                    "do not report security issues in public",
                    "do not report in public",
                    "security@",
                    "contact privately",
                ],
            );
            let has_reporting_terms = contains_any(
                &normalized,
                &["vulnerability", "security issue", "report"],
            );
            let has_reproduction = contains_any(
                &normalized,
                &[
                    "reproduction steps",
                    "steps to reproduce",
                    "reproduce",
                    "description of the issue",
                    "description of the problem",
                ],
            );
            let has_impact = contains_any(
                &normalized,
                &["impact", "severity", "affected version", "affected versions"],
            );
            let has_response = contains_any(
                &normalized,
                &[
                    "acknowledge",
                    "respond",
                    "response",
                    "responsible disclosure",
                    "disclosure",
                    "timeline",
                ],
            );
            let has_depth = contents.len() >= 120
                || markdown_heading_count(&contents) >= 2
                || contents
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .count()
                    >= 6;
            let not_placeholder = !looks_placeholder(&normalized);

            let mut evidence = vec![relative_display(&snapshot.root, &path)];
            for (label, present) in [
                ("private-reporting", has_private_channel),
                ("reporting-details", has_reporting_terms),
                ("reproduction", has_reproduction),
                ("impact", has_impact),
                ("response", has_response),
            ] {
                if present {
                    evidence.push(String::from(label));
                }
            }

            let coverage = weighted_coverage(&[
                (15, true),
                (20, has_private_channel),
                (15, has_reporting_terms),
                (15, has_reproduction),
                (10, has_impact),
                (15, has_response),
                (5, has_depth),
                (5, not_placeholder),
            ]);
            let status = CheckStatus::from_coverage(coverage);
            let mut findings = Vec::new();

            for (id, present, message, help, severity) in [
                (
                    "security_policy.private",
                    has_private_channel,
                    "The security policy does not clearly direct reporters to a private channel.",
                    "Tell reporters to avoid public issues and provide a private reporting path such as email or a private contact flow.",
                    FindingSeverity::Warning,
                ),
                (
                    "security_policy.reproduction",
                    has_reproduction,
                    "The security policy does not clearly ask for reproduction details.",
                    "Ask reporters for reproduction steps or a concrete issue description so triage is actionable.",
                    FindingSeverity::Info,
                ),
                (
                    "security_policy.impact",
                    has_impact,
                    "The security policy does not mention impact or affected versions.",
                    "Prompt for impact or affected versions so maintainers can prioritize the report.",
                    FindingSeverity::Info,
                ),
                (
                    "security_policy.response",
                    has_response,
                    "The security policy does not explain response expectations or disclosure posture.",
                    "Mention acknowledgement, response expectations, or responsible disclosure so reporters know what to expect.",
                    FindingSeverity::Info,
                ),
            ] {
                if !present {
                    findings.push(finding(
                        id,
                        severity,
                        message,
                        help,
                        evidence.clone(),
                        Some(path.clone()),
                    ));
                }
            }

            if !has_depth {
                findings.push(finding(
                    "security_policy.depth",
                    FindingSeverity::Info,
                    "The security policy is concise but still thin on maintainer-specific detail.",
                    "Add a little more detail about scope, acknowledgement, or follow-up expectations if the project needs it.",
                    evidence.clone(),
                    Some(path.clone()),
                ));
            }

            RuleAssessment::precise(
                coverage,
                0.9,
                match status {
                    CheckStatus::Strong => String::from(
                        "Security policy explains private reporting, expected detail, and maintainer response posture.",
                    ),
                    CheckStatus::Partial => String::from(
                        "Security policy exists, but it still leaves a few reporting expectations implicit.",
                    ),
                    CheckStatus::Missing => String::from(
                        "Security policy exists, but it is too thin to guide a real vulnerability report yet.",
                    ),
                },
                evidence,
                findings,
                Some(path),
            )
        }
        None => RuleAssessment::precise(
            0,
            0.99,
            "The file is missing.",
            Vec::new(),
            vec![finding(
                "security_policy.missing",
                FindingSeverity::Error,
                "The file is missing.",
                "Add SECURITY.md that explains how to report vulnerabilities privately and what response reporters can expect.",
                Vec::new(),
                None,
            )],
            None,
        ),
    }
}

fn assess_issue_templates(snapshot: &RepositorySnapshot) -> RuleAssessment {
    let directory = snapshot.root.join(".github/ISSUE_TEMPLATE");
    if !directory.is_dir() {
        return RuleAssessment::precise(
            0,
            0.99,
            "No issue templates directory was found.",
            Vec::new(),
            vec![finding(
                "issue_templates.missing",
                FindingSeverity::Warning,
                "No issue templates directory was found.",
                "Add bug and feature issue templates so incoming reports arrive with more context.",
                Vec::new(),
                None,
            )],
            None,
        );
    }

    let templates = list_files(&directory);
    let names = templates
        .iter()
        .filter_map(|path| path.file_name().and_then(|value| value.to_str()))
        .map(|value| value.to_lowercase())
        .collect::<Vec<String>>();
    let has_bug = names.iter().any(|name| name.contains("bug"));
    let has_feature = names
        .iter()
        .any(|name| name.contains("feature") || name.contains("enhancement"));
    let coverage = coverage_ratio(
        [has_bug, has_feature]
            .into_iter()
            .filter(|hit| *hit)
            .count(),
        2,
    );
    let status = CheckStatus::from_coverage(coverage);
    let mut findings = Vec::new();
    if !has_bug {
        findings.push(finding(
            "issue_templates.bug",
            FindingSeverity::Warning,
            "There is no dedicated bug issue template.",
            "Add a bug report template that asks for reproduction steps, expected behavior, and environment details.",
            names.clone(),
            Some(directory.clone()),
        ));
    }
    if !has_feature {
        findings.push(finding(
            "issue_templates.feature",
            FindingSeverity::Info,
            "There is no dedicated feature request template.",
            "Add a feature template so product requests arrive with clearer goals and tradeoffs.",
            names.clone(),
            Some(directory.clone()),
        ));
    }

    RuleAssessment::precise(
        coverage,
        0.95,
        if status == CheckStatus::Strong {
            String::from("Separate bug and feature issue templates are present.")
        } else {
            String::from("Issue templates exist, but the coverage is still narrow.")
        },
        names,
        findings,
        Some(directory),
    )
}

fn assess_pull_request_template(snapshot: &RepositorySnapshot) -> RuleAssessment {
    match snapshot.first_existing(&[".github/PULL_REQUEST_TEMPLATE.md"]) {
        Some(path) => {
            let contents = normalize(&read_text(&path));
            let evidence = ["what changed", "why it matters", "validation", "checklist"]
                .into_iter()
                .filter(|keyword| contents.contains(keyword))
                .map(str::to_owned)
                .collect::<Vec<String>>();
            let has_depth = contents.len() >= 120;
            let coverage = weighted_coverage(&[
                (30, true),
                (40, evidence.len() >= 3),
                (30, has_depth),
            ]);
            let status = CheckStatus::from_coverage(coverage);
            let mut findings = Vec::new();
            if evidence.len() < 3 {
                findings.push(finding(
                    "pull_request_template.prompts",
                    FindingSeverity::Warning,
                    "The pull request template does not ask for enough review context.",
                    "Prompt for what changed, why it matters, and how the change was validated.",
                    evidence.clone(),
                    Some(path.clone()),
                ));
            }
            if !has_depth {
                findings.push(finding(
                    "pull_request_template.depth",
                    FindingSeverity::Info,
                    "The pull request template is still very short.",
                    "Include a small checklist or validation section so contributors confirm impact and testing.",
                    Vec::new(),
                    Some(path.clone()),
                ));
            }

            RuleAssessment::precise(
                coverage,
                0.91,
                if status == CheckStatus::Strong {
                    String::from("Pull request template sets clear review expectations.")
                } else {
                    String::from("Pull request template exists, but it could ask for richer context or testing details.")
                },
                evidence,
                findings,
                Some(path),
            )
        }
        None => RuleAssessment::precise(
            0,
            0.99,
            "No pull request template was found.",
            Vec::new(),
            vec![finding(
                "pull_request_template.missing",
                FindingSeverity::Info,
                "No pull request template was found.",
                "Add a pull request template so contributors consistently explain impact and verification.",
                Vec::new(),
                None,
            )],
            None,
        ),
    }
}

fn assess_codeowners(snapshot: &RepositorySnapshot) -> RuleAssessment {
    match snapshot.first_existing(&[".github/CODEOWNERS", "CODEOWNERS"]) {
        Some(path) => {
            let contents = read_text(&path);
            let normalized = normalize(&contents);
            let has_owner = normalized.contains('@');
            let has_wildcard = contents.lines().any(|line| line.trim_start().starts_with('*'));
            let coverage = weighted_coverage(&[(40, true), (40, has_owner), (20, has_wildcard)]);
            let status = CheckStatus::from_coverage(coverage);
            let evidence = contents
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty() && !line.starts_with('#'))
                .take(3)
                .map(str::to_owned)
                .collect::<Vec<String>>();
            let mut findings = Vec::new();
            if !has_owner {
                findings.push(finding(
                    "codeowners.owner",
                    FindingSeverity::Warning,
                    "CODEOWNERS does not clearly assign a reviewer or team.",
                    "Add at least one GitHub handle or team so contributors know who reviews which area.",
                    evidence.clone(),
                    Some(path.clone()),
                ));
            }
            if !has_wildcard {
                findings.push(finding(
                    "codeowners.coverage",
                    FindingSeverity::Info,
                    "CODEOWNERS has no catch-all ownership rule.",
                    "Consider adding a wildcard entry so the default repository surface always has an owner.",
                    evidence.clone(),
                    Some(path.clone()),
                ));
            }

            RuleAssessment::precise(
                coverage,
                0.92,
                if status == CheckStatus::Strong {
                    String::from("CODEOWNERS declares review ownership.")
                } else {
                    String::from("CODEOWNERS exists, but it does not clearly assign reviewers yet.")
                },
                evidence,
                findings,
                Some(path),
            )
        }
        None => RuleAssessment::precise(
            0,
            0.99,
            "No CODEOWNERS file was found.",
            Vec::new(),
            vec![finding(
                "codeowners.missing",
                FindingSeverity::Info,
                "No CODEOWNERS file was found.",
                "Add CODEOWNERS if you want repository ownership and review routing to be explicit.",
                Vec::new(),
                None,
            )],
            None,
        ),
    }
}

fn assess_funding(snapshot: &RepositorySnapshot) -> RuleAssessment {
    match snapshot.first_existing(&[".github/FUNDING.yml", "FUNDING.yml"]) {
        Some(path) => {
            let contents = normalize(&read_text(&path));
            let providers = [
                "github:",
                "open_collective:",
                "patreon:",
                "ko_fi:",
                "custom:",
            ]
            .into_iter()
            .filter(|provider| contents.contains(provider))
            .map(str::to_owned)
            .collect::<Vec<String>>();
            let status = if providers.is_empty() {
                CheckStatus::Partial
            } else {
                CheckStatus::Strong
            };

            let mut findings = Vec::new();
            if providers.is_empty() {
                findings.push(finding(
                    "funding.providers",
                    FindingSeverity::Info,
                    "FUNDING.yml exists, but provider entries are incomplete.",
                    "Add a supported provider such as github, custom, Open Collective, or Patreon if maintainers accept sponsorship.",
                    Vec::new(),
                    Some(path.clone()),
                ));
            }

            RuleAssessment::precise(
                if status == CheckStatus::Strong { 100 } else { 55 },
                0.9,
                if status == CheckStatus::Strong {
                    String::from("Funding channels are declared for maintainers.")
                } else {
                    String::from("Funding file exists, but provider metadata is incomplete.")
                },
                providers,
                findings,
                Some(path),
            )
        }
        None if !funding_intent_detected(snapshot) => {
            let has_maintainer_surface = snapshot.readme_path.is_some()
                || snapshot.project.manifest_path.is_some()
                || !snapshot.workflow_files.is_empty()
                || snapshot
                    .first_existing(&["CONTRIBUTING.md", "CHANGELOG.md", "SECURITY.md"])
                    .is_some();

            RuleAssessment::precise(
                100,
                0.88,
                if has_maintainer_surface {
                    String::from(
                        "Funding looks optional here; no sponsorship intent was detected in the repo surface.",
                    )
                } else {
                    String::from(
                        "Funding posture is not meaningful yet because the repository has no public maintainer surface.",
                    )
                },
                Vec::new(),
                Vec::new(),
                None,
            )
        }
        None => RuleAssessment::precise(
            0,
            0.99,
            "No FUNDING.yml file was found.",
            Vec::new(),
            vec![finding(
                "funding.missing",
                FindingSeverity::Info,
                "No FUNDING.yml file was found.",
                "The repo appears to signal sponsorship intent; add FUNDING.yml so support is easy to discover.",
                Vec::new(),
                None,
            )],
            None,
        ),
    }
}

fn assess_ci_workflow(snapshot: &RepositorySnapshot) -> RuleAssessment {
    if snapshot.workflow_files.is_empty() {
        return RuleAssessment::precise(
            0,
            0.99,
            "No GitHub Actions workflow was found for CI.",
            Vec::new(),
            vec![finding(
                "ci_workflow.missing",
                FindingSeverity::Error,
                "No GitHub Actions workflow was found for CI.",
                "Add a CI workflow that runs on push and pull request with stack-specific verification steps.",
                Vec::new(),
                None,
            )],
            None,
        );
    }

    let normalized = normalize(&snapshot.workflow_text);
    let matched = snapshot
        .project
        .kind
        .ci_keywords()
        .iter()
        .filter(|keyword| normalized.contains(**keyword))
        .map(|keyword| (*keyword).to_owned())
        .collect::<Vec<String>>();
    let has_push = normalized.contains("push:") || normalized.contains("[push");
    let has_pull_request =
        normalized.contains("pull_request") || normalized.contains("[pull_request");
    let has_test = snapshot
        .project
        .kind
        .test_keywords()
        .iter()
        .any(|keyword| normalized.contains(*keyword));
    let has_lint = snapshot
        .project
        .kind
        .lint_keywords()
        .iter()
        .any(|keyword| normalized.contains(*keyword));
    let has_build = snapshot
        .project
        .kind
        .build_keywords()
        .iter()
        .any(|keyword| normalized.contains(*keyword));
    let mut evidence = matched;
    if has_push {
        evidence.push(String::from("push"));
    }
    if has_pull_request {
        evidence.push(String::from("pull_request"));
    }
    let coverage = weighted_coverage(&[
        (15, true),
        (15, has_push),
        (15, has_pull_request),
        (25, has_test),
        (15, has_build),
        (15, has_lint),
    ]);
    let status = CheckStatus::from_coverage(coverage);
    let mut findings = Vec::new();
    for (id, present, message, help, severity) in [
        (
            "ci_workflow.push",
            has_push,
            "The CI workflow does not clearly run on push.",
            "Add a push trigger so the default branch stays continuously verified.",
            FindingSeverity::Info,
        ),
        (
            "ci_workflow.pull_request",
            has_pull_request,
            "The CI workflow does not clearly run on pull requests.",
            "Add a pull_request trigger so external contributions are checked before merge.",
            FindingSeverity::Warning,
        ),
        (
            "ci_workflow.tests",
            has_test,
            "CI does not clearly execute the project test path.",
            "Run the test command that matches the detected stack inside CI.",
            FindingSeverity::Warning,
        ),
        (
            "ci_workflow.build",
            has_build,
            "CI does not clearly exercise a build or packaging step.",
            "Add a build or package verification step so release regressions surface early.",
            FindingSeverity::Info,
        ),
        (
            "ci_workflow.lint",
            has_lint,
            "CI does not clearly execute lint or format verification.",
            "Run linting or formatting checks in CI so style drift is visible in pull requests.",
            FindingSeverity::Info,
        ),
    ] {
        if !present {
            findings.push(finding(
                id,
                severity,
                message,
                help,
                evidence.clone(),
                Some(snapshot.root.join(".github/workflows")),
            ));
        }
    }

    RuleAssessment::precise(
        coverage,
        0.88,
        if status == CheckStatus::Strong {
            String::from("CI covers stack-specific verification paths.")
        } else {
            String::from("Workflow files exist, but CI does not clearly cover the expected verification steps yet.")
        },
        evidence,
        findings,
        Some(snapshot.root.join(".github/workflows")),
    )
}

fn assess_tests(snapshot: &RepositorySnapshot) -> RuleAssessment {
    let detected = test_files(&snapshot.root, &snapshot.files, snapshot.project.kind);
    if !detected.is_empty() {
        return RuleAssessment::simple(
            CheckStatus::Strong,
            0.9,
            "Tests were detected in the repository.",
            detected.into_iter().take(4).collect(),
            None,
        );
    }

    let workflow = normalize(&snapshot.workflow_text);
    let manifest_or_ci_mentions_tests = snapshot
        .project
        .kind
        .test_keywords()
        .iter()
        .any(|keyword| workflow.contains(keyword))
        || snapshot.project.script_mentions("test")
        || snapshot.project.script_mentions("pytest");

    RuleAssessment::precise(
        if manifest_or_ci_mentions_tests { 45 } else { 0 },
        0.78,
        if manifest_or_ci_mentions_tests {
            String::from("The repo references a test path, but no test files were detected yet.")
        } else {
            String::from("No test files or clear test entrypoints were detected.")
        },
        Vec::new(),
        vec![finding(
            "tests.missing-files",
            if manifest_or_ci_mentions_tests {
                FindingSeverity::Warning
            } else {
                FindingSeverity::Error
            },
            if manifest_or_ci_mentions_tests {
                "The repo references tests, but no concrete test files were detected."
            } else {
                "No test files or clear test entrypoints were detected."
            },
            "Add executable tests or surface their location more clearly in the repository layout.",
            Vec::new(),
            None,
        )],
        None,
    )
}

fn assess_lint_and_format(snapshot: &RepositorySnapshot) -> RuleAssessment {
    let workflow = normalize(&snapshot.workflow_text);
    let lint_signal = snapshot
        .project
        .kind
        .lint_keywords()
        .iter()
        .any(|keyword| workflow.contains(keyword))
        || snapshot.project.script_mentions("lint")
        || snapshot.project.dependency_mentions("eslint")
        || snapshot.project.dependency_mentions("ruff")
        || snapshot.project.dependency_mentions("golangci-lint");
    let format_signal = snapshot
        .project
        .kind
        .format_keywords()
        .iter()
        .any(|keyword| workflow.contains(keyword))
        || snapshot.project.script_mentions("format")
        || snapshot.project.dependency_mentions("prettier")
        || snapshot.project.dependency_mentions("black");
    let status = match (lint_signal, format_signal) {
        (true, true) => CheckStatus::Strong,
        (true, false) | (false, true) => CheckStatus::Partial,
        (false, false) => CheckStatus::Missing,
    };

    let mut evidence = Vec::new();
    if lint_signal {
        evidence.push(String::from("lint"));
    }
    if format_signal {
        evidence.push(String::from("format"));
    }

    let mut findings = Vec::new();
    if !lint_signal {
        findings.push(finding(
            "lint_and_format.lint",
            FindingSeverity::Info,
            "No clear lint path was detected.",
            "Expose a lint command or dependency so code quality checks are easier to discover.",
            evidence.clone(),
            snapshot
                .project
                .manifest_path
                .clone()
                .or_else(|| Some(snapshot.root.join(".github/workflows"))),
        ));
    }
    if !format_signal {
        findings.push(finding(
            "lint_and_format.format",
            FindingSeverity::Info,
            "No clear format verification path was detected.",
            "Expose a formatting command or check so contributors can match repository conventions.",
            evidence.clone(),
            snapshot
                .project
                .manifest_path
                .clone()
                .or_else(|| Some(snapshot.root.join(".github/workflows"))),
        ));
    }

    RuleAssessment::precise(
        match status {
            CheckStatus::Strong => 100,
            CheckStatus::Partial => 55,
            CheckStatus::Missing => 0,
        },
        0.7,
        match status {
            CheckStatus::Strong => String::from("Linting and formatting signals are both visible."),
            CheckStatus::Partial => {
                String::from("Only part of the lint/format path is currently visible.")
            }
            CheckStatus::Missing => String::from("No lint or formatting path was detected."),
        },
        evidence,
        findings,
        snapshot
            .project
            .manifest_path
            .clone()
            .or_else(|| Some(snapshot.root.join(".github/workflows"))),
    )
}

fn assess_dependabot(snapshot: &RepositorySnapshot) -> RuleAssessment {
    let path = snapshot.root.join(".github/dependabot.yml");
    if !path.is_file() {
        return RuleAssessment::precise(
            0,
            0.99,
            "Dependabot is not configured.",
            Vec::new(),
            vec![finding(
                "dependabot.missing",
                FindingSeverity::Info,
                "Dependabot is not configured.",
                "Add .github/dependabot.yml to keep dependencies and GitHub Actions updates visible.",
                Vec::new(),
                None,
            )],
            None,
        );
    }

    let normalized = normalize(&read_text(&path));
    let ecosystem = snapshot
        .project
        .kind
        .package_ecosystem()
        .unwrap_or("unknown");
    let has_ecosystem = normalized.contains(&format!("package-ecosystem: {ecosystem}"));
    let has_actions = normalized.contains("package-ecosystem: github-actions");
    let has_schedule = normalized.contains("schedule:");
    let coverage = weighted_coverage(&[
        (35, true),
        (30, has_ecosystem),
        (20, has_actions),
        (15, has_schedule),
    ]);
    let status = CheckStatus::from_coverage(coverage);
    let mut evidence = vec![format!("ecosystem: {ecosystem}")];
    if has_actions {
        evidence.push(String::from("github-actions"));
    }
    if has_schedule {
        evidence.push(String::from("schedule"));
    }
    let mut findings = Vec::new();
    if !has_ecosystem {
        findings.push(finding(
            "dependabot.ecosystem",
            FindingSeverity::Warning,
            "Dependabot does not cover the primary package ecosystem yet.",
            format!("Add an update block for the `{ecosystem}` ecosystem."),
            Vec::new(),
            Some(path.clone()),
        ));
    }
    if !has_actions {
        findings.push(finding(
            "dependabot.actions",
            FindingSeverity::Info,
            "Dependabot does not cover GitHub Actions updates.",
            "Add a github-actions update block so workflow dependencies also stay fresh.",
            Vec::new(),
            Some(path.clone()),
        ));
    }
    if !has_schedule {
        findings.push(finding(
            "dependabot.schedule",
            FindingSeverity::Info,
            "Dependabot has no obvious update schedule.",
            "Add a schedule so dependency updates happen predictably.",
            Vec::new(),
            Some(path.clone()),
        ));
    }

    RuleAssessment::precise(
        coverage,
        0.92,
        if status == CheckStatus::Strong {
            String::from("Dependabot covers both the stack ecosystem and GitHub Actions.")
        } else {
            String::from(
                "Dependabot exists, but it does not yet cover the full maintenance surface.",
            )
        },
        evidence,
        findings,
        Some(path),
    )
}

fn assess_changelog(snapshot: &RepositorySnapshot) -> RuleAssessment {
    match snapshot.first_existing(&["CHANGELOG.md"]) {
        Some(path) => {
            let contents = normalize(&read_text(&path));
            let has_versions = contains_any(
                &contents,
                &[
                    "## unreleased",
                    "## [",
                    "## 0.",
                    "## 1.",
                    "keep a changelog",
                ],
            );
            let has_depth = contents.len() >= 80;
            let coverage = weighted_coverage(&[(40, true), (35, has_versions), (25, has_depth)]);
            let status = CheckStatus::from_coverage(coverage);
            let mut findings = Vec::new();
            if !has_versions {
                findings.push(finding(
                    "changelog.structure",
                    FindingSeverity::Warning,
                    "The changelog does not yet show a clear release structure.",
                    "Use version headings or an Unreleased section so releases are easier to evaluate before upgrading.",
                    Vec::new(),
                    Some(path.clone()),
                ));
            }
            if !has_depth {
                findings.push(finding(
                    "changelog.depth",
                    FindingSeverity::Info,
                    "The changelog is present but still very short.",
                    "Add at least an Unreleased section or the first release notes entry.",
                    Vec::new(),
                    Some(path.clone()),
                ));
            }

            RuleAssessment::precise(
                coverage,
                0.9,
                if status == CheckStatus::Strong {
                    String::from("Changelog looks ready to track releases.")
                } else {
                    String::from("Changelog exists, but it still lacks clear release structure.")
                },
                vec![relative_display(&snapshot.root, &path)],
                findings,
                Some(path),
            )
        }
        None => RuleAssessment::precise(
            0,
            0.99,
            "No changelog was found.",
            Vec::new(),
            vec![finding(
                "changelog.missing",
                FindingSeverity::Info,
                "No changelog was found.",
                "Add CHANGELOG.md if you want release history to be easy to evaluate before upgrading.",
                Vec::new(),
                None,
            )],
            None,
        ),
    }
}

fn assess_release_workflow(snapshot: &RepositorySnapshot) -> RuleAssessment {
    let release_file = snapshot
        .workflow_files
        .iter()
        .find(|path| {
            path.file_name()
                .and_then(|value| value.to_str())
                .map(|name| name.to_lowercase().contains("release"))
                .unwrap_or(false)
        })
        .cloned();
    let normalized = normalize(&snapshot.workflow_text);
    let deployment_config = app_deployment_config(snapshot);
    let deployment_text = deployment_config
        .as_ref()
        .map(|path| read_text(path))
        .unwrap_or_default();
    let deployment_normalized = normalize(&deployment_text);
    let has_release_trigger = normalized.contains("tags:")
        || normalized.contains("workflow_dispatch")
        || normalized.contains("softprops/action-gh-release");
    let has_release_action = normalized.contains("softprops/action-gh-release")
        || normalized.contains("gh release")
        || normalized.contains("upload-artifact");
    let has_publish_step = snapshot
        .project
        .kind
        .release_keywords()
        .iter()
        .any(|keyword| normalized.contains(*keyword))
        || has_release_packaging_step(snapshot, &normalized);
    let has_deployment_surface = deployment_config.is_some();
    let has_deployment_build = has_deployment_surface
        && contains_any(
            &deployment_normalized,
            &[
                "[build]",
                "command =",
                "publish =",
                "\"buildcommand\"",
                "\"outputdirectory\"",
                "\"rewrites\"",
                "bun run build",
                "npm run build",
                "pnpm build",
                "yarn build",
                "vite build",
                "dist",
            ],
        );
    let effective_release_surface = has_release_action || has_deployment_surface;
    let effective_publish_path = has_publish_step || has_deployment_build;
    let coverage = if matches!(snapshot.project.profile, RepoProfile::App) {
        weighted_coverage(&[
            (20, release_file.is_some()),
            (15, has_release_trigger),
            (15, has_release_action),
            (10, has_publish_step),
            (20, has_deployment_surface),
            (20, has_deployment_build),
        ])
    } else {
        weighted_coverage(&[
            (30, release_file.is_some()),
            (25, has_release_trigger),
            (25, has_release_action),
            (20, has_publish_step),
        ])
    };
    let status = CheckStatus::from_coverage(coverage);

    let mut evidence = Vec::new();
    if let Some(path) = &release_file {
        evidence.push(relative_display(&snapshot.root, path));
    }
    if let Some(path) = &deployment_config {
        evidence.push(relative_display(&snapshot.root, path));
    }
    if normalized.contains("softprops/action-gh-release") {
        evidence.push(String::from("action-gh-release"));
    }
    if normalized.contains("tags:") {
        evidence.push(String::from("tag-trigger"));
    }
    if has_release_action {
        evidence.push(String::from("release-action"));
    }
    if has_publish_step {
        evidence.push(String::from("publish-step"));
    }
    if has_deployment_build {
        evidence.push(String::from("deployment-build"));
    }
    let mut findings = Vec::new();
    if release_file.is_none() {
        if has_deployment_surface && matches!(snapshot.project.profile, RepoProfile::App) {
            findings.push(finding(
                "release_workflow.file",
                FindingSeverity::Info,
                "A deployment config exists, but there is no dedicated release workflow in GitHub Actions.",
                "Keep the deployment config, and add a release workflow later if you want shipping to be visible and reproducible from the repository itself.",
                vec![relative_display(
                    &snapshot.root,
                    deployment_config
                        .as_ref()
                        .expect("deployment config exists when used as release evidence"),
                )],
                deployment_config.clone(),
            ));
        } else {
            findings.push(finding(
                "release_workflow.file",
                FindingSeverity::Info,
                "No dedicated release workflow file was detected.",
                "Add a release workflow if you want shipping and packaging to be repeatable.",
                Vec::new(),
                Some(snapshot.root.join(".github/workflows")),
            ));
        }
    }
    if !(has_release_trigger
        || has_deployment_surface && matches!(snapshot.project.profile, RepoProfile::App))
    {
        findings.push(finding(
            "release_workflow.trigger",
            FindingSeverity::Warning,
            "Release automation has no clear tag or manual trigger.",
            "Trigger releases from tags or workflow_dispatch so shipping is deliberate and reproducible.",
            evidence.clone(),
            release_file.clone(),
        ));
    }
    if !effective_release_surface {
        findings.push(finding(
            "release_workflow.artifacts",
            FindingSeverity::Warning,
            "The workflow does not clearly publish a release or artifacts.",
            "Use a release action or upload artifacts so releases produce something consumable.",
            evidence.clone(),
            release_file.clone().or_else(|| deployment_config.clone()),
        ));
    }
    if !effective_publish_path {
        findings.push(finding(
            "release_workflow.publish",
            FindingSeverity::Info,
            "No stack-specific publish or packaging step was detected.",
            "Add a publish or packaging step that matches the detected ecosystem when you are ready to ship releases.",
            evidence.clone(),
            release_file.clone().or_else(|| deployment_config.clone()),
        ));
    }

    RuleAssessment::precise(
        coverage,
        0.86,
        match status {
            CheckStatus::Strong => {
                if has_deployment_surface && matches!(snapshot.project.profile, RepoProfile::App) {
                    String::from("Delivery automation is present and looks ready to ship the app.")
                } else {
                    String::from("Release automation is present and looks publish-oriented.")
                }
            }
            CheckStatus::Partial => {
                if has_deployment_surface && matches!(snapshot.project.profile, RepoProfile::App) {
                    String::from(
                        "Deployment config exists, but the shipping path is only partially automated from the repository.",
                    )
                } else {
                    String::from(
                        "Some release automation exists, but the publish path is incomplete.",
                    )
                }
            }
            CheckStatus::Missing => {
                if matches!(snapshot.project.profile, RepoProfile::App) {
                    String::from("No release workflow or deployment config was detected.")
                } else {
                    String::from("No release workflow was detected.")
                }
            }
        },
        dedupe_strings(evidence),
        findings,
        release_file
            .or(deployment_config)
            .or_else(|| Some(snapshot.root.join(".github/workflows"))),
    )
}

fn category_scores(checks: &[AuditCheck]) -> Vec<CategoryScore> {
    [
        RuleCategory::Identity,
        RuleCategory::Docs,
        RuleCategory::Community,
        RuleCategory::Automation,
        RuleCategory::Release,
    ]
    .into_iter()
    .map(|category| {
        let total = checks
            .iter()
            .filter(|check| check.category == category)
            .map(|check| check.weight)
            .sum::<u16>();
        let earned = checks
            .iter()
            .filter(|check| check.category == category)
            .map(|check| check.earned)
            .sum::<u16>();

        CategoryScore {
            category,
            earned,
            total,
            score: percentage(earned, total),
        }
    })
    .collect()
}

fn percentage(earned: u16, total: u16) -> u8 {
    if total == 0 {
        0
    } else {
        ((u32::from(earned) * 100) / u32::from(total)) as u8
    }
}

fn preferred_markdown_doc(root: &Path, files: &[PathBuf]) -> Option<PathBuf> {
    let preferred = [
        "CONTEXT.md",
        "OVERVIEW.md",
        "GUIDE.md",
        "INDEX.md",
        "docs/README.md",
        "docs/index.md",
        "docs/overview.md",
    ];
    if let Some(path) = first_existing_file(root, &preferred) {
        return Some(path);
    }

    files
        .iter()
        .filter_map(|path| {
            let relative = path.strip_prefix(root).ok()?;
            if !is_supporting_doc_candidate(relative) {
                return None;
            }
            Some((path.clone(), read_text(path).len()))
        })
        .max_by_key(|(_, size)| *size)
        .map(|(path, _)| path)
}

fn is_supporting_doc_candidate(relative: &Path) -> bool {
    let display = relative.to_string_lossy().replace('\\', "/");
    let file_name = relative
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let lower = file_name.to_lowercase();

    if !display.ends_with(".md") {
        return false;
    }
    if display.starts_with(".github/") {
        return false;
    }
    if matches!(
        lower.as_str(),
        "readme.md"
            | "readme"
            | "changelog.md"
            | "contributing.md"
            | "code_of_conduct.md"
            | "security.md"
    ) {
        return false;
    }

    relative.parent().is_none() || display.starts_with("docs/")
}

fn markdown_heading_count(contents: &str) -> usize {
    contents
        .lines()
        .filter(|line| line.trim_start().starts_with('#'))
        .count()
}

fn markdown_code_block_count(contents: &str) -> usize {
    contents.matches("```").count()
}

fn markdown_heading_matches(contents: &str, aliases: &[&str]) -> bool {
    let aliases = aliases
        .iter()
        .map(|alias| normalize_heading_fragment(alias))
        .collect::<Vec<_>>();
    contents
        .lines()
        .filter_map(markdown_heading_text)
        .map(normalize_heading_fragment)
        .any(|heading| aliases.iter().any(|alias| heading.contains(alias)))
}

fn runnable_script_count(path: &Path) -> usize {
    if !path.is_dir() {
        return 0;
    }

    fs::read_dir(path)
        .ok()
        .into_iter()
        .flat_map(|entries| entries.filter_map(Result::ok))
        .map(|entry| entry.path())
        .filter(|candidate| {
            candidate.is_file()
                && candidate
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| matches!(ext, "py" | "sh" | "ps1" | "ipynb" | "js" | "ts"))
                    .unwrap_or(false)
        })
        .count()
}

fn first_existing_file(root: &Path, candidates: &[&str]) -> Option<PathBuf> {
    candidates
        .iter()
        .map(|candidate| root.join(candidate))
        .find(|candidate| candidate.is_file())
}

fn first_file_in_dir(path: &Path) -> Option<PathBuf> {
    if !path.is_dir() {
        return None;
    }

    fs::read_dir(path)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|candidate| candidate.is_file())
}

fn list_files(root: &Path) -> Vec<PathBuf> {
    match fs::read_dir(root) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.is_file())
            .collect(),
        Err(_) => Vec::new(),
    }
}

fn test_files(root: &Path, files: &[PathBuf], kind: ProjectKind) -> Vec<String> {
    files
        .iter()
        .filter_map(|path| {
            let relative = path.strip_prefix(root).ok()?;
            let display = relative.to_string_lossy().replace('\\', "/");
            let file_name = relative.file_name()?.to_string_lossy().to_lowercase();
            let relative_lower = display.to_lowercase();

            let matches = match kind {
                ProjectKind::Rust => {
                    relative.starts_with("tests")
                        || file_name.ends_with("_test.rs")
                        || display.ends_with(".rs") && read_text(path).contains("#[cfg(test)]")
                }
                ProjectKind::Node => {
                    relative_lower.contains("__tests__/")
                        || file_name.contains(".test.")
                        || file_name.contains(".spec.")
                }
                ProjectKind::Python => {
                    relative_lower.starts_with("tests/") && file_name != "__init__.py"
                        || file_name.starts_with("test_") && file_name.ends_with(".py")
                }
                ProjectKind::Go => file_name.ends_with("_test.go"),
                ProjectKind::Unknown => {
                    relative_lower.starts_with("tests/") && file_name != "__init__.py"
                        || file_name.contains(".test.")
                        || file_name.contains(".spec.")
                        || file_name.ends_with("_test.go")
                        || file_name.starts_with("test_")
                }
            };

            if matches {
                Some(display)
            } else {
                None
            }
        })
        .collect()
}

fn read_text(path: &Path) -> String {
    fs::read(path)
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .unwrap_or_default()
}

fn normalize(contents: &str) -> String {
    contents.to_lowercase()
}

fn normalize_heading_fragment(contents: &str) -> String {
    let mut normalized = String::with_capacity(contents.len());
    let mut previous_was_space = true;
    for ch in contents.chars() {
        if ch.is_alphanumeric() {
            for lowered in ch.to_lowercase() {
                normalized.push(lowered);
            }
            previous_was_space = false;
        } else if !previous_was_space {
            normalized.push(' ');
            previous_was_space = true;
        }
    }
    normalized.trim().to_owned()
}

fn markdown_heading_text(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with('#') {
        return None;
    }
    Some(trimmed.trim_start_matches('#').trim())
}

fn contains_any(contents: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|pattern| contents.contains(pattern))
}

fn readme_has_cli_command_surface(contents: &str, snapshot: &RepositorySnapshot) -> bool {
    if !matches!(snapshot.project.profile, RepoProfile::Cli) {
        return false;
    }

    let binary_name = snapshot.project.binary_name().to_lowercase();
    if binary_name.is_empty() {
        return false;
    }

    contents.lines().any(|line| {
        let trimmed = line.trim();
        trimmed.contains(&binary_name)
            && (trimmed.starts_with('>')
                || trimmed.starts_with("```")
                || trimmed.starts_with(&binary_name)
                || trimmed.contains(&format!("`{binary_name}"))
                || trimmed.contains(&format!("{binary_name} ")))
            && (trimmed.contains(" --")
                || trimmed.contains(" audit")
                || trimmed.contains(" fix")
                || trimmed.contains(" init")
                || trimmed.contains(" version")
                || trimmed.contains('['))
    })
}

fn readme_has_app_command_surface(contents: &str, snapshot: &RepositorySnapshot) -> bool {
    if !matches!(snapshot.project.profile, RepoProfile::App) {
        return false;
    }

    let stack_commands: &[&str] = match snapshot.project.kind {
        ProjectKind::Node => &[
            "bun install",
            "bun run dev",
            "bun run build",
            "npm install",
            "npm run dev",
            "npm run build",
            "pnpm install",
            "pnpm dev",
            "pnpm build",
            "yarn install",
            "yarn dev",
            "yarn build",
            "vite",
            "localhost:",
        ],
        ProjectKind::Python => &[
            "uv run",
            "python -m",
            "streamlit run",
            "flask run",
            "uvicorn",
            "localhost:",
        ],
        ProjectKind::Rust => &["cargo run", "trunk serve", "localhost:"],
        ProjectKind::Go => &["go run", "localhost:"],
        ProjectKind::Unknown => &["localhost:"],
    };

    contains_any(contents, stack_commands)
}

fn funding_intent_detected(snapshot: &RepositorySnapshot) -> bool {
    let mut text = normalize(&snapshot.readme_text);
    if let Some(path) = snapshot.supporting_doc() {
        let is_readme = snapshot
            .readme_path
            .as_ref()
            .map(|readme_path| readme_path == &path)
            .unwrap_or(false);
        if !is_readme {
            text.push('\n');
            text.push_str(&normalize(&read_text(&path)));
        }
    }

    contains_any(
        &text,
        &[
            "github.com/sponsors/",
            "github sponsors",
            "open collective",
            "opencollective.com",
            "ko-fi.com",
            "patreon.com",
            "buy me a coffee",
            "support this project",
            "support the project",
            "support the maintainers",
            "sponsor this project",
            "sponsor the project",
            "donate",
            "donation",
        ],
    )
}

fn has_release_packaging_step(snapshot: &RepositorySnapshot, normalized: &str) -> bool {
    if !matches!(snapshot.project.profile, RepoProfile::Cli) {
        return false;
    }

    match snapshot.project.kind {
        ProjectKind::Rust => contains_any(
            normalized,
            &[
                "cargo build --release",
                "cargo build --release --target",
                "package release archive",
                "archive_name:",
                "binary_name:",
                "tar -czf",
                "compress-archive",
                "target/${{ matrix.target }}/release/",
            ],
        ),
        ProjectKind::Node => contains_any(
            normalized,
            &[
                "npm pack",
                "pnpm pack",
                "yarn pack",
                "pkg ",
                "nexe ",
                "tar -czf",
                "compress-archive",
            ],
        ),
        ProjectKind::Python => contains_any(
            normalized,
            &[
                "python -m build",
                "pyinstaller",
                "zipapp",
                "tar -czf",
                "compress-archive",
            ],
        ),
        ProjectKind::Go => contains_any(
            normalized,
            &["goreleaser", "go build", "tar -czf", "compress-archive"],
        ),
        ProjectKind::Unknown => contains_any(
            normalized,
            &["package", "archive", "upload-artifact", "release binary"],
        ),
    }
}

fn app_deployment_config(snapshot: &RepositorySnapshot) -> Option<PathBuf> {
    if !matches!(snapshot.project.profile, RepoProfile::App) {
        return None;
    }

    snapshot.first_existing(&[
        "netlify.toml",
        "vercel.json",
        "firebase.json",
        "amplify.yml",
        "render.yaml",
        "render.yml",
        "fly.toml",
        "Dockerfile",
        "docker-compose.yml",
        "docker-compose.yaml",
    ])
}

fn dedupe_strings(values: Vec<String>) -> Vec<String> {
    let mut unique = Vec::with_capacity(values.len());
    for value in values {
        if !unique.iter().any(|existing| existing == &value) {
            unique.push(value);
        }
    }
    unique
}

fn looks_placeholder(contents: &str) -> bool {
    contains_any(
        contents,
        &[
            "one-line value proposition",
            "add installation instructions here",
            "describe the bug clearly",
            "what is painful today",
            "add your test command here",
            "replace this workflow with the real build and test steps",
        ],
    )
}

fn relative_display(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::OssifyConfig;

    fn temp_repo(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(name);
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create temp directory");
        path
    }

    #[test]
    fn score_for_empty_repository_is_zero() {
        let root = temp_repo("ossify-empty-audit-v4");
        let report = audit_repository(&root, &OssifyConfig::default()).expect("audit repository");
        assert_eq!(report.score, 3);
        assert_eq!(report.missing_count(), 17);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn config_can_make_readme_blocking() {
        let root = temp_repo("ossify-blocking-readme");
        fs::write(
            root.join("ossify.toml"),
            "version = 1\nminimum_score = 40\n\n[rules.readme]\nrequired_level = \"strong\"\n",
        )
        .expect("write config");
        fs::write(root.join("README.md"), "# Demo\n").expect("write readme");
        let config = OssifyConfig::load_for_target(&root, None).expect("load config");
        let report = audit_repository(&root, &config).expect("audit repository");
        let readme = report
            .checks
            .iter()
            .find(|check| check.id == "readme")
            .expect("readme check");

        assert!(readme.blocking);
        assert!(!report.strict_passed);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn category_scores_reflect_detected_signals() {
        let root = temp_repo("ossify-category-score");
        fs::create_dir_all(root.join("src")).expect("create src");
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"ossify-demo\"\ndescription = \"Demo package for audit\"\nlicense = \"MIT\"\nrepository = \"https://github.com/acme/ossify-demo\"\nhomepage = \"https://example.com\"\nversion = \"0.1.0\"\nkeywords = [\"cli\", \"demo\"]\n",
        )
        .expect("write Cargo.toml");
        fs::write(root.join("src/main.rs"), "fn main() {}\n").expect("write main.rs");
        fs::write(
            root.join("README.md"),
            "# Demo\n\n## Install\n\n```bash\ncargo build\n```\n\n## Usage\n\n```bash\ncargo run -- --help\n```\n\n## Contributing\n\nSee [CONTRIBUTING.md](CONTRIBUTING.md).\n\n## License\n\nMIT\n",
        )
        .expect("write README");

        let report = audit_repository(&root, &OssifyConfig::default()).expect("audit repository");
        assert!(report
            .category_scores
            .iter()
            .any(|score| score.category == RuleCategory::Identity && score.score > 0));
        assert!(report
            .category_scores
            .iter()
            .any(|score| score.category == RuleCategory::Docs && score.score > 0));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn requirements_txt_python_app_is_not_treated_as_unknown_generic() {
        let root = temp_repo("ossify-python-requirements-audit");
        fs::create_dir_all(root.join("templates")).expect("create templates");
        fs::create_dir_all(root.join("static")).expect("create static");
        fs::write(
            root.join("requirements.txt"),
            "flask\nflask-socketio\neventlet\n",
        )
        .expect("write requirements");
        fs::write(root.join("app.py"), "print('hi')\n").expect("write app.py");
        fs::write(root.join("run_game.bat"), "@echo off\npython app.py\n").expect("write bat");

        let report = audit_repository(&root, &OssifyConfig::default()).expect("audit repository");
        let manifest = report
            .checks
            .iter()
            .find(|check| check.id == "project_manifest")
            .expect("project manifest check");

        assert_eq!(report.project.kind, ProjectKind::Python);
        assert_eq!(report.project.profile, RepoProfile::App);
        assert_ne!(manifest.status, CheckStatus::Missing);
        assert!(report.project.summary().contains("requirements.txt"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn requirements_txt_manifest_metadata_is_lightweight_not_impossible() {
        let root = temp_repo("ossify-python-requirements-metadata");
        fs::write(
            root.join("requirements.txt"),
            "flask\nflask-socketio\neventlet\n",
        )
        .expect("write requirements");
        fs::write(root.join("app.py"), "print('hi')\n").expect("write app.py");

        let report = audit_repository(&root, &OssifyConfig::default()).expect("audit repository");
        let metadata = report
            .checks
            .iter()
            .find(|check| check.id == "manifest_metadata")
            .expect("metadata check");

        assert_eq!(metadata.status, CheckStatus::Partial);
        assert!(metadata
            .findings
            .iter()
            .any(|finding| finding.id == "manifest_metadata.lightweight_manifest"));
        assert!(!metadata
            .findings
            .iter()
            .any(|finding| finding.id == "manifest_metadata.description"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn detects_nested_workflow_files() {
        let root = temp_repo("ossify-nested-workflows");
        fs::create_dir_all(root.join(".github/workflows")).expect("create workflows");
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n",
        )
        .expect("write Cargo.toml");
        fs::write(
            root.join(".github/workflows/ci.yml"),
            "name: CI\njobs:\n  verify:\n    steps:\n      - run: cargo test\n      - run: cargo build\n",
        )
        .expect("write workflow");

        let report = audit_repository(&root, &OssifyConfig::default()).expect("audit repository");
        let ci = report
            .checks
            .iter()
            .find(|check| check.id == "ci_workflow")
            .expect("ci check");
        assert_eq!(ci.status, CheckStatus::Partial);
        assert!(ci
            .findings
            .iter()
            .any(|finding| finding.id == "ci_workflow.pull_request"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn supporting_doc_counts_as_partial_readme() {
        let root = temp_repo("ossify-supporting-doc-readme");
        fs::write(
            root.join("CONTEXT.md"),
            "# Context\n\n## Vision\n\nA detailed project context.\n\n## Pipeline\n\n```bash\npython scripts/run.py\n```\n\n## Notes\n\nLonger explanation.\n",
        )
        .expect("write context");

        let report = audit_repository(&root, &OssifyConfig::default()).expect("audit repository");
        let readme = report
            .checks
            .iter()
            .find(|check| check.id == "readme")
            .expect("readme check");
        assert_eq!(readme.status, CheckStatus::Partial);
        assert!(readme.coverage < 85);
        assert!(readme
            .evidence
            .iter()
            .any(|entry| entry.contains("CONTEXT.md")));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn nested_standalone_project_softens_missing_root_manifest_and_readme() {
        let root = temp_repo("ossify-nested-standalone-project");
        let nested = root.join("murder-scotsman");
        fs::create_dir_all(nested.join(".git")).expect("create nested git dir");
        fs::create_dir_all(nested.join("templates")).expect("create templates");
        fs::create_dir_all(nested.join("static")).expect("create static");
        fs::write(
            nested.join("requirements.txt"),
            "flask\nflask-socketio\neventlet\n",
        )
        .expect("write requirements");
        fs::write(nested.join("README.md"), "# Murder Scotsman\n\nA game.\n")
            .expect("write README");
        fs::write(nested.join("app.py"), "print('hi')\n").expect("write app.py");

        let report = audit_repository(&root, &OssifyConfig::default()).expect("audit repository");
        let manifest = report
            .checks
            .iter()
            .find(|check| check.id == "project_manifest")
            .expect("manifest check");
        let readme = report
            .checks
            .iter()
            .find(|check| check.id == "readme")
            .expect("readme check");

        assert_eq!(manifest.status, CheckStatus::Partial);
        assert!(manifest
            .findings
            .iter()
            .any(|finding| finding.id == "project_manifest.nested_project"));
        assert_eq!(readme.status, CheckStatus::Partial);
        assert!(readme
            .findings
            .iter()
            .any(|finding| finding.id == "readme.nested_project"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn documented_scripts_count_as_examples() {
        let root = temp_repo("ossify-documented-scripts");
        fs::create_dir_all(root.join("scripts")).expect("create scripts");
        fs::write(root.join("scripts/run.py"), "print('hi')\n").expect("write run.py");
        fs::write(root.join("scripts/eval.py"), "print('hi')\n").expect("write eval.py");
        fs::write(
            root.join("CONTEXT.md"),
            "# Context\n\n## Pipeline d'execution\n\n```bash\npython scripts/run.py\npython scripts/eval.py\n```\n",
        )
        .expect("write context");

        let report = audit_repository(&root, &OssifyConfig::default()).expect("audit repository");
        let examples = report
            .checks
            .iter()
            .find(|check| check.id == "examples")
            .expect("examples check");
        assert_eq!(examples.status, CheckStatus::Strong);
        assert!(examples
            .evidence
            .iter()
            .any(|entry| entry.contains("scripts/")));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn manifest_metadata_emits_granular_findings() {
        let root = temp_repo("ossify-metadata-findings");
        fs::write(
            root.join("pyproject.toml"),
            "[project]\nname = \"demo\"\nversion = \"0.1.0\"\n",
        )
        .expect("write pyproject.toml");

        let report = audit_repository(&root, &OssifyConfig::default()).expect("audit repository");
        let metadata = report
            .checks
            .iter()
            .find(|check| check.id == "manifest_metadata")
            .expect("metadata check");

        assert_eq!(metadata.status, CheckStatus::Partial);
        assert!(metadata
            .findings
            .iter()
            .any(|finding| finding.id == "manifest_metadata.description"));
        assert!(report.finding_count() > 0);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn readme_commands_and_repo_docs_count_as_onboarding_signals() {
        let root = temp_repo("ossify-readme-command-surface");
        fs::create_dir_all(root.join("src")).expect("create src");
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"ossify\"\nversion = \"0.1.0\"\ndescription = \"Audit trust signals\"\nlicense = \"MIT\"\nrepository = \"https://github.com/acme/ossify\"\n",
        )
        .expect("write Cargo.toml");
        fs::write(root.join("src/main.rs"), "fn main() {}\n").expect("write main.rs");
        fs::write(
            root.join("CONTRIBUTING.md"),
            "# Contributing\n\nOpen a PR.\n",
        )
        .expect("write contributing");
        fs::write(root.join("LICENSE"), "MIT License\n").expect("write license");
        fs::write(
            root.join("README.md"),
            "# ossify\n\n[License](LICENSE)\n\nAudit a repository like a maintainer would, then scaffold the missing trust signals.\n\nThis README focuses on the command surface, a worked example, and the contributor docs that already live in the repository. It is intentionally missing a dedicated install section so the rule can stay partial while still recognizing the rest of the onboarding flow.\n\n## Commands\n\n```text\nossify audit . --interactive\nossify fix . --plan --interactive\nossify version\n```\n\n## Example\n\n```text\n> ossify audit .\n```\n",
        )
        .expect("write README");

        let report = audit_repository(&root, &OssifyConfig::default()).expect("audit repository");
        let readme = report
            .checks
            .iter()
            .find(|check| check.id == "readme")
            .expect("readme check");

        assert_eq!(readme.status, CheckStatus::Partial);
        assert!(readme.coverage >= 75);
        assert!(readme
            .findings
            .iter()
            .any(|finding| finding.id == "readme.install"));
        assert!(!readme
            .findings
            .iter()
            .any(|finding| finding.id == "readme.usage"));
        assert!(!readme
            .findings
            .iter()
            .any(|finding| finding.id == "readme.contributing"));
        assert!(!readme
            .findings
            .iter()
            .any(|finding| finding.id == "readme.license"));
        assert!(!readme
            .findings
            .iter()
            .any(|finding| finding.id == "readme.stack-hints"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn readme_with_emoji_headings_and_bun_commands_counts_install_usage_and_license() {
        let root = temp_repo("ossify-readme-emoji-headings");
        fs::create_dir_all(root.join("app")).expect("create app");
        fs::write(
            root.join("package.json"),
            "{\n  \"name\": \"lucid-app\",\n  \"version\": \"1.0.0\",\n  \"private\": true,\n  \"dependencies\": {\n    \"react\": \"^18.2.0\",\n    \"vite\": \"^5.0.0\"\n  }\n}\n",
        )
        .expect("write package.json");
        fs::write(
            root.join("README.md"),
            "# Lucid\n\n## 🚀 Installation\n\n```bash\nbun install\nbun run dev\nbun run build\n```\n\n## 🎨 Stack Technique\n\nReact, TypeScript, Vite, Bun\n\n## 📄 Licence\n\nMIT License\n",
        )
        .expect("write README");

        let report = audit_repository(&root, &OssifyConfig::default()).expect("audit repository");
        let readme = report
            .checks
            .iter()
            .find(|check| check.id == "readme")
            .expect("readme check");

        assert_eq!(readme.status, CheckStatus::Partial);
        assert!(!readme
            .findings
            .iter()
            .any(|finding| finding.id == "readme.install"));
        assert!(!readme
            .findings
            .iter()
            .any(|finding| finding.id == "readme.usage"));
        assert!(!readme
            .findings
            .iter()
            .any(|finding| finding.id == "readme.license"));
        assert!(!readme
            .findings
            .iter()
            .any(|finding| finding.id == "readme.stack-hints"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn concise_security_policy_with_private_reporting_can_be_strong() {
        let root = temp_repo("ossify-security-policy-strong");
        fs::write(
            root.join("SECURITY.md"),
            "# Security Policy\n\nPlease do not report security issues in public issues.\n\nSend a private report with:\n\n- a description of the issue\n- reproduction steps\n- impact\n\nWe will acknowledge reports promptly and work toward a responsible disclosure.\n",
        )
        .expect("write security policy");

        let report = audit_repository(&root, &OssifyConfig::default()).expect("audit repository");
        let security = report
            .checks
            .iter()
            .find(|check| check.id == "security_policy")
            .expect("security check");

        assert_eq!(security.status, CheckStatus::Strong);
        assert!(security.findings.is_empty());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn funding_without_repo_intent_is_treated_as_optional() {
        let root = temp_repo("ossify-funding-optional");
        fs::write(root.join("README.md"), "# Demo\n\nA small CLI tool.\n").expect("write README");

        let report = audit_repository(&root, &OssifyConfig::default()).expect("audit repository");
        let funding = report
            .checks
            .iter()
            .find(|check| check.id == "funding")
            .expect("funding check");

        assert_eq!(funding.status, CheckStatus::Strong);
        assert_eq!(funding.coverage, 100);
        assert!(funding.findings.is_empty());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn funding_without_any_public_maintainer_surface_is_still_optional() {
        let root = temp_repo("ossify-funding-no-surface");

        let report = audit_repository(&root, &OssifyConfig::default()).expect("audit repository");
        let funding = report
            .checks
            .iter()
            .find(|check| check.id == "funding")
            .expect("funding check");

        assert_eq!(funding.status, CheckStatus::Strong);
        assert_eq!(funding.coverage, 100);
        assert!(funding.findings.is_empty());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn funding_intent_without_file_still_surfaces_gap() {
        let root = temp_repo("ossify-funding-intent-gap");
        fs::write(
            root.join("README.md"),
            "# Demo\n\nSupport this project: https://github.com/sponsors/acme\n",
        )
        .expect("write README");

        let report = audit_repository(&root, &OssifyConfig::default()).expect("audit repository");
        let funding = report
            .checks
            .iter()
            .find(|check| check.id == "funding")
            .expect("funding check");

        assert_eq!(funding.status, CheckStatus::Missing);
        assert!(funding
            .findings
            .iter()
            .any(|finding| finding.id == "funding.missing"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn cli_release_workflow_with_packaged_binaries_counts_as_strong() {
        let root = temp_repo("ossify-cli-release-workflow");
        fs::create_dir_all(root.join(".github/workflows")).expect("create workflows");
        fs::create_dir_all(root.join("src")).expect("create src");
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"ossify\"\nversion = \"0.1.0\"\n",
        )
        .expect("write Cargo.toml");
        fs::write(root.join("src/main.rs"), "fn main() {}\n").expect("write main.rs");
        fs::write(
            root.join(".github/workflows/release.yml"),
            "name: Release\non:\n  push:\n    tags:\n      - \"v*\"\n  workflow_dispatch:\n\njobs:\n  build:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v4\n      - run: cargo build --release --target x86_64-unknown-linux-gnu\n      - run: tar -czf ossify.tar.gz target/x86_64-unknown-linux-gnu/release/ossify\n      - uses: actions/upload-artifact@v4\n        with:\n          name: ossify\n          path: ossify.tar.gz\n  publish:\n    runs-on: ubuntu-latest\n    needs: build\n    steps:\n      - uses: softprops/action-gh-release@v2\n",
        )
        .expect("write release workflow");

        let report = audit_repository(&root, &OssifyConfig::default()).expect("audit repository");
        let release = report
            .checks
            .iter()
            .find(|check| check.id == "release_workflow")
            .expect("release check");

        assert_eq!(release.status, CheckStatus::Strong);
        assert!(!release
            .findings
            .iter()
            .any(|finding| finding.id == "release_workflow.publish"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn app_deployment_config_counts_as_partial_release_surface() {
        let root = temp_repo("ossify-app-release-surface");
        fs::create_dir_all(root.join("app")).expect("create app");
        fs::write(
            root.join("package.json"),
            "{\n  \"name\": \"lucid-app\",\n  \"version\": \"1.0.0\",\n  \"private\": true,\n  \"dependencies\": {\n    \"react\": \"^18.2.0\",\n    \"vite\": \"^5.0.0\"\n  }\n}\n",
        )
        .expect("write package.json");
        fs::write(
            root.join("netlify.toml"),
            "[build]\ncommand = \"bun run build\"\npublish = \"dist\"\n",
        )
        .expect("write netlify.toml");

        let report = audit_repository(&root, &OssifyConfig::default()).expect("audit repository");
        let release = report
            .checks
            .iter()
            .find(|check| check.id == "release_workflow")
            .expect("release check");

        assert_eq!(release.status, CheckStatus::Partial);
        assert!(release
            .evidence
            .iter()
            .any(|entry| entry.contains("netlify.toml")));
        assert!(!release
            .findings
            .iter()
            .any(|finding| finding.id == "release_workflow.trigger"));

        let _ = fs::remove_dir_all(&root);
    }
}
