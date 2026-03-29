use crate::audit::{
    AuditDiagnostic, AuditReport, CheckStatus, FindingSeverity, Fixability, ReadinessTier,
    RuleCategory,
};
use crate::generator::{FileAction, FixReport, InitReport, PlanReport};
use crate::intel::ProofKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiMode {
    Audit,
    Fix,
    Plan,
}

#[derive(Debug, Clone, Copy)]
pub struct UiScoreSummary {
    pub score: u8,
    pub readiness: ReadinessTier,
    pub minimum_score: u8,
    pub strict_passed: bool,
}

#[derive(Debug, Clone)]
pub struct UiCategoryScore {
    pub label: &'static str,
    pub score: u8,
    pub earned: u16,
    pub total: u16,
}

#[derive(Debug, Clone)]
pub struct UiCheck {
    pub id: &'static str,
    pub label: String,
    pub status: CheckStatus,
    pub category: RuleCategory,
    pub fixability: Fixability,
    pub fixable: bool,
    pub gap: u16,
    pub coverage: u8,
    pub blocking: bool,
    pub message: String,
    pub hint: String,
    pub evidence: Vec<String>,
    pub primary_cause: Option<String>,
    pub primary_cause_detail: Option<String>,
    pub strongest_proof: Option<String>,
    pub strongest_contradiction: Option<String>,
    pub closest_context: Option<String>,
    pub retrieval_scope: Vec<String>,
    pub history_refs: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct UiDiagnostic {
    pub rule_id: &'static str,
    pub rule_label: String,
    pub category: RuleCategory,
    pub severity: FindingSeverity,
    pub check_status: CheckStatus,
    pub message: String,
    pub help: String,
    pub evidence: Vec<String>,
    pub location: Option<String>,
    pub primary_cause: Option<String>,
    pub strongest_proof: Option<String>,
    pub strongest_contradiction: Option<String>,
    pub closest_context: Option<String>,
    pub impact: f32,
}

#[derive(Debug, Clone)]
pub struct UiFile {
    pub action: FileAction,
    pub path: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UiReport {
    pub mode: UiMode,
    pub title: &'static str,
    pub target: String,
    pub project_name: String,
    pub project_summary: String,
    pub previous: Option<UiScoreSummary>,
    pub current: UiScoreSummary,
    pub categories: Vec<UiCategoryScore>,
    pub checks: Vec<UiCheck>,
    pub diagnostics: Vec<UiDiagnostic>,
    pub files: Vec<UiFile>,
    pub next_moves: Vec<String>,
}

impl UiReport {
    pub fn from_audit(report: &AuditReport) -> Self {
        Self {
            mode: UiMode::Audit,
            title: "OSSIFY REPORT",
            target: report.target.display().to_string(),
            project_name: report.project.name.clone(),
            project_summary: report.project.summary(),
            previous: None,
            current: score_summary(report),
            categories: map_categories(report),
            checks: map_checks(report),
            diagnostics: map_diagnostics(report),
            files: Vec::new(),
            next_moves: vec![
                String::from("ossify fix . --plan"),
                String::from("ossify audit . --strict"),
            ],
        }
    }

    pub fn from_fix(report: &FixReport) -> Self {
        Self {
            mode: UiMode::Fix,
            title: "OSSIFY FIX",
            target: report.target.display().to_string(),
            project_name: report.after.project.name.clone(),
            project_summary: report.after.project.summary(),
            previous: Some(score_summary(&report.before)),
            current: score_summary(&report.after),
            categories: map_categories(&report.after),
            checks: map_checks(&report.after),
            diagnostics: map_diagnostics(&report.after),
            files: map_files(&report.generated),
            next_moves: vec![String::from("ossify audit . --strict")],
        }
    }

    pub fn from_plan(report: &PlanReport) -> Self {
        Self {
            mode: UiMode::Plan,
            title: "OSSIFY PLAN",
            target: report.target.display().to_string(),
            project_name: report.estimated_after.project.name.clone(),
            project_summary: report.estimated_after.project.summary(),
            previous: Some(score_summary(&report.before)),
            current: score_summary(&report.estimated_after),
            categories: map_categories(&report.estimated_after),
            checks: map_checks(&report.estimated_after),
            diagnostics: map_diagnostics(&report.estimated_after),
            files: map_files(&report.planned),
            next_moves: vec![format!("ossify fix {}", report.target.display())],
        }
    }
}

fn score_summary(report: &AuditReport) -> UiScoreSummary {
    UiScoreSummary {
        score: report.score,
        readiness: report.readiness,
        minimum_score: report.minimum_score,
        strict_passed: report.strict_passed,
    }
}

fn map_categories(report: &AuditReport) -> Vec<UiCategoryScore> {
    report
        .category_scores
        .iter()
        .map(|score| UiCategoryScore {
            label: score.category.as_str(),
            score: score.score,
            earned: score.earned,
            total: score.total,
        })
        .collect()
}

fn map_checks(report: &AuditReport) -> Vec<UiCheck> {
    report
        .checks
        .iter()
        .map(|check| UiCheck {
            id: check.id,
            label: check.label.to_owned(),
            status: check.status,
            category: check.category,
            fixability: check.fixability,
            fixable: check.fixable,
            gap: check.gap(),
            coverage: check.coverage,
            blocking: check.blocking,
            message: check.message.clone(),
            hint: check.hint.to_owned(),
            evidence: check.evidence.clone(),
            primary_cause: check
                .primary_cause
                .as_ref()
                .map(|cause| cause.title.clone()),
            primary_cause_detail: check
                .primary_cause
                .as_ref()
                .map(|cause| cause.detail.clone()),
            strongest_proof: strongest_proof_text(check),
            strongest_contradiction: strongest_contradiction_text(check),
            closest_context: check
                .context_refs
                .first()
                .map(format_context_ref)
                .or_else(|| {
                    check
                        .location
                        .as_ref()
                        .map(|path| path.display().to_string())
                }),
            retrieval_scope: check.retrieval_scope.consulted_paths.clone(),
            history_refs: check
                .history_refs
                .iter()
                .map(|history| history.summary.clone())
                .collect(),
        })
        .collect()
}

fn map_diagnostics(report: &AuditReport) -> Vec<UiDiagnostic> {
    let mut diagnostics = report
        .diagnostics()
        .into_iter()
        .map(|diagnostic| {
            let check = report
                .checks
                .iter()
                .find(|check| check.id == diagnostic.rule_id);

            UiDiagnostic {
                rule_id: diagnostic.rule_id,
                rule_label: diagnostic.rule_label.to_owned(),
                category: diagnostic.category,
                severity: diagnostic.severity,
                check_status: check_status_for(report, &diagnostic),
                message: diagnostic.message,
                help: diagnostic.help,
                evidence: diagnostic.evidence,
                location: diagnostic.location.map(|path| path.display().to_string()),
                primary_cause: check.and_then(|check| {
                    check
                        .primary_cause
                        .as_ref()
                        .map(|cause| cause.title.clone())
                }),
                strongest_proof: check.and_then(strongest_proof_text),
                strongest_contradiction: check.and_then(strongest_contradiction_text),
                closest_context: check.and_then(|check| {
                    check
                        .context_refs
                        .first()
                        .map(format_context_ref)
                        .or_else(|| {
                            check
                                .location
                                .as_ref()
                                .map(|path| path.display().to_string())
                        })
                }),
                impact: check
                    .and_then(|check| check.primary_cause.as_ref().map(|cause| cause.impact))
                    .unwrap_or_default(),
            }
        })
        .collect::<Vec<_>>();

    diagnostics.sort_by(|left, right| {
        right
            .impact
            .total_cmp(&left.impact)
            .then_with(|| left.severity.rank().cmp(&right.severity.rank()))
            .then_with(|| left.rule_label.cmp(&right.rule_label))
    });
    diagnostics
}

fn check_status_for(report: &AuditReport, diagnostic: &AuditDiagnostic) -> CheckStatus {
    report
        .checks
        .iter()
        .find(|check| check.id == diagnostic.rule_id)
        .map(|check| check.status)
        .unwrap_or(CheckStatus::Missing)
}

fn map_files(report: &InitReport) -> Vec<UiFile> {
    report
        .files
        .iter()
        .map(|file| UiFile {
            action: file.action.clone(),
            path: file.path.display().to_string(),
            reason: file.reason.clone(),
        })
        .collect()
}

fn strongest_proof_text(check: &crate::audit::AuditCheck) -> Option<String> {
    check
        .proof
        .iter()
        .filter(|item| matches!(item.kind, ProofKind::Satisfied | ProofKind::Historical))
        .max_by(|left, right| proof_rank(left).total_cmp(&proof_rank(right)))
        .map(|item| format!("{}: {}", item.expectation, item.detail))
}

fn strongest_contradiction_text(check: &crate::audit::AuditCheck) -> Option<String> {
    check
        .proof
        .iter()
        .filter(|item| matches!(item.kind, ProofKind::Missing | ProofKind::Contradiction))
        .max_by(|left, right| proof_rank(left).total_cmp(&proof_rank(right)))
        .map(|item| format!("{}: {}", item.expectation, item.detail))
}

fn proof_rank(item: &crate::intel::ProofItem) -> f32 {
    item.weight as f32 * item.confidence
}

fn format_context_ref(context: &crate::intel::ContextRef) -> String {
    match (context.line_start, context.line_end) {
        (Some(start), Some(end)) if start == end => format!("{}:{}", context.path.display(), start),
        (Some(start), Some(end)) => format!("{}:{}-{}", context.path.display(), start, end),
        _ => context.path.display().to_string(),
    }
}
