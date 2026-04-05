use crossterm::terminal;
use unicode_width::UnicodeWidthStr;

use crate::audit::{CheckStatus, FindingSeverity, ReadinessTier};
use crate::generator::{FileAction, InitReport};
use crate::prompt::{BugPromptReport, PromptIssue};

use super::model::{UiMode, UiReport};

const MIN_WIDTH: usize = 72;
const DEFAULT_WIDTH: usize = 100;
const TERMINAL_EDGE_GUTTER: usize = 1;
const CATEGORY_BAR_WIDTH: usize = 18;

/// Remove the Windows extended-length path prefix (`\\?\`) wherever it appears.
fn clean_path(raw: &str) -> String {
    raw.replace(r"\\?\", "")
}

/// Replace the absolute target directory prefix with a relative filename.
///
/// "project manifest exists: Detected manifest at C:\path\Cargo.toml"
/// → "project manifest exists: Detected manifest at Cargo.toml"

#[allow(clippy::empty_line_after_doc_comments)]
pub fn render_audit(model: &UiReport, color: bool) -> String {
    render_ui_report(model, color, current_terminal_width())
}

pub fn render_fix(model: &UiReport, color: bool) -> String {
    render_ui_report(model, color, current_terminal_width())
}

pub fn render_plan(model: &UiReport, color: bool) -> String {
    render_ui_report(model, color, current_terminal_width())
}

pub fn render_prompt(report: &BugPromptReport, color: bool) -> String {
    let style = Style::new(color);
    let width = current_terminal_width().max(MIN_WIDTH);
    let wide = width >= 112;
    let Some(prompt) = report.prompts.first() else {
        return box_section(
            width,
            &style.badge("OSSIFY PROMPT", 94, true),
            &[String::from(
                "No bug prompt could be generated for this repository.",
            )],
        );
    };

    let hero = box_section(
        width,
        &style.badge("OSSIFY PROMPT", 94, true),
        &[
            format!(
                "Project: {} ({})",
                report.project.name,
                report.project.summary()
            ),
            format!("Target: {}", report.target.display()),
            format!(
                "Score: {}   Tier: {}",
                style.score(report.score),
                style.tier(report.readiness)
            ),
            format!(
                "Prompt style: {}   Issues in scope: {}",
                style.accent(prompt.strategy.as_str()),
                prompt.issue_count
            ),
        ],
    );

    let scope_lines = vec![
        format!(
            "Fixability mix: automatic {} | guided {} | manual {}",
            prompt.automatic_count, prompt.guided_count, prompt.manual_count
        ),
        format!(
            "Selected rules: {}",
            if prompt.selected_rules.is_empty() {
                String::from("none")
            } else {
                prompt.selected_rules.join(", ")
            }
        ),
        format!(
            "Signal mix: {} strong | {} partial | {} missing",
            report.strong_count, report.partial_count, report.missing_count
        ),
        String::from("Copy the full prompt below into Codex, Cursor, Claude, or a teammate issue."),
    ];
    let issue_lines = prompt
        .issues
        .iter()
        .take(6)
        .map(render_prompt_issue_line)
        .collect::<Vec<_>>();
    let preserve_lines = if prompt.preserve.is_empty() {
        vec![String::from(
            "No strong signals were captured; focus on the gaps in scope.",
        )]
    } else {
        prompt.preserve.clone()
    };
    let prompt_lines = prompt.prompt.lines().map(str::to_owned).collect::<Vec<_>>();

    let mut sections = vec![hero];
    if !wide {
        sections.push(box_section(width, "Prompt Scope", &scope_lines));
        sections.push(box_section(width, "Issues In Scope", &issue_lines));
        sections.push(box_section(width, "Protect These Signals", &preserve_lines));
        sections.push(box_section_verbatim(
            width,
            "Copy/Paste Prompt",
            &prompt_lines,
        ));
        return sections.join("\n\n");
    }

    let left = stack_sections(vec![
        box_section(width / 2 - 1, "Prompt Scope", &scope_lines),
        box_section(width / 2 - 1, "Protect These Signals", &preserve_lines),
    ]);
    let right = stack_sections(vec![box_section(
        width / 2 - 1,
        "Issues In Scope",
        &issue_lines,
    )]);
    sections.push(join_columns(&left, &right, width / 2 - 1, width / 2 - 1));
    sections.push(box_section_verbatim(
        width,
        "Copy/Paste Prompt",
        &prompt_lines,
    ));
    sections.join("\n\n")
}

pub fn render_init(report: &InitReport, color: bool) -> String {
    let style = Style::new(color);
    let width = current_terminal_width();
    let mut sections = Vec::new();

    sections.push(box_section(
        width,
        &style.badge("OSSIFY INIT", 94, true),
        &[
            format!(
                "Project: {} ({})",
                report.project.name,
                report.project.summary()
            ),
            format!("Target: {}", report.target.display()),
            format!("Mode: {}", report.mode.as_str()),
        ],
    ));

    let file_lines = if report.files.is_empty() {
        vec![String::from("No files were generated.")]
    } else {
        report
            .files
            .iter()
            .map(|file| {
                let label = match file.action {
                    FileAction::Created => style.good("created"),
                    FileAction::Updated => style.good("updated"),
                    FileAction::Skipped => style.warn("skipped"),
                };
                format!("[{}] {}", label, file.path.display())
            })
            .collect::<Vec<_>>()
    };
    sections.push(box_section(width, "Files", &file_lines));
    sections.push(box_section(
        width,
        "Next Move",
        &[format!(
            "Run `ossify audit {}` to inspect the result.",
            report.target.display()
        )],
    ));

    sections.join("\n\n")
}

pub fn render_ui_report(model: &UiReport, color: bool, width: usize) -> String {
    let style = Style::new(color);
    let width = width.max(MIN_WIDTH);
    let wide = width >= 112;

    let hero = render_hero(model, &style, width);
    let category_label_width = model
        .categories
        .iter()
        .map(|category| display_width(category.label))
        .max()
        .unwrap_or(10)
        .max(10);
    let category_lines = model
        .categories
        .iter()
        .map(|category| render_category_line(category, category_label_width))
        .collect::<Vec<_>>();
    let domain_label_width = model
        .domains
        .iter()
        .map(|domain| display_width(domain.label))
        .max()
        .unwrap_or(8)
        .max(8);
    let domain_lines = model
        .domains
        .iter()
        .flat_map(|domain| render_domain_lines(domain, domain_label_width))
        .collect::<Vec<_>>();
    let strengths = top_strengths(model);
    let gaps = top_gaps(model);
    let diagnostics = top_diagnostics(model);
    let files = file_sections(model, &style);
    let next_moves = model.next_moves.clone();

    if !wide {
        let mut parts = vec![hero];
        if !domain_lines.is_empty() {
            parts.push(box_section_preformatted(
                width,
                "Domain Scores",
                &domain_lines,
            ));
        }
        parts.push(box_section_preformatted(
            width,
            "Category Scores",
            &category_lines,
        ));
        if !strengths.is_empty() {
            parts.push(box_section(width, "Top Strengths", &strengths));
        }
        if !gaps.is_empty() {
            parts.push(box_section(width, section_title_for_gaps(model), &gaps));
        }
        if !diagnostics.is_empty() {
            parts.push(box_section(width, "Diagnostics", &diagnostics));
        }
        for (title, lines) in files {
            parts.push(box_section(width, title, &lines));
        }
        parts.push(box_section(width, "Next Move", &next_moves));
        return parts.join("\n\n");
    }

    let gap_title = section_title_for_gaps(model);
    let left_sections = {
        let mut sections = Vec::new();
        if !domain_lines.is_empty() {
            sections.push(box_section_preformatted(
                width / 2 - 1,
                "Domain Scores",
                &domain_lines,
            ));
        }
        sections.push(box_section_preformatted(
            width / 2 - 1,
            "Category Scores",
            &category_lines,
        ));
        if !strengths.is_empty() {
            sections.push(box_section(width / 2 - 1, "Top Strengths", &strengths));
        }
        for (title, lines) in &files {
            sections.push(box_section(width / 2 - 1, title, lines));
        }
        sections
    };

    let right_sections = {
        let mut sections = Vec::new();
        if !gaps.is_empty() {
            sections.push(box_section(width / 2 - 1, gap_title, &gaps));
        }
        if !diagnostics.is_empty() {
            sections.push(box_section(width / 2 - 1, "Diagnostics", &diagnostics));
        }
        sections.push(box_section(width / 2 - 1, "Next Move", &next_moves));
        sections
    };

    let left = stack_sections(left_sections);
    let right = stack_sections(right_sections);

    format!(
        "{}\n\n{}",
        hero,
        join_columns(&left, &right, width / 2 - 1, width / 2 - 1)
    )
}

fn render_hero(model: &UiReport, style: &Style, width: usize) -> String {
    let current = &model.current;
    // Strip the manifest path from the project summary ("Rust cli via /path" → "Rust cli")
    let raw_summary = clean_path(&model.project_summary);
    let summary = raw_summary.split(" via ").next().unwrap_or(&raw_summary);
    let mut lines = vec![format!("{}  ·  {}", model.project_name, summary)];
    if let Some(previous) = model.previous {
        let delta = current.score as i16 - previous.score as i16;
        let label = if model.mode == UiMode::Plan {
            "estimated"
        } else {
            "score"
        };
        lines.push(format!(
            "{}  {}  →  {}  ({:+})",
            style.dim(label),
            style.score(previous.score),
            style.score(current.score),
            delta
        ));
        lines.push(format!(
            "{}  {}  →  {}",
            style.dim("tier"),
            style.tier(previous.readiness),
            style.tier(current.readiness)
        ));
    } else {
        lines.push(format!(
            "{}  {}   {}  {}   {}  {}  (≥{})",
            style.dim("score"),
            style.score(current.score),
            style.dim("tier"),
            style.tier(current.readiness),
            style.dim("strict"),
            if current.strict_passed {
                style.good("pass")
            } else {
                style.bad("fail")
            },
            current.minimum_score
        ));
    }
    lines.push(meter(current.score, 30));

    box_section(
        width,
        &style.badge(model.title, accent_for_mode(model.mode), true),
        &lines,
    )
}

fn top_strengths(model: &UiReport) -> Vec<String> {
    let mut checks = model
        .checks
        .iter()
        .filter(|check| check.status == CheckStatus::Strong)
        .filter(|check| {
            !(check.id == "funding"
                && (check
                    .strongest_proof
                    .as_ref()
                    .map(|proof| {
                        let proof = proof.to_lowercase();
                        proof.contains("optional") || proof.contains("not meaningful")
                    })
                    .unwrap_or(false)
                    || {
                        let message = check.message.to_lowercase();
                        message.contains("optional") || message.contains("not meaningful")
                    }))
        })
        .collect::<Vec<_>>();
    checks.sort_by_key(|check| std::cmp::Reverse(check.coverage));
    let label_w = checks
        .iter()
        .map(|c| display_width(&c.label))
        .max()
        .unwrap_or(0);
    checks
        .into_iter()
        .take(4)
        .map(|check| {
            let label = pad_to_width(&check.label, label_w);
            format!("{}  {}%", label, check.coverage)
        })
        .collect()
}

fn render_category_line(category: &super::model::UiCategoryScore, label_width: usize) -> String {
    let label = pad_to_width(category.label, label_width);
    format!(
        "{}  {}  {:>3}%  {}/{}",
        label,
        meter(category.score, CATEGORY_BAR_WIDTH),
        category.score,
        category.earned,
        category.total
    )
}

fn render_domain_lines(domain: &super::model::UiDomainScore, label_width: usize) -> Vec<String> {
    let label = pad_to_width(domain.label, label_width);
    let score = domain
        .score
        .map(|value| format!("{value:>3}%"))
        .unwrap_or_else(|| String::from(" n/a"));
    let cap_tag = domain
        .cap
        .map(|value| format!("  cap {value:>3}"))
        .unwrap_or_default();
    // Score + cap only — no engine/summary so this line never wraps.
    // wrap_text splits on whitespace and collapses spaces, which would
    // break column alignment for longer engine strings.
    let mut result = vec![format!("{}  {}{}", label, score, cap_tag)];
    if let Some(reason) = &domain.cap_reason {
        result.push(format!("  cap  {}", compact_cap_reason(reason)));
    }
    result
}

fn top_gaps(model: &UiReport) -> Vec<String> {
    let mut checks = model
        .checks
        .iter()
        .filter(|check| check.status != CheckStatus::Strong)
        .collect::<Vec<_>>();
    checks.sort_by_key(|check| std::cmp::Reverse(check.gap));
    checks
        .into_iter()
        .take(5)
        .map(|check| {
            let cause = check
                .primary_cause
                .clone()
                .or_else(|| check.strongest_contradiction.clone())
                .unwrap_or_else(|| check.message.clone());
            format!(
                "{} | {} | {} | -{}",
                status_label(check.status),
                check.label,
                cause,
                check.gap
            )
        })
        .collect()
}

fn top_diagnostics(model: &UiReport) -> Vec<String> {
    let mut diagnostics = model.diagnostics.clone();
    diagnostics.sort_by(|left, right| {
        right
            .impact
            .total_cmp(&left.impact)
            .then_with(|| left.severity.rank().cmp(&right.severity.rank()))
            .then_with(|| left.rule_label.cmp(&right.rule_label))
    });
    diagnostics
        .into_iter()
        .take(5)
        .map(|diagnostic| {
            let location = diagnostic
                .location
                .map(|location| format!(" @ {}", location))
                .unwrap_or_default();
            let cause = diagnostic
                .primary_cause
                .map(|cause| format!(" | cause: {cause}"))
                .unwrap_or_default();
            format!(
                "{} | {} | {}{}{}",
                severity_label(diagnostic.severity),
                diagnostic.rule_label,
                diagnostic.message,
                cause,
                location
            )
        })
        .collect()
}

fn file_sections(model: &UiReport, style: &Style) -> Vec<(&'static str, Vec<String>)> {
    if model.files.is_empty() {
        return Vec::new();
    }

    let actionable = model
        .files
        .iter()
        .filter(|file| matches!(file.action, FileAction::Created | FileAction::Updated))
        .map(|file| {
            let label = match file.action {
                FileAction::Created => style.good("created"),
                FileAction::Updated => style.good("updated"),
                FileAction::Skipped => style.warn("skipped"),
            };
            format!("[{}] {}", label, file.path)
        })
        .collect::<Vec<_>>();
    let skipped = model
        .files
        .iter()
        .filter(|file| matches!(file.action, FileAction::Skipped))
        .map(|file| match &file.reason {
            Some(reason) => format!("[skipped] {} | {}", file.path, reason),
            None => format!("[skipped] {}", file.path),
        })
        .collect::<Vec<_>>();

    let mut sections = Vec::new();
    if !actionable.is_empty() {
        sections.push((file_title(model.mode), actionable));
    }
    if !skipped.is_empty() {
        sections.push(("Blocked or Skipped", skipped));
    }
    sections
}

fn file_title(mode: UiMode) -> &'static str {
    match mode {
        UiMode::Fix => "Scaffolded Files",
        UiMode::Plan => "Would Scaffold Files",
        UiMode::Audit => "Files",
    }
}

fn section_title_for_gaps(modeled: &UiReport) -> &'static str {
    match modeled.mode {
        UiMode::Plan => "Still Manual After Plan",
        _ => "Top Gaps",
    }
}

fn box_section(width: usize, title: &str, raw_lines: &[String]) -> String {
    let inner = width.saturating_sub(4).max(12);
    let mut lines = vec![format!(
        "┌{}┐",
        section_header(title, width.saturating_sub(2))
    )];

    for raw in raw_lines {
        let wrapped = wrap_text(raw, inner);
        for line in wrapped {
            let pad = inner.saturating_sub(display_width(&line));
            lines.push(format!("│ {}{} │", line, " ".repeat(pad)));
        }
    }

    lines.push(format!("└{}┘", "─".repeat(width.saturating_sub(2))));
    lines.join("\n")
}

fn box_section_preformatted(width: usize, title: &str, raw_lines: &[String]) -> String {
    box_section(width, title, raw_lines)
}

fn box_section_verbatim(width: usize, title: &str, raw_lines: &[String]) -> String {
    let inner = width.saturating_sub(4).max(12);
    let mut lines = vec![format!(
        "┌{}┐",
        section_header(title, width.saturating_sub(2))
    )];

    for raw in raw_lines {
        let expanded = raw.replace('\t', "    ");
        for line in wrap_verbatim(&expanded, inner) {
            let pad = inner.saturating_sub(display_width(&line));
            lines.push(format!("│ {}{} │", line, " ".repeat(pad)));
        }
    }

    lines.push(format!("└{}┘", "─".repeat(width.saturating_sub(2))));
    lines.join("\n")
}

fn section_header(title: &str, width: usize) -> String {
    let text = format!("─ {} ", title);
    let text_width = display_width(&text);
    if text_width >= width {
        truncate_to_width(&text, width)
    } else {
        format!("{text}{}", "─".repeat(width - text_width))
    }
}

fn stack_sections(sections: Vec<String>) -> Vec<String> {
    let mut lines = Vec::new();
    for (index, section) in sections.into_iter().enumerate() {
        if index > 0 {
            lines.push(String::new());
        }
        lines.extend(section.lines().map(str::to_owned));
    }
    lines
}

fn join_columns(
    left: &[String],
    right: &[String],
    left_width: usize,
    right_width: usize,
) -> String {
    let height = left.len().max(right.len());
    let mut out = Vec::with_capacity(height);
    for row in 0..height {
        let left_line = left
            .get(row)
            .cloned()
            .unwrap_or_else(|| " ".repeat(left_width));
        let right_line = right
            .get(row)
            .cloned()
            .unwrap_or_else(|| " ".repeat(right_width));
        out.push(format!(
            "{}  {}",
            pad_to_width(&left_line, left_width),
            pad_to_width(&right_line, right_width)
        ));
    }
    out.join("\n")
}

/// Condense a cap reason into a short, scannable string.
///
/// "unmaintained advisory RUSTSEC-2024-0436 for paste 1.0.15 capped …"
/// → "RUSTSEC-2024-0436 · paste 1.0.15"
fn compact_cap_reason(reason: &str) -> String {
    let words: Vec<&str> = reason.split_whitespace().collect();
    let advisory = words
        .iter()
        .find(|w| w.starts_with("RUSTSEC-") || w.starts_with("CVE-") || w.starts_with("GHSA-"))
        .copied();
    let for_idx = words.iter().position(|w| *w == "for");
    match (advisory, for_idx) {
        (Some(id), Some(i)) if i + 2 < words.len() => {
            format!("{id}  ·  {} {}", words[i + 1], words[i + 2])
        }
        (Some(id), Some(i)) if i + 1 < words.len() => {
            format!("{id}  ·  {}", words[i + 1])
        }
        (Some(id), _) => id.to_owned(),
        _ => {
            let joined = words.join(" ");
            if joined.len() > 50 {
                format!("{}…", &joined[..47])
            } else {
                joined
            }
        }
    }
}

fn meter(score: u8, width: usize) -> String {
    let width = width.max(1);
    let filled = ((score as usize * width) + 50) / 100;
    format!("{}{}", "█".repeat(filled), "░".repeat(width - filled))
}

fn wrap_text(input: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![String::new()];
    }
    if display_width(input) <= width {
        return vec![input.to_owned()];
    }
    let mut lines = Vec::new();
    let mut current = String::new();

    for word in input.split_whitespace() {
        let candidate = if current.is_empty() {
            word.to_owned()
        } else {
            format!("{current} {word}")
        };
        if display_width(&candidate) <= width {
            current = candidate;
        } else {
            if !current.is_empty() {
                lines.push(current);
            }
            if display_width(word) > width {
                lines.push(truncate_to_width(word, width));
                current = String::new();
            } else {
                current = word.to_owned();
            }
        }
    }

    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn wrap_verbatim(input: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![String::new()];
    }
    if input.is_empty() {
        return vec![String::new()];
    }
    if display_width(input) <= width {
        return vec![input.to_owned()];
    }

    let mut lines = Vec::new();
    let mut current = String::new();
    for ch in input.chars() {
        let candidate = format!("{current}{ch}");
        if !current.is_empty() && display_width(&candidate) > width {
            lines.push(current);
            current = String::new();
        }
        current.push(ch);
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

fn pad_to_width(input: &str, width: usize) -> String {
    let pad = width.saturating_sub(display_width(input));
    format!("{input}{}", " ".repeat(pad))
}

fn truncate_to_width(input: &str, width: usize) -> String {
    let mut out = String::new();
    for ch in input.chars() {
        let candidate = format!("{out}{ch}");
        if display_width(&candidate) > width {
            break;
        }
        out.push(ch);
    }
    out
}

fn display_width(input: &str) -> usize {
    UnicodeWidthStr::width(strip_ansi(input).as_str())
}

fn current_terminal_width() -> usize {
    terminal::size()
        .map(|(width, _)| width as usize)
        .unwrap_or(DEFAULT_WIDTH)
        .saturating_sub(TERMINAL_EDGE_GUTTER)
        .max(MIN_WIDTH)
}

fn render_prompt_issue_line(issue: &PromptIssue) -> String {
    format!(
        "{} | {} | {} | {} | gap {}{}",
        status_label(issue.status),
        issue.rule_label,
        issue.category.as_str(),
        issue.fixability.as_str(),
        issue.gap,
        if issue.blocking { " | blocking" } else { "" }
    )
}

fn strip_ansi(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' && matches!(chars.peek(), Some('[')) {
            let _ = chars.next();
            for next in chars.by_ref() {
                if ('@'..='~').contains(&next) {
                    break;
                }
            }
        } else {
            out.push(ch);
        }
    }

    out
}

fn accent_for_mode(mode: UiMode) -> u8 {
    match mode {
        UiMode::Audit => 96,
        UiMode::Fix => 92,
        UiMode::Plan => 95,
    }
}

fn status_label(status: CheckStatus) -> &'static str {
    match status {
        CheckStatus::Strong => "strong",
        CheckStatus::Partial => "partial",
        CheckStatus::Missing => "missing",
    }
}

fn severity_label(severity: FindingSeverity) -> &'static str {
    match severity {
        FindingSeverity::Error => "error",
        FindingSeverity::Warning => "warning",
        FindingSeverity::Info => "info",
    }
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

    fn good(&self, text: &str) -> String {
        self.paint("92", text)
    }

    fn dim(&self, text: &str) -> String {
        self.paint("90", text)
    }

    fn accent(&self, text: &str) -> String {
        self.paint("96", text)
    }

    fn warn(&self, text: &str) -> String {
        self.paint("93", text)
    }

    fn bad(&self, text: &str) -> String {
        self.paint("91", text)
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

    fn tier(&self, tier: ReadinessTier) -> String {
        match tier {
            ReadinessTier::LaunchReady => self.good(tier.as_str()),
            ReadinessTier::Promising => self.warn(tier.as_str()),
            ReadinessTier::Rough => self.bad(tier.as_str()),
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
    use crate::config::OssifyConfig;
    use crate::generator::{fix_repository, plan_fix_repository, InitOptions, LicenseKind};
    use crate::prompt::build_bug_prompt_report;
    use crate::ui::model::UiReport;
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

    fn sample_audit_model() -> UiReport {
        let root = temp_repo("ossify-ui-audit-render");
        fs::create_dir_all(root.join("src")).expect("create src");
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n",
        )
        .expect("write Cargo.toml");
        fs::write(root.join("src/main.rs"), "fn main() {}\n").expect("write main");
        let report =
            crate::audit::audit_repository(&root, &OssifyConfig::default()).expect("audit");
        let model = UiReport::from_audit(&report);
        let _ = fs::remove_dir_all(&root);
        model
    }

    #[test]
    fn audit_render_is_width_aware_at_80() {
        let rendered = render_ui_report(&sample_audit_model(), false, 80);
        assert!(rendered.contains("OSSIFY REPORT"));
        assert!(rendered.contains("Category Scores"));
        assert!(rendered.lines().all(|line| display_width(line) <= 80));
    }

    #[test]
    fn audit_render_uses_columns_at_120() {
        let rendered = render_ui_report(&sample_audit_model(), false, 120);
        assert!(rendered.contains("Top Strengths"));
        assert!(rendered.contains("Top Gaps"));
        assert!(rendered.lines().all(|line| display_width(line) <= 120));
    }

    #[test]
    fn category_gauges_share_the_same_start_column() {
        let rendered = render_ui_report(&sample_audit_model(), false, 100);
        let positions = rendered
            .lines()
            .filter(|line| {
                line.contains("identity")
                    || line.contains("docs")
                    || line.contains("community")
                    || line.contains("automation")
                    || line.contains("release")
            })
            .filter_map(|line| line.find('█').or_else(|| line.find('░')))
            .collect::<Vec<_>>();

        assert_eq!(positions.len(), 5);
        assert!(positions.windows(2).all(|pair| pair[0] == pair[1]));
    }

    #[test]
    fn color_render_stays_within_visible_width() {
        let rendered = render_ui_report(&sample_audit_model(), true, 100);
        assert!(rendered.lines().all(|line| display_width(line) <= 100));
    }

    #[test]
    fn optional_funding_does_not_show_up_in_top_strengths() {
        let model = sample_audit_model();
        let strengths = top_strengths(&model);
        assert!(!strengths.iter().any(|line| line.contains("Funding file")));
    }

    #[test]
    fn plan_render_contains_blocked_section() {
        let root = temp_repo("ossify-ui-plan-render");
        fs::create_dir_all(root.join("src")).expect("create src");
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n",
        )
        .expect("write Cargo.toml");
        fs::write(root.join("src/main.rs"), "fn main() {}\n").expect("write main");
        let plan = plan_fix_repository(
            &root,
            &InitOptions {
                overwrite: false,
                license: LicenseKind::Mit,
                owner: String::from("Open Source Maintainers"),
                funding: None,
            },
            &OssifyConfig::default(),
        )
        .expect("plan");
        let rendered = render_ui_report(&UiReport::from_plan(&plan), false, 120);
        assert!(rendered.contains("Would Scaffold Files"));
        assert!(rendered.contains("Blocked or Skipped"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn fix_render_contains_scaffolded_files() {
        let root = temp_repo("ossify-ui-fix-render");
        fs::create_dir_all(root.join("src")).expect("create src");
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n",
        )
        .expect("write Cargo.toml");
        fs::write(root.join("src/main.rs"), "fn main() {}\n").expect("write main");
        let fix = fix_repository(
            &root,
            &InitOptions {
                overwrite: false,
                license: LicenseKind::Mit,
                owner: String::from("@acme"),
                funding: Some(String::from("github:acme")),
            },
            &OssifyConfig::default(),
        )
        .expect("fix");
        let rendered = render_ui_report(&UiReport::from_fix(&fix), false, 120);
        assert!(rendered.contains("Scaffolded Files"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn prompt_render_contains_copy_paste_box() {
        let root = temp_repo("ossify-ui-prompt-render");
        fs::write(root.join("requirements.txt"), "flask\n").expect("write requirements");
        fs::write(root.join("app.py"), "print('hi')\n").expect("write app.py");
        let audit =
            crate::audit::audit_repository(&root, &OssifyConfig::default()).expect("audit repo");
        let prompt = build_bug_prompt_report(&audit, None, 0).expect("bug prompt");
        let rendered = render_prompt(&prompt, false);
        assert!(rendered.contains("OSSIFY PROMPT"));
        assert!(rendered.contains("Copy/Paste Prompt"));
        assert!(rendered.contains("Prioritized issues to fix"));
        let _ = fs::remove_dir_all(&root);
    }
}
