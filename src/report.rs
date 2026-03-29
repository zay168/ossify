use std::io;

use serde_json::json;

use crate::audit::AuditReport;
use crate::cli::OutputFormat;
use crate::clipboard::{copy_prompt_report, PROMPT_COPIED_MESSAGE, PROMPT_COPY_FAILED_PREFIX};
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

fn render_audit_json(report: &AuditReport) -> String {
    serde_json::to_string(&json!({
        "command": "audit",
        "target": &report.target,
        "project": &report.project,
        "readiness": report.readiness.as_str(),
        "score": report.score,
        "minimum_score": report.minimum_score,
        "strict_passed": report.strict_passed,
        "config_source": &report.config_source,
        "strong_count": report.strong_count(),
        "partial_count": report.partial_count(),
        "missing_count": report.missing_count(),
        "diagnostic_count": report.finding_count(),
        "category_scores": &report.category_scores,
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
