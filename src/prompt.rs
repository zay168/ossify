use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::audit::{
    AuditCheck, AuditFinding, AuditReport, CheckStatus, FindingSeverity, Fixability, ReadinessTier,
    RuleCategory,
};
use crate::project::ProjectContext;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PromptStrategy {
    OneShot,
    Targeted,
}

impl PromptStrategy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OneShot => "one-shot",
            Self::Targeted => "targeted",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PromptIssue {
    pub rule_id: &'static str,
    pub rule_label: String,
    pub category: RuleCategory,
    pub status: CheckStatus,
    pub fixability: Fixability,
    pub gap: u16,
    pub blocking: bool,
    pub primary_cause: Option<String>,
    pub summary: String,
    pub hint: String,
    pub findings: Vec<String>,
    pub locations: Vec<String>,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GeneratedBugPrompt {
    pub strategy: PromptStrategy,
    pub title: String,
    pub issue_count: usize,
    pub selected_rules: Vec<String>,
    pub automatic_count: usize,
    pub guided_count: usize,
    pub manual_count: usize,
    pub preserve: Vec<String>,
    pub issues: Vec<PromptIssue>,
    pub prompt: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct BugPromptReport {
    pub target: PathBuf,
    pub project: ProjectContext,
    pub score: u8,
    pub readiness: ReadinessTier,
    pub minimum_score: u8,
    pub strong_count: usize,
    pub partial_count: usize,
    pub missing_count: usize,
    pub prompt_count: usize,
    pub prompts: Vec<GeneratedBugPrompt>,
}

pub fn build_bug_prompt_report(
    audit: &AuditReport,
    requested_rule: Option<&str>,
    count: usize,
) -> Result<BugPromptReport, String> {
    let selected = select_checks(audit, requested_rule, count)?;
    let prompt = if requested_rule.is_some() {
        build_targeted_prompt(audit, selected[0])
    } else {
        build_one_shot_prompt(audit, &selected)
    };

    Ok(BugPromptReport {
        target: audit.target.clone(),
        project: audit.project.clone(),
        score: audit.score,
        readiness: audit.readiness,
        minimum_score: audit.minimum_score,
        strong_count: audit.strong_count(),
        partial_count: audit.partial_count(),
        missing_count: audit.missing_count(),
        prompt_count: 1,
        prompts: vec![prompt],
    })
}

fn select_checks<'a>(
    audit: &'a AuditReport,
    requested_rule: Option<&str>,
    count: usize,
) -> Result<Vec<&'a AuditCheck>, String> {
    let mut candidates = audit
        .checks
        .iter()
        .filter(|check| check.status != CheckStatus::Strong)
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        right
            .blocking
            .cmp(&left.blocking)
            .then_with(|| right.gap().cmp(&left.gap()))
            .then_with(|| severity_score(right).cmp(&severity_score(left)))
            .then_with(|| left.label.cmp(right.label))
    });

    if let Some(rule) = requested_rule {
        let needle = rule.trim().to_lowercase();
        return candidates
            .into_iter()
            .find(|check| {
                check.id.eq_ignore_ascii_case(&needle)
                    || check.label.eq_ignore_ascii_case(&needle)
                    || check.id.to_lowercase().contains(&needle)
                    || check.label.to_lowercase().contains(&needle)
            })
            .map(|check| vec![check])
            .ok_or_else(|| format!("No matching bug rule found for `{rule}`."));
    }

    if candidates.is_empty() {
        return Err(String::from(
            "No promptable gaps were found. Every audited rule is already strong.",
        ));
    }

    let limit = if count == 0 { usize::MAX } else { count.max(1) };
    Ok(candidates.into_iter().take(limit).collect())
}

fn build_one_shot_prompt(audit: &AuditReport, selected: &[&AuditCheck]) -> GeneratedBugPrompt {
    let issues = selected
        .iter()
        .map(|check| build_issue(check, &audit.target, &audit.project))
        .collect::<Vec<_>>();
    let preserve = collect_preserve_signals(audit);
    let selected_rules = issues
        .iter()
        .map(|issue| issue.rule_id.to_owned())
        .collect::<Vec<_>>();
    let (automatic_count, guided_count, manual_count) = count_fixability(&issues);
    let issue_block = issues
        .iter()
        .enumerate()
        .map(|(index, issue)| format_issue_block(index + 1, issue))
        .collect::<Vec<_>>()
        .join("\n\n");
    let preserve_block = bullet_block(
        &preserve,
        "",
        "- Keep the currently strong signals intact while you fix the weak ones.",
    );
    let selection_note = if selected.len() == audit.partial_count() + audit.missing_count() {
        String::from("This prompt covers every currently non-strong rule in the repository audit.")
    } else {
        format!(
            "This prompt covers the highest-impact {} non-strong rule(s) from the current audit.",
            selected.len()
        )
    };
    let mission_line = if audit.score >= audit.minimum_score {
        format!(
            "Raise ossify readiness for `{}` above {}/100 ({}) by closing the remaining high-impact gaps.",
            audit.project.name,
            audit.score,
            audit.readiness.as_str()
        )
    } else {
        format!(
            "Raise ossify readiness for `{}` from {}/100 ({}) toward at least {}/100.",
            audit.project.name,
            audit.score,
            audit.readiness.as_str(),
            audit.minimum_score
        )
    };
    let prompt = format!(
        "You are taking one repository-wide bug-fix pass on `{target}`.\n\nMission\n- {mission_line}\n- Resolve the prioritized gaps below in one coherent change set instead of isolated micro-fixes.\n- Preserve working behavior, the project's product direction, and the strong signals already present.\n- {selection_note}\n\nRepository context\n- Project: {project}\n- Summary: {summary}\n- Current score: {score}/100\n- Current tier: {tier}\n- Gaps in scope: {issue_count}\n- Fixability mix: automatic {automatic_count}, guided {guided_count}, manual {manual_count}\n\nStrong signals to preserve\n{preserve_block}\n\nPrioritized issues to fix\n{issue_block}\n\nExecution rules\n- Prefer repository-specific fixes over generic boilerplate.\n- Keep the change set cohesive: combine related docs, config, workflow, and code updates when they solve the same issue cluster.\n- If a rule is automatic or guided, it is acceptable to add the missing file or config directly.\n- If a rule is manual, write concrete content tailored to this repository rather than placeholders.\n- If executable code or runtime behavior must change, keep the change minimal and add focused tests when the repo already has a test setup.\n- Do not refactor unrelated parts of the repository.\n\nDefinition of done\n- Re-run `ossify audit {target}` after the changes.\n- The selected rules should be materially improved or resolved: {rules}.\n- Summarize each changed file and map it to the rule or rules it fixes.\n- If one issue cannot be fully resolved without a product or maintainer decision, explain the exact blocker instead of guessing.\n",
        target = audit.target.display(),
        project = audit.project.name,
        summary = audit.project.summary(),
        score = audit.score,
        tier = audit.readiness.as_str(),
        mission_line = mission_line,
        issue_count = issues.len(),
        automatic_count = automatic_count,
        guided_count = guided_count,
        manual_count = manual_count,
        selection_note = selection_note,
        preserve_block = preserve_block,
        issue_block = issue_block,
        rules = selected_rules.join(", "),
    );

    GeneratedBugPrompt {
        strategy: PromptStrategy::OneShot,
        title: format!("One-Shot Fix Prompt for {}", audit.project.name),
        issue_count: issues.len(),
        selected_rules,
        automatic_count,
        guided_count,
        manual_count,
        preserve,
        issues,
        prompt,
    }
}

fn build_targeted_prompt(audit: &AuditReport, check: &AuditCheck) -> GeneratedBugPrompt {
    let issue = build_issue(check, &audit.target, &audit.project);
    let preserve = collect_preserve_signals(audit);
    let prompt = format!(
        "You are fixing one concrete repository issue in `{target}`.\n\nMission\n- Resolve the rule `{rule_id}` ({rule_label}) in a way that fits this repository's current direction.\n- Keep the fix focused and coherent, but do enough work that the issue is genuinely closed rather than cosmetically hidden.\n- Preserve the strong signals already present in the repository.\n\nRepository context\n- Project: {project}\n- Summary: {summary}\n- Current ossify score: {score}/100 ({tier})\n- Target rule: {rule_label} (`{rule_id}`)\n- Category: {category}\n- Current status: {status}\n- Fixability: {fixability}\n- Gap to close: {gap} points\n\nStrong signals to preserve\n{preserve_block}\n\nIssue to fix\n{issue_block}\n\nExecution rules\n- Make the smallest coherent change set that truly resolves this issue.\n- Prefer repository-specific content over generic scaffolding.\n- If the fix touches executable code or runtime behavior, add or update focused tests when the repository already has a test setup.\n- Do not refactor unrelated parts of the repo.\n\nDefinition of done\n- `ossify audit {target}` no longer reports `{rule_id}` as `{status}`.\n- The findings above are removed or materially reduced.\n- Any new docs, policies, or config files are specific to this repository.\n- Summarize exactly what changed and why it resolves the issue.\n",
        target = audit.target.display(),
        project = audit.project.name,
        summary = audit.project.summary(),
        score = audit.score,
        tier = audit.readiness.as_str(),
        rule_id = issue.rule_id,
        rule_label = issue.rule_label,
        category = issue.category.as_str(),
        status = issue.status.as_str(),
        fixability = issue.fixability.as_str(),
        gap = issue.gap,
        preserve_block = bullet_block(
            &preserve,
            "",
            "- Keep the currently strong signals intact while you fix this issue."
        ),
        issue_block = format_issue_block(1, &issue),
    );

    GeneratedBugPrompt {
        strategy: PromptStrategy::Targeted,
        title: format!("Fix {} in {}", issue.rule_label, audit.project.name),
        issue_count: 1,
        selected_rules: vec![issue.rule_id.to_owned()],
        automatic_count: usize::from(issue.fixability == Fixability::Automatic),
        guided_count: usize::from(issue.fixability == Fixability::Guided),
        manual_count: usize::from(issue.fixability == Fixability::Manual),
        preserve,
        issues: vec![issue],
        prompt,
    }
}

fn build_issue(check: &AuditCheck, target: &Path, project: &ProjectContext) -> PromptIssue {
    PromptIssue {
        rule_id: check.id,
        rule_label: check.label.to_owned(),
        category: check.category,
        status: check.status,
        fixability: check.fixability,
        gap: check.gap(),
        blocking: check.blocking,
        primary_cause: check.primary_cause.as_ref().map(format_cause),
        summary: check.message.clone(),
        hint: check.hint.to_owned(),
        findings: collect_findings(check),
        locations: collect_locations(check, target, project),
        evidence: collect_evidence(check),
    }
}

fn collect_preserve_signals(audit: &AuditReport) -> Vec<String> {
    let mut checks = audit.strong_checks().collect::<Vec<_>>();
    checks.sort_by(|left, right| {
        right
            .coverage
            .cmp(&left.coverage)
            .then_with(|| left.label.cmp(right.label))
    });
    checks
        .into_iter()
        .filter(|check| {
            if check.id != "funding" {
                return true;
            }
            let message = check.message.to_lowercase();
            !(message.contains("optional") || message.contains("not meaningful"))
        })
        .take(4)
        .map(|check| {
            let proof = check
                .proof
                .iter()
                .find(|item| {
                    matches!(
                        item.kind,
                        crate::intel::ProofKind::Satisfied | crate::intel::ProofKind::Historical
                    )
                })
                .map(|item| format!("{}: {}", item.expectation, item.detail))
                .unwrap_or_else(|| check.message.clone());
            format!("{} | {}", check.label, proof)
        })
        .collect()
}

fn collect_findings(check: &AuditCheck) -> Vec<String> {
    let findings = check
        .findings
        .iter()
        .take(4)
        .collect::<Vec<&AuditFinding>>();
    if findings.is_empty() {
        return vec![String::from(
            "No granular findings were emitted; rely on the rule summary, hint, and evidence.",
        )];
    }

    findings.into_iter().map(format_finding).collect()
}

fn collect_evidence(check: &AuditCheck) -> Vec<String> {
    if check.evidence.is_empty() {
        vec![String::from(
            "No direct evidence was captured beyond the rule summary.",
        )]
    } else {
        check.evidence.iter().take(6).cloned().collect()
    }
}

fn count_fixability(issues: &[PromptIssue]) -> (usize, usize, usize) {
    let automatic = issues
        .iter()
        .filter(|issue| issue.fixability == Fixability::Automatic)
        .count();
    let guided = issues
        .iter()
        .filter(|issue| issue.fixability == Fixability::Guided)
        .count();
    let manual = issues
        .iter()
        .filter(|issue| issue.fixability == Fixability::Manual)
        .count();
    (automatic, guided, manual)
}

fn format_issue_block(index: usize, issue: &PromptIssue) -> String {
    let header = format!("{}. {} (`{}`)", index, issue.rule_label, issue.rule_id);
    let details = vec![
        format!(
            "Status: {} | Category: {} | Fixability: {} | Gap: {}{}",
            issue.status.as_str(),
            issue.category.as_str(),
            issue.fixability.as_str(),
            issue.gap,
            if issue.blocking { " | Blocking" } else { "" }
        ),
        format!(
            "Primary cause: {}",
            issue
                .primary_cause
                .clone()
                .unwrap_or_else(|| String::from("No single cause was isolated; use the findings and evidence below."))
        ),
        format!("Rule summary: {}", issue.summary),
        format!("Hint: {}", issue.hint),
        String::from("Findings:"),
        bullet_block(
            &issue.findings,
            "  ",
            "- No granular findings were emitted for this issue.",
        ),
        String::from("Inspect first:"),
        bullet_block(
            &issue.locations,
            "  ",
            "- No exact file was pinned; inspect the repository root and the nearest related files.",
        ),
        String::from("Evidence captured during audit:"),
        bullet_block(
            &issue.evidence,
            "  ",
            "- No direct evidence was captured beyond the rule summary.",
        ),
    ];

    format!("{header}\n{}", indent_block(&details.join("\n"), "   "))
}

fn indent_block(input: &str, indent: &str) -> String {
    input
        .lines()
        .map(|line| format!("{indent}{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn bullet_block(items: &[String], indent: &str, fallback: &str) -> String {
    if items.is_empty() {
        return format!("{indent}{fallback}");
    }

    items
        .iter()
        .map(|item| format!("{indent}- {item}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_cause(cause: &crate::intel::RootCause) -> String {
    if cause.detail.trim().is_empty() {
        cause.title.clone()
    } else {
        format!("{}: {}", cause.title, cause.detail)
    }
}

fn format_finding(finding: &AuditFinding) -> String {
    let suffix = if finding.help.trim().is_empty() {
        String::new()
    } else {
        format!(" | {}", finding.help.trim())
    };
    format!(
        "[{}] {}{}",
        severity_tag(finding.severity),
        finding.message,
        suffix
    )
}

fn severity_score(check: &AuditCheck) -> u8 {
    check
        .findings
        .iter()
        .map(|finding| match finding.severity {
            FindingSeverity::Error => 3,
            FindingSeverity::Warning => 2,
            FindingSeverity::Info => 1,
        })
        .max()
        .unwrap_or(0)
}

fn severity_tag(severity: FindingSeverity) -> &'static str {
    match severity {
        FindingSeverity::Error => "error",
        FindingSeverity::Warning => "warning",
        FindingSeverity::Info => "info",
    }
}

fn collect_locations(check: &AuditCheck, target: &Path, project: &ProjectContext) -> Vec<String> {
    let mut locations = expected_locations(check.id, target, project);
    if let Some(path) = &check.location {
        push_location(&mut locations, path.display().to_string());
    }
    for finding in &check.findings {
        if let Some(path) = &finding.location {
            push_location(&mut locations, path.display().to_string());
        }
    }
    for context in &check.context_refs {
        let entry = if let Some(line_start) = context.line_start {
            format!(
                "{}:{}{}",
                context.path.display(),
                line_start,
                if context.approximate { " (approx)" } else { "" }
            )
        } else {
            context.path.display().to_string()
        };
        push_location(&mut locations, entry);
    }
    if locations.is_empty() {
        locations.push(target.display().to_string());
    }
    locations.truncate(8);
    locations
}

fn expected_locations(rule_id: &str, target: &Path, project: &ProjectContext) -> Vec<String> {
    let mut locations = Vec::new();
    let expected = |relative: &str| {
        if relative.is_empty() {
            target.display().to_string()
        } else {
            format!(
                "{}\\{} (expected)",
                target.display(),
                relative.replace('/', "\\")
            )
        }
    };

    match rule_id {
        "readme" => push_location(&mut locations, expected("README.md")),
        "license" => push_location(&mut locations, expected("LICENSE")),
        "contributing" => push_location(&mut locations, expected("CONTRIBUTING.md")),
        "code_of_conduct" => push_location(&mut locations, expected("CODE_OF_CONDUCT.md")),
        "security_policy" => push_location(&mut locations, expected("SECURITY.md")),
        "changelog" => push_location(&mut locations, expected("CHANGELOG.md")),
        "codeowners" => push_location(&mut locations, expected(".github/CODEOWNERS")),
        "funding" => push_location(&mut locations, expected(".github/FUNDING.yml")),
        "dependabot" => push_location(&mut locations, expected(".github/dependabot.yml")),
        "ci_workflow" => push_location(&mut locations, expected(".github/workflows/ci.yml")),
        "release_workflow" => {
            push_location(&mut locations, expected(".github/workflows/release.yml"))
        }
        "issue_templates" => {
            push_location(
                &mut locations,
                expected(".github/ISSUE_TEMPLATE/bug_report.md"),
            );
            push_location(
                &mut locations,
                expected(".github/ISSUE_TEMPLATE/feature_request.md"),
            );
        }
        "pull_request_template" => {
            push_location(&mut locations, expected(".github/PULL_REQUEST_TEMPLATE.md"))
        }
        "tests" => push_location(&mut locations, expected("tests")),
        _ => {}
    }

    if matches!(
        rule_id,
        "project_manifest" | "manifest_metadata" | "lint_and_format" | "tests" | "examples"
    ) {
        if let Some(manifest_path) = &project.manifest_path {
            push_location(&mut locations, manifest_path.display().to_string());
        }
    }
    if rule_id == "examples" {
        push_location(&mut locations, expected("examples"));
        push_location(&mut locations, expected("scripts"));
    }
    if rule_id == "lint_and_format" {
        push_location(&mut locations, expected(".github/workflows/ci.yml"));
    }

    locations
}

fn push_location(locations: &mut Vec<String>, entry: String) {
    let normalized = entry.replace('\\', "/").to_lowercase();
    if normalized.contains("/.git/")
        || normalized.contains("/.agents/")
        || normalized.contains("/.codex/")
        || normalized.contains("/node_modules/")
    {
        return;
    }
    if !locations.iter().any(|value| value == &entry) {
        locations.push(entry);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::audit_repository;
    use crate::config::OssifyConfig;
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
    fn prompt_defaults_to_one_shot_for_all_non_strong_rules() {
        let root = temp_repo("ossify-prompt-top-gap");
        fs::write(root.join("requirements.txt"), "flask\n").expect("write requirements");
        fs::write(root.join("app.py"), "print('hi')\n").expect("write app.py");

        let report = audit_repository(&root, &OssifyConfig::default()).expect("audit repository");
        let prompt = build_bug_prompt_report(&report, None, 0).expect("build prompt");

        assert_eq!(prompt.prompts.len(), 1);
        assert_eq!(prompt.prompts[0].strategy.as_str(), "one-shot");
        assert!(prompt.prompts[0].issue_count >= 2);
        assert!(prompt.prompts[0]
            .prompt
            .contains("Prioritized issues to fix"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn prompt_can_target_explicit_rule() {
        let root = temp_repo("ossify-prompt-explicit-rule");
        fs::write(root.join("requirements.txt"), "flask\n").expect("write requirements");
        fs::write(root.join("app.py"), "print('hi')\n").expect("write app.py");

        let report = audit_repository(&root, &OssifyConfig::default()).expect("audit repository");
        let prompt = build_bug_prompt_report(&report, Some("license"), 0).expect("build prompt");

        assert_eq!(prompt.prompts.len(), 1);
        assert_eq!(prompt.prompts[0].strategy.as_str(), "targeted");
        assert_eq!(
            prompt.prompts[0].selected_rules,
            vec![String::from("license")]
        );
        assert!(prompt.prompts[0].prompt.contains("Issue to fix"));

        let _ = fs::remove_dir_all(&root);
    }
}
