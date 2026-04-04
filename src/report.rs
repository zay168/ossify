use std::collections::{BTreeMap, BTreeSet};
use std::io;

use crossterm::terminal;
use serde_json::json;

use crate::audit::AuditReport;
use crate::cli::OutputFormat;
use crate::clipboard::{copy_prompt_report, PROMPT_COPIED_MESSAGE, PROMPT_COPY_FAILED_PREFIX};
use crate::doctor::{
    DepsDoctorReport, DocsDoctorReport, DoctorEcosystem, DoctorFinding, DoctorSeverity,
    EcosystemDoctorScore, ReleaseDoctorReport, WorkflowDoctorReport,
};
use crate::generator::{FixReport, InitReport, PlanReport};
use crate::prompt::BugPromptReport;
use crate::ui::{self, UiReport};

#[derive(Clone, Copy)]
pub struct OutputOptions {
    pub format: OutputFormat,
    pub color: bool,
    pub interactive: bool,
}

pub fn print_audit_report(report: &AuditReport, options: &OutputOptions) -> io::Result<()> {
    match options.format {
        OutputFormat::Human => {
            let model = UiReport::from_audit(report);
            if options.interactive && ui::supports_interactive() {
                ui::run_interactive_audit(model)
            } else {
                println!("{}", ui::render_audit(&model, options.color));
                Ok(())
            }
        }
        OutputFormat::Json => {
            println!("{}", render_audit_json(report));
            Ok(())
        }
    }
}

pub fn print_init_report(report: &InitReport, options: &OutputOptions) -> io::Result<()> {
    match options.format {
        OutputFormat::Human => {
            println!("{}", ui::render_init_report(report, options.color));
            Ok(())
        }
        OutputFormat::Json => {
            println!("{}", render_init_json(report));
            Ok(())
        }
    }
}

pub fn print_fix_report(report: &FixReport, options: &OutputOptions) -> io::Result<()> {
    match options.format {
        OutputFormat::Human => {
            println!(
                "{}",
                ui::render_fix(&UiReport::from_fix(report), options.color)
            );
            Ok(())
        }
        OutputFormat::Json => {
            println!("{}", render_fix_json(report));
            Ok(())
        }
    }
}

pub fn print_plan_report(report: &PlanReport, options: &OutputOptions) -> io::Result<()> {
    match options.format {
        OutputFormat::Human => {
            let model = UiReport::from_plan(report);
            if options.interactive && ui::supports_interactive() {
                ui::run_interactive_plan(model)
            } else {
                println!("{}", ui::render_plan(&model, options.color));
                Ok(())
            }
        }
        OutputFormat::Json => {
            println!("{}", render_plan_json(report));
            Ok(())
        }
    }
}

pub fn print_bug_prompt_report(
    report: &BugPromptReport,
    options: &OutputOptions,
) -> io::Result<()> {
    match options.format {
        OutputFormat::Human => {
            println!("{}", ui::render_prompt_report(report, options.color));
            let notice = match copy_prompt_report(report) {
                Ok(()) => String::from(PROMPT_COPIED_MESSAGE),
                Err(error) => format!("{PROMPT_COPY_FAILED_PREFIX} {error}"),
            };
            println!("\n{notice}");
            Ok(())
        }
        OutputFormat::Json => {
            println!("{}", render_bug_prompt_json(report));
            Ok(())
        }
    }
}

pub fn print_docs_doctor_report(
    report: &DocsDoctorReport,
    options: &OutputOptions,
) -> io::Result<()> {
    match options.format {
        OutputFormat::Human => {
            println!("{}", render_docs_doctor_human(report));
            Ok(())
        }
        OutputFormat::Json => {
            println!("{}", render_docs_doctor_json(report));
            Ok(())
        }
    }
}

pub fn print_workflow_doctor_report(
    report: &WorkflowDoctorReport,
    options: &OutputOptions,
) -> io::Result<()> {
    match options.format {
        OutputFormat::Human => {
            println!("{}", render_workflow_doctor_human(report, options.color));
            Ok(())
        }
        OutputFormat::Json => {
            println!("{}", render_workflow_doctor_json(report));
            Ok(())
        }
    }
}

pub fn print_deps_doctor_report(
    report: &DepsDoctorReport,
    options: &OutputOptions,
) -> io::Result<()> {
    match options.format {
        OutputFormat::Human => {
            println!("{}", render_deps_doctor_human(report, options.color));
            Ok(())
        }
        OutputFormat::Json => {
            println!("{}", render_deps_doctor_json(report));
            Ok(())
        }
    }
}

pub fn print_release_doctor_report(
    report: &ReleaseDoctorReport,
    options: &OutputOptions,
) -> io::Result<()> {
    match options.format {
        OutputFormat::Human => {
            println!("{}", render_release_doctor_human(report, options.color));
            Ok(())
        }
        OutputFormat::Json => {
            println!("{}", render_release_doctor_json(report));
            Ok(())
        }
    }
}

fn render_audit_json(report: &AuditReport) -> String {
    serde_json::to_string(&json!({
        "command": "audit",
        "target": &report.target,
        "project": &report.project,
        "readiness": report.readiness.as_str(),
        "score": report.score,
        "base_score": report.base_score,
        "minimum_score": report.minimum_score,
        "strict_passed": report.strict_passed,
        "config_source": &report.config_source,
        "strong_count": report.strong_count(),
        "partial_count": report.partial_count(),
        "missing_count": report.missing_count(),
        "diagnostic_count": report.finding_count(),
        "category_scores": &report.category_scores,
        "domain_scores": &report.domain_scores,
        "checks": &report.checks,
        "diagnostics": report.diagnostics(),
    }))
    .unwrap_or_else(|_| String::from("{}"))
}

fn render_init_json(report: &InitReport) -> String {
    serde_json::to_string(&json!({
        "command": report.mode.as_str(),
        "target": &report.target,
        "project": &report.project,
        "file_count": report.files.len(),
        "files": &report.files,
    }))
    .unwrap_or_else(|_| String::from("{}"))
}

fn render_fix_json(report: &FixReport) -> String {
    serde_json::to_string(&json!({
        "command": "fix",
        "target": &report.target,
        "before_score": report.before.score,
        "after_score": report.after.score,
        "score_delta": report.after.score as i16 - report.before.score as i16,
        "before": &report.before,
        "generated": &report.generated,
        "after": &report.after,
    }))
    .unwrap_or_else(|_| String::from("{}"))
}

fn render_plan_json(report: &PlanReport) -> String {
    serde_json::to_string(&json!({
        "command": "fix",
        "mode": "plan",
        "target": &report.target,
        "before_score": report.before.score,
        "estimated_after_score": report.estimated_after.score,
        "score_delta": report.estimated_after.score as i16 - report.before.score as i16,
        "before": &report.before,
        "planned": &report.planned,
        "estimated_after": &report.estimated_after,
    }))
    .unwrap_or_else(|_| String::from("{}"))
}

fn render_bug_prompt_json(report: &BugPromptReport) -> String {
    serde_json::to_string(&json!({
        "command": "prompt",
        "mode": "bug",
        "target": &report.target,
        "project": &report.project,
        "score": report.score,
        "readiness": report.readiness.as_str(),
        "minimum_score": report.minimum_score,
        "strong_count": report.strong_count,
        "partial_count": report.partial_count,
        "missing_count": report.missing_count,
        "prompt_count": report.prompt_count,
        "prompts": &report.prompts,
    }))
    .unwrap_or_else(|_| String::from("{}"))
}

fn render_docs_doctor_json(report: &DocsDoctorReport) -> String {
    serde_json::to_string(&json!({
        "command": "doctor",
        "doctor": "docs",
        "target": &report.target,
        "score": report.score,
        "markdown_file_count": report.markdown_files,
        "local_link_count": report.local_links_checked,
        "error_count": report.error_count(),
        "warning_count": report.warning_count(),
        "info_count": report.info_count(),
        "summary": report.summary(),
        "findings": &report.findings,
    }))
    .unwrap_or_else(|_| String::from("{}"))
}

fn render_docs_doctor_human(report: &DocsDoctorReport) -> String {
    let mut lines = vec![
        String::from("OSSIFY DOCS DOCTOR"),
        format!("Target: {}", report.target.display()),
        format!("Score: {}/100", report.score),
        format!("Markdown files scanned: {}", report.markdown_files),
        format!("Local links checked: {}", report.local_links_checked),
        format!("Summary: {}", report.summary()),
    ];

    if report.findings.is_empty() {
        lines.push(String::new());
        lines.push(String::from("No documentation findings."));
        return lines.join("\n");
    }

    lines.push(String::new());
    lines.push(String::from("Findings"));
    for finding in &report.findings {
        let location = finding
            .file
            .as_ref()
            .map(|path| {
                path.strip_prefix(&report.target)
                    .unwrap_or(path.as_path())
                    .display()
                    .to_string()
            })
            .unwrap_or_else(|| String::from("."));
        lines.push(format!(
            "[{}] {} | {}",
            finding.severity.label(),
            location,
            finding.message
        ));
        if let Some(help) = &finding.help {
            lines.push(format!("  -> {help}"));
        }
    }

    lines.join("\n")
}

fn render_workflow_doctor_json(report: &WorkflowDoctorReport) -> String {
    serde_json::to_string(&json!({
        "command": "doctor",
        "doctor": "workflow",
        "target": &report.target,
        "score": report.score,
        "workflow_file_count": report.workflow_files,
        "engine": report.engine,
        "engine_available": report.engine_available,
        "error_count": report.error_count(),
        "warning_count": report.warning_count(),
        "info_count": report.info_count(),
        "summary": report.summary(),
        "findings": &report.findings,
    }))
    .unwrap_or_else(|_| String::from("{}"))
}

fn render_workflow_doctor_human(report: &WorkflowDoctorReport, color: bool) -> String {
    let style = ReportStyle::new(color);
    let width = current_terminal_width();
    let mut lines = vec![
        style.badge("OSSIFY WORKFLOW DOCTOR", 96, true),
        format!(
            "{}  {}  {}  {}  {}  {}",
            style.label("score"),
            style.score(report.score),
            style.label("workflows"),
            report.workflow_files,
            style.label("engine"),
            if report.engine_available {
                style.good(&report.engine)
            } else {
                style.bad(&report.engine)
            }
        ),
        format!("{} {}", style.label("target"), report.target.display()),
        format!(
            "{} {}  {}  {}",
            style.label("summary"),
            report.summary(),
            format_severity_counts(
                &style,
                report.warning_count(),
                report.info_count(),
                report.error_count()
            ),
            if report.engine_available {
                style.muted("actionlint + ossify")
            } else {
                style.muted("bootstrap pending")
            }
        ),
    ];

    if report.findings.is_empty() {
        lines.push(String::new());
        lines.push(style.good("No workflow findings."));
        return lines.join("\n");
    }

    lines.push(String::new());
    lines.push(style.section("By File"));
    for file_report in build_workflow_file_reports(report) {
        lines.push(format!(
            "{}  {}  {}",
            style.file(&file_report.path),
            format_severity_counts(
                &style,
                file_report.warning_count,
                file_report.info_count,
                file_report.error_count
            ),
            style.muted(&format!("{} issue(s)", file_report.detail_count))
        ));
        for detail in file_report.details {
            lines.extend(wrap_prefixed(
                &format!("{} {}", style.dot(detail.severity), detail.message),
                "  ",
                width,
            ));
        }
    }

    let hints = workflow_next_fixes(report);
    if !hints.is_empty() {
        lines.push(String::new());
        lines.push(style.section("Next Fixes"));
        for hint in hints {
            lines.extend(wrap_prefixed(
                &format!("{} {}", style.muted("•"), hint),
                "  ",
                width,
            ));
        }
    }

    lines.join("\n")
}

fn render_deps_doctor_json(report: &DepsDoctorReport) -> String {
    serde_json::to_string(&json!({
        "command": "doctor",
        "doctor": "deps",
        "target": &report.target,
        "requested_ecosystem": report.requested_ecosystem,
        "score": report.domain.score,
        "cap": report.domain.cap,
        "cap_reason": report.domain.cap_reason,
        "cap_code": report.domain.cap_code,
        "engine": report.domain.engine,
        "engine_source": report.domain.engine_source,
        "ecosystem_count": report.ecosystems.len(),
        "ecosystems": &report.ecosystems,
        "error_count": report.error_count(),
        "warning_count": report.warning_count(),
        "info_count": report.info_count(),
        "summary": report.summary(),
        "findings": &report.findings,
    }))
    .unwrap_or_else(|_| String::from("{}"))
}

fn render_release_doctor_json(report: &ReleaseDoctorReport) -> String {
    serde_json::to_string(&json!({
        "command": "doctor",
        "doctor": "release",
        "target": &report.target,
        "requested_ecosystem": report.requested_ecosystem,
        "score": report.domain.score,
        "cap": report.domain.cap,
        "cap_reason": report.domain.cap_reason,
        "cap_code": report.domain.cap_code,
        "engine": report.domain.engine,
        "engine_source": report.domain.engine_source,
        "ecosystem_count": report.ecosystems.len(),
        "ecosystems": &report.ecosystems,
        "error_count": report.error_count(),
        "warning_count": report.warning_count(),
        "info_count": report.info_count(),
        "summary": report.summary(),
        "findings": &report.findings,
    }))
    .unwrap_or_else(|_| String::from("{}"))
}

fn render_deps_doctor_human(report: &DepsDoctorReport, color: bool) -> String {
    render_multi_domain_doctor_human(
        "OSSIFY DEPS DOCTOR",
        "deps",
        &report.target,
        report.domain.score,
        &report.domain.engine,
        &report.domain.summary,
        &report.ecosystems,
        &report.findings,
        color,
    )
}

fn render_release_doctor_human(report: &ReleaseDoctorReport, color: bool) -> String {
    render_multi_domain_doctor_human(
        "OSSIFY RELEASE DOCTOR",
        "release",
        &report.target,
        report.domain.score,
        &report.domain.engine,
        &report.domain.summary,
        &report.ecosystems,
        &report.findings,
        color,
    )
}

#[allow(clippy::too_many_arguments)]
fn render_multi_domain_doctor_human(
    title: &str,
    doctor_label: &str,
    target: &std::path::Path,
    score: Option<u8>,
    engine: &str,
    summary: &str,
    ecosystems: &[EcosystemDoctorScore],
    findings: &[DoctorFinding],
    color: bool,
) -> String {
    let style = ReportStyle::new(color);
    let width = current_terminal_width();
    let ecosystem_summary = if ecosystems.is_empty() {
        style.muted("none detected")
    } else {
        ecosystems
            .iter()
            .map(|entry| entry.ecosystem.label().to_owned())
            .collect::<Vec<_>>()
            .join(", ")
    };

    let mut lines = vec![
        style.badge(title, 96, true),
        format!(
            "{}  {}  {}  {}  {}  {}",
            style.label("score"),
            style.score(score),
            style.label("ecosystems"),
            ecosystem_summary,
            style.label("engine"),
            style.muted(engine)
        ),
        format!("{} {}", style.label("target"), target.display()),
        format!(
            "{} {}  {}  {}",
            style.label("summary"),
            summary,
            format_severity_counts(
                &style,
                findings
                    .iter()
                    .filter(|finding| finding.severity == DoctorSeverity::Warning)
                    .count(),
                findings
                    .iter()
                    .filter(|finding| finding.severity == DoctorSeverity::Info)
                    .count(),
                findings
                    .iter()
                    .filter(|finding| finding.severity == DoctorSeverity::Error)
                    .count()
            ),
            style.muted(&format!("{} policy blend", doctor_label))
        ),
    ];
    let dominant_cap_reason = ecosystems
        .iter()
        .filter_map(|entry| entry.cap_reason.as_ref().map(|reason| (entry.cap, reason)))
        .min_by_key(|(cap, _)| cap.unwrap_or(u8::MAX))
        .map(|(_, reason)| reason.clone());
    if let Some(reason) = dominant_cap_reason {
        lines.push(format!("{} {}", style.label("cap"), reason));
    }

    if !ecosystems.is_empty() {
        lines.push(String::new());
        lines.push(style.section("By Ecosystem"));
        for ecosystem in ecosystems {
            let cap = ecosystem
                .cap
                .map(|cap| format!(" cap {}", style.muted(&cap.to_string())))
                .unwrap_or_default();
            let detail = ecosystem
                .engine_detail
                .as_deref()
                .map(|_| {
                    format!(
                        " {}",
                        style.muted(&format!("[{}]", ecosystem.engine_status.label()))
                    )
                })
                .unwrap_or_else(|| {
                    if ecosystem.engine_status.label() == "managed" {
                        String::new()
                    } else {
                        format!(
                            " {}",
                            style.muted(&format!("[{}]", ecosystem.engine_status.label()))
                        )
                    }
                });
            lines.push(format!(
                "{}  {}  {}{}  {} finding(s){}",
                style.file(ecosystem.ecosystem.label()),
                style.score(Some(ecosystem.score)),
                style.muted(&ecosystem.engine),
                detail,
                ecosystem.finding_count,
                cap
            ));
            if let Some(reason) = &ecosystem.cap_reason {
                lines.extend(wrap_prefixed(
                    &format!("{} {}", style.muted("cap reason:"), reason),
                    "  ",
                    width,
                ));
            }
        }
    }

    if findings.is_empty() {
        lines.push(String::new());
        lines.push(style.good(&format!("No {} findings.", doctor_label)));
        return lines.join("\n");
    }

    lines.push(String::new());
    lines.push(style.section("By File"));
    for file_report in build_domain_file_reports(target, findings) {
        let ecosystem = file_report
            .ecosystems
            .iter()
            .map(|entry| entry.label())
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(format!(
            "{}  {}  {}  {}",
            style.file(&file_report.path),
            format_severity_counts(
                &style,
                file_report.warning_count,
                file_report.info_count,
                file_report.error_count
            ),
            style.muted(&format!("{} issue(s)", file_report.detail_count)),
            if ecosystem.is_empty() {
                String::new()
            } else {
                style.muted(&format!("[{ecosystem}]"))
            }
        ));
        for detail in file_report.details {
            lines.extend(wrap_prefixed(
                &format!("{} {}", style.dot(detail.severity), detail.message),
                "  ",
                width,
            ));
        }
    }

    let hints = domain_next_fixes(findings);
    if !hints.is_empty() {
        lines.push(String::new());
        lines.push(style.section("Next Fixes"));
        for hint in hints {
            lines.extend(wrap_prefixed(
                &format!("{} {}", style.muted("•"), hint),
                "  ",
                width,
            ));
        }
    }

    lines.join("\n")
}

#[derive(Debug)]
struct WorkflowFileReport {
    path: String,
    warning_count: usize,
    info_count: usize,
    error_count: usize,
    detail_count: usize,
    details: Vec<WorkflowDetail>,
}

#[derive(Debug)]
struct WorkflowDetail {
    severity: DoctorSeverity,
    message: String,
}

#[derive(Debug)]
struct DomainFileReport {
    path: String,
    ecosystems: BTreeSet<DoctorEcosystem>,
    warning_count: usize,
    info_count: usize,
    error_count: usize,
    detail_count: usize,
    details: Vec<WorkflowDetail>,
}

fn build_workflow_file_reports(report: &WorkflowDoctorReport) -> Vec<WorkflowFileReport> {
    let mut grouped = BTreeMap::<String, Vec<&DoctorFinding>>::new();
    for finding in &report.findings {
        let location = finding
            .file
            .as_ref()
            .map(|path| {
                path.strip_prefix(&report.target)
                    .unwrap_or(path.as_path())
                    .display()
                    .to_string()
            })
            .unwrap_or_else(|| String::from("."));
        grouped.entry(location).or_default().push(finding);
    }

    grouped
        .into_iter()
        .map(|(path, findings)| {
            let details = compact_workflow_details(&findings);
            WorkflowFileReport {
                path,
                warning_count: findings
                    .iter()
                    .filter(|finding| finding.severity == DoctorSeverity::Warning)
                    .count(),
                info_count: findings
                    .iter()
                    .filter(|finding| finding.severity == DoctorSeverity::Info)
                    .count(),
                error_count: findings
                    .iter()
                    .filter(|finding| finding.severity == DoctorSeverity::Error)
                    .count(),
                detail_count: findings.len(),
                details,
            }
        })
        .collect()
}

fn build_domain_file_reports(
    target: &std::path::Path,
    findings: &[DoctorFinding],
) -> Vec<DomainFileReport> {
    let mut grouped = BTreeMap::<String, Vec<&DoctorFinding>>::new();
    for finding in findings {
        let location = finding
            .file
            .as_ref()
            .map(|path| {
                path.strip_prefix(target)
                    .unwrap_or(path.as_path())
                    .display()
                    .to_string()
            })
            .unwrap_or_else(|| String::from("."));
        grouped.entry(location).or_default().push(finding);
    }

    grouped
        .into_iter()
        .map(|(path, findings)| DomainFileReport {
            path,
            ecosystems: findings
                .iter()
                .filter_map(|finding| finding.ecosystem)
                .collect(),
            warning_count: findings
                .iter()
                .filter(|finding| finding.severity == DoctorSeverity::Warning)
                .count(),
            info_count: findings
                .iter()
                .filter(|finding| finding.severity == DoctorSeverity::Info)
                .count(),
            error_count: findings
                .iter()
                .filter(|finding| finding.severity == DoctorSeverity::Error)
                .count(),
            detail_count: findings.len(),
            details: findings
                .into_iter()
                .map(|finding| WorkflowDetail {
                    severity: finding.severity,
                    message: finding.message.clone(),
                })
                .collect(),
        })
        .collect()
}

fn compact_workflow_details(findings: &[&DoctorFinding]) -> Vec<WorkflowDetail> {
    let mut details = Vec::new();

    for finding in findings {
        match finding.code.as_str() {
            "workflow.permissions.missing" => details.push(WorkflowDetail {
                severity: finding.severity,
                message: String::from("explicit `permissions:` block is missing"),
            }),
            "workflow.concurrency.missing" => details.push(WorkflowDetail {
                severity: finding.severity,
                message: String::from("no `concurrency` guard for superseded runs"),
            }),
            "workflow.actions.unpinned" => details.push(WorkflowDetail {
                severity: finding.severity,
                message: compact_mutable_refs_message(&finding.message),
            }),
            "workflow.timeout.missing" => details.push(WorkflowDetail {
                severity: finding.severity,
                message: finding.message.clone(),
            }),
            "workflow.engine-missing" | "workflow.engine-bootstrap-failed" => {
                details.push(WorkflowDetail {
                    severity: finding.severity,
                    message: finding.message.clone(),
                });
            }
            _ => details.push(WorkflowDetail {
                severity: finding.severity,
                message: finding.message.clone(),
            }),
        }
    }

    details.sort_by(|left, right| {
        workflow_severity_rank(left.severity).cmp(&workflow_severity_rank(right.severity))
    });
    details
}

fn compact_mutable_refs_message(message: &str) -> String {
    let refs = message
        .split(": ")
        .nth(1)
        .map(|tail| {
            tail.split(", ")
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if refs.is_empty() {
        return String::from("mutable action refs should be pinned to full commit SHAs");
    }

    format!("{} mutable action ref(s): {}", refs.len(), refs.join(", "))
}

fn workflow_next_fixes(report: &WorkflowDoctorReport) -> Vec<String> {
    let mut hints = BTreeSet::new();
    for finding in &report.findings {
        match finding.code.as_str() {
            "workflow.permissions.missing" => {
                hints.insert(String::from(
                    "Declare top-level `permissions:` in CI with the narrowest token scope.",
                ));
            }
            "workflow.concurrency.missing" => {
                hints.insert(String::from(
                    "Add `concurrency` to workflows where newer runs should cancel older ones.",
                ));
            }
            "workflow.timeout.missing" => {
                hints.insert(String::from(
                    "Add `timeout-minutes` to every job so stuck runners fail fast.",
                ));
            }
            "workflow.actions.unpinned" => {
                hints.insert(String::from(
                    "Pin third-party GitHub Actions to full commit SHAs for stronger supply-chain hygiene.",
                ));
            }
            "workflow.engine-bootstrap-failed" => {
                hints.insert(String::from(
                    "Let ossify bootstrap the managed engine or point `OSSIFY_ACTIONLINT` at an existing binary.",
                ));
            }
            _ => {}
        }
    }
    hints.into_iter().collect()
}

fn domain_next_fixes(findings: &[DoctorFinding]) -> Vec<String> {
    let mut hints = BTreeSet::new();
    for finding in findings {
        if let Some(fix_hint) = &finding.fix_hint {
            hints.insert(fix_hint.clone());
        } else if let Some(help) = &finding.help {
            hints.insert(help.clone());
        }
    }
    hints.into_iter().collect()
}

fn workflow_severity_rank(severity: DoctorSeverity) -> u8 {
    match severity {
        DoctorSeverity::Error => 0,
        DoctorSeverity::Warning => 1,
        DoctorSeverity::Info => 2,
    }
}

fn format_severity_counts(
    style: &ReportStyle,
    warnings: usize,
    infos: usize,
    errors: usize,
) -> String {
    let mut parts = Vec::new();
    if errors > 0 {
        parts.push(style.severity_badge(DoctorSeverity::Error, errors));
    }
    if warnings > 0 {
        parts.push(style.severity_badge(DoctorSeverity::Warning, warnings));
    }
    if infos > 0 {
        parts.push(style.severity_badge(DoctorSeverity::Info, infos));
    }
    if parts.is_empty() {
        style.good("clean")
    } else {
        parts.join(" ")
    }
}

fn wrap_prefixed(message: &str, indent: &str, width: usize) -> Vec<String> {
    let available = width.saturating_sub(indent.len()).max(24);
    let mut lines = Vec::new();
    let wrapped = wrap_words(message, available);
    for line in wrapped {
        lines.push(format!("{indent}{line}"));
    }
    lines
}

fn wrap_words(message: &str, width: usize) -> Vec<String> {
    if visible_width(message) <= width {
        return vec![message.to_owned()];
    }

    let mut lines = Vec::new();
    let mut current = String::new();
    for word in message.split_whitespace() {
        let candidate = if current.is_empty() {
            word.to_owned()
        } else {
            format!("{current} {word}")
        };
        if visible_width(&candidate) <= width {
            current = candidate;
        } else {
            if !current.is_empty() {
                lines.push(current);
            }
            if visible_width(word) > width {
                lines.push(truncate_visible(word, width));
                current = String::new();
            } else {
                current = word.to_owned();
            }
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

fn truncate_visible(message: &str, width: usize) -> String {
    let mut output = String::new();
    for ch in message.chars() {
        let candidate = format!("{output}{ch}");
        if visible_width(&candidate) > width {
            break;
        }
        output.push(ch);
    }
    output
}

fn visible_width(message: &str) -> usize {
    let mut width = 0usize;
    let mut chars = message.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' && matches!(chars.peek(), Some('[')) {
            let _ = chars.next();
            for next in chars.by_ref() {
                if ('@'..='~').contains(&next) {
                    break;
                }
            }
        } else {
            width += 1;
        }
    }
    width
}

fn current_terminal_width() -> usize {
    terminal::size()
        .map(|(width, _)| width as usize)
        .unwrap_or(100)
        .clamp(72, 140)
}

struct ReportStyle {
    enabled: bool,
}

impl ReportStyle {
    fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    fn badge(&self, text: &str, color: u8, bold: bool) -> String {
        let code = if bold {
            format!("1;{color}")
        } else {
            color.to_string()
        };
        self.paint(&code, text)
    }

    fn label(&self, text: &str) -> String {
        self.paint("90", text)
    }

    fn section(&self, text: &str) -> String {
        self.paint("1;90", text)
    }

    fn file(&self, text: &str) -> String {
        self.paint("1;97", text)
    }

    fn muted(&self, text: &str) -> String {
        self.paint("90", text)
    }

    fn good(&self, text: &str) -> String {
        self.paint("92", text)
    }

    fn bad(&self, text: &str) -> String {
        self.paint("91", text)
    }

    fn warn(&self, text: &str) -> String {
        self.paint("93", text)
    }

    fn info(&self, text: &str) -> String {
        self.paint("94", text)
    }

    fn dot(&self, severity: DoctorSeverity) -> String {
        match severity {
            DoctorSeverity::Error => self.bad("●"),
            DoctorSeverity::Warning => self.warn("●"),
            DoctorSeverity::Info => self.info("●"),
        }
    }

    fn severity_badge(&self, severity: DoctorSeverity, count: usize) -> String {
        let text = match severity {
            DoctorSeverity::Error => format!("err {count}"),
            DoctorSeverity::Warning => format!("warn {count}"),
            DoctorSeverity::Info => format!("info {count}"),
        };
        match severity {
            DoctorSeverity::Error => self.bad(&text),
            DoctorSeverity::Warning => self.warn(&text),
            DoctorSeverity::Info => self.info(&text),
        }
    }

    fn score(&self, score: Option<u8>) -> String {
        match score {
            Some(value) if value >= 85 => self.good(&format!("{value}/100")),
            Some(value) if value >= 60 => self.warn(&format!("{value}/100")),
            Some(value) => self.bad(&format!("{value}/100")),
            None => self.muted("unavailable"),
        }
    }

    fn paint(&self, code: &str, text: &str) -> String {
        if self.enabled {
            format!("\u{1b}[{code}m{text}\u{1b}[0m")
        } else {
            text.to_owned()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::audit_repository;
    use crate::config::OssifyConfig;
    use crate::generator::{plan_fix_repository, InitOptions, LicenseKind};
    use crate::prompt::build_bug_prompt_report;
    use std::fs;
    use std::path::PathBuf;
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
    fn plan_json_output_includes_mode_and_estimate() {
        let root = temp_repo("ossify-report-plan-json");
        fs::create_dir_all(root.join("src")).expect("create src");
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n",
        )
        .expect("write Cargo.toml");
        fs::write(root.join("src/main.rs"), "fn main() {}\n").expect("write main.rs");

        let options = InitOptions {
            overwrite: false,
            license: LicenseKind::Mit,
            owner: String::from("@acme"),
            funding: Some(String::from("github:acme")),
        };
        let report =
            plan_fix_repository(&root, &options, &OssifyConfig::default()).expect("plan fix");
        let rendered = render_plan_json(&report);

        assert!(rendered.contains(r#""mode":"plan""#));
        assert!(rendered.contains(r#""estimated_after""#));
        assert!(rendered.contains(r#""planned""#));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn audit_json_output_includes_v5_causal_fields() {
        let root = temp_repo("ossify-report-audit-json");
        fs::create_dir_all(root.join("src")).expect("create src");
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n",
        )
        .expect("write Cargo.toml");
        fs::write(root.join("src/main.rs"), "fn main() {}\n").expect("write main.rs");

        let report = audit_repository(&root, &OssifyConfig::default()).expect("audit repo");
        let rendered = render_audit_json(&report);

        assert!(rendered.contains(r#""primary_cause""#));
        assert!(rendered.contains(r#""proof""#));
        assert!(rendered.contains(r#""context_refs""#));
        assert!(rendered.contains(r#""retrieval_scope""#));
        assert!(rendered.contains(r#""history_refs""#));
        assert!(rendered.contains(r#""confidence_breakdown""#));
        assert!(rendered.contains(r#""domain_scores""#));
        assert!(rendered.contains(r#""base_score""#));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn bug_prompt_json_output_includes_prompt_payload() {
        let root = temp_repo("ossify-report-bug-prompt-json");
        fs::write(root.join("requirements.txt"), "flask\n").expect("write requirements");
        fs::write(root.join("app.py"), "print('hi')\n").expect("write app.py");

        let audit = audit_repository(&root, &OssifyConfig::default()).expect("audit repo");
        let prompt = build_bug_prompt_report(&audit, None, 1).expect("build bug prompt");
        let rendered = render_bug_prompt_json(&prompt);

        assert!(rendered.contains(r#""command":"prompt""#));
        assert!(rendered.contains(r#""mode":"bug""#));
        assert!(rendered.contains(r#""prompts""#));
        assert!(rendered.contains(r#""rule_id":"readme""#));

        let _ = fs::remove_dir_all(&root);
    }
}
