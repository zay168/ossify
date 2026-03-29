use crate::audit::{AuditCheck, AuditReport};
use crate::cli::OutputFormat;
use crate::generator::{FixReport, InitReport};

pub struct OutputOptions {
    pub format: OutputFormat,
    pub color: bool,
}

pub fn print_audit_report(report: &AuditReport, options: &OutputOptions) {
    match options.format {
        OutputFormat::Human => println!("{}", render_audit_human(report, options.color)),
        OutputFormat::Json => println!("{}", render_audit_json(report)),
    }
}

pub fn print_init_report(report: &InitReport, options: &OutputOptions) {
    match options.format {
        OutputFormat::Human => println!("{}", render_init_human(report, options.color)),
        OutputFormat::Json => println!("{}", render_init_json(report)),
    }
}

pub fn print_fix_report(report: &FixReport, options: &OutputOptions) {
    match options.format {
        OutputFormat::Human => println!("{}", render_fix_human(report, options.color)),
        OutputFormat::Json => println!("{}", render_fix_json(report)),
    }
}

fn render_audit_human(report: &AuditReport, color: bool) -> String {
    let style = Style::new(color);
    let mut lines = Vec::new();

    lines.push(style.badge("OSSIFY REPORT", 96, true));
    lines.push(format!("Target: {}", report.target.display()));
    lines.push(format!(
        "Open source readiness score: {}",
        style.score(report.score)
    ));
    lines.push(String::new());

    lines.push(style.section("Healthy"));
    for check in report.present_checks() {
        lines.push(format!(
            "  [{}] {} (+{})",
            style.good("ok"),
            check.label,
            check.weight
        ));
    }

    lines.push(String::new());
    lines.push(style.section("Missing or weak"));
    for check in report.missing_checks() {
        let fix_status = if check.fixable { "autofixable" } else { "manual" };
        lines.push(format!(
            "  [{}] {} (+{}, {})",
            style.bad(check.status.as_str()),
            check.label,
            check.weight,
            fix_status
        ));
        lines.push(format!("           {}", style.dim(check.hint)));
    }

    lines.push(String::new());
    lines.push(style.section("Next move"));
    lines.push(String::from(
        "  ossify fix . --license mit --owner \"Your Name\"",
    ));

    lines.join("\n")
}

fn render_init_human(report: &InitReport, color: bool) -> String {
    let style = Style::new(color);
    let mut lines = Vec::new();

    lines.push(style.badge("OSSIFY INIT", 94, true));
    lines.push(format!("Target: {}", report.target.display()));
    lines.push(format!("Mode: {}", style.emphasis(report.mode.as_str())));
    lines.push(String::new());

    for file in &report.files {
        let label = match file.action.as_str() {
            "created" => style.good("created"),
            _ => style.warn("skipped"),
        };
        lines.push(format!("  [{}] {}", label, file.path.display()));
    }

    lines.push(String::new());
    lines.push(style.section("Tip"));
    lines.push(format!(
        "  run `ossify audit {}` to see the new score",
        report.target.display()
    ));

    lines.join("\n")
}

fn render_fix_human(report: &FixReport, color: bool) -> String {
    let style = Style::new(color);
    let mut lines = Vec::new();
    let delta = report.after.score as i16 - report.before.score as i16;

    lines.push(style.badge("OSSIFY FIX", 92, true));
    lines.push(format!("Target: {}", report.target.display()));
    lines.push(format!(
        "Score: {} -> {} ({:+})",
        style.score(report.before.score),
        style.score(report.after.score),
        delta
    ));
    lines.push(String::new());

    if report.generated.files.is_empty() {
        lines.push(String::from("No files were changed."));
    } else {
        lines.push(style.section("Scaffolded files"));
        for file in &report.generated.files {
            let label = match file.action.as_str() {
                "created" => style.good("created"),
                _ => style.warn("skipped"),
            };
            lines.push(format!("  [{}] {}", label, file.path.display()));
        }
    }

    let remaining: Vec<&AuditCheck> = report.after.missing_checks().collect();
    if !remaining.is_empty() {
        lines.push(String::new());
        lines.push(style.section("Still missing"));
        for check in remaining {
            let label = if check.fixable {
                style.warn("review")
            } else {
                style.bad("manual")
            };
            lines.push(format!("  [{}] {}", label, check.label));
            lines.push(format!("           {}", style.dim(check.hint)));
        }
    } else {
        lines.push(String::new());
        lines.push(style.section("Result"));
        lines.push(style.good("Repository now looks open-source ready."));
    }

    lines.join("\n")
}

fn render_audit_json(report: &AuditReport) -> String {
    json_object(&[
        field("command", json_string("audit")),
        field("target", json_string(&report.target.display().to_string())),
        field("score", report.score.to_string()),
        field("present_count", report.present_count().to_string()),
        field("missing_count", report.missing_count().to_string()),
        field(
            "checks",
            json_array(
                report
                    .checks
                    .iter()
                    .map(render_check_json)
                    .collect::<Vec<String>>(),
            ),
        ),
        field(
            "next_step",
            json_string("ossify fix . --license mit --owner \"Your Name\""),
        ),
    ])
}

fn render_init_json(report: &InitReport) -> String {
    json_object(&[
        field("command", json_string(report.mode.as_str())),
        field("target", json_string(&report.target.display().to_string())),
        field("file_count", report.files.len().to_string()),
        field(
            "files",
            json_array(
                report
                    .files
                    .iter()
                    .map(|file| {
                        json_object(&[
                            field("path", json_string(&file.path.display().to_string())),
                            field("action", json_string(file.action.as_str())),
                        ])
                    })
                    .collect::<Vec<String>>(),
            ),
        ),
    ])
}

fn render_fix_json(report: &FixReport) -> String {
    json_object(&[
        field("command", json_string("fix")),
        field("target", json_string(&report.target.display().to_string())),
        field("before_score", report.before.score.to_string()),
        field("after_score", report.after.score.to_string()),
        field(
            "score_delta",
            (report.after.score as i16 - report.before.score as i16).to_string(),
        ),
        field("generated", render_init_json(&report.generated)),
        field("before", render_audit_json(&report.before)),
        field("after", render_audit_json(&report.after)),
    ])
}

fn render_check_json(check: &AuditCheck) -> String {
    json_object(&[
        field("id", json_string(check.id)),
        field("label", json_string(check.label)),
        field("weight", check.weight.to_string()),
        field("status", json_string(check.status.as_str())),
        field("fixable", json_bool(check.fixable)),
        field("hint", json_string(check.hint)),
    ])
}

fn field(key: &str, value: String) -> (String, String) {
    (key.to_owned(), value)
}

fn json_object(fields: &[(String, String)]) -> String {
    let parts: Vec<String> = fields
        .iter()
        .map(|(key, value)| format!("{}:{}", json_string(key), value))
        .collect();
    format!("{{{}}}", parts.join(","))
}

fn json_array(items: Vec<String>) -> String {
    format!("[{}]", items.join(","))
}

fn json_bool(value: bool) -> String {
    if value {
        String::from("true")
    } else {
        String::from("false")
    }
}

fn json_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');

    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            control if control.is_control() => {
                escaped.push_str(&format!("\\u{:04x}", control as u32));
            }
            other => escaped.push(other),
        }
    }

    escaped.push('"');
    escaped
}

struct Style {
    enabled: bool,
}

impl Style {
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

    fn section(&self, text: &str) -> String {
        self.paint("1;97", text)
    }

    fn emphasis(&self, text: &str) -> String {
        self.paint("1;96", text)
    }

    fn good(&self, text: &str) -> String {
        self.paint("92", text)
    }

    fn warn(&self, text: &str) -> String {
        self.paint("93", text)
    }

    fn bad(&self, text: &str) -> String {
        self.paint("91", text)
    }

    fn dim(&self, text: &str) -> String {
        self.paint("2", text)
    }

    fn score(&self, score: u8) -> String {
        if score >= 85 {
            self.good(&format!("{score}/100"))
        } else if score >= 60 {
            self.warn(&format!("{score}/100"))
        } else {
            self.bad(&format!("{score}/100"))
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
