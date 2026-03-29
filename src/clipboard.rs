use crate::prompt::BugPromptReport;

pub const PROMPT_COPIED_MESSAGE: &str = "Prompt copied automatically to clipboard.";
pub const PROMPT_COPY_FAILED_PREFIX: &str = "Automatic clipboard copy unavailable:";

pub fn copy_prompt_report(report: &BugPromptReport) -> Result<(), String> {
    let text = prompt_report_text(report);
    let mut clipboard =
        arboard::Clipboard::new().map_err(|error| format!("clipboard unavailable: {error}"))?;
    clipboard
        .set_text(text)
        .map_err(|error| format!("clipboard write failed: {error}"))
}

pub fn prompt_report_text(report: &BugPromptReport) -> String {
    report
        .prompts
        .iter()
        .map(|prompt| prompt.prompt.clone())
        .collect::<Vec<_>>()
        .join("\n\n---\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::{CheckStatus, ReadinessTier, RuleCategory};
    use crate::project::{ProjectContext, ProjectKind, ProjectMetadata, RepoProfile};
    use crate::prompt::{BugPromptReport, GeneratedBugPrompt, PromptIssue, PromptStrategy};
    use std::path::PathBuf;

    #[test]
    fn prompt_report_text_joins_prompt_bodies() {
        let report = BugPromptReport {
            target: PathBuf::from("."),
            project: ProjectContext {
                kind: ProjectKind::Rust,
                profile: RepoProfile::Cli,
                name: String::from("ossify"),
                manifest_path: None,
                metadata: ProjectMetadata::default(),
            },
            score: 50,
            readiness: ReadinessTier::Promising,
            minimum_score: 85,
            strong_count: 1,
            partial_count: 1,
            missing_count: 1,
            prompt_count: 2,
            prompts: vec![
                GeneratedBugPrompt {
                    strategy: PromptStrategy::Targeted,
                    title: String::from("One"),
                    issue_count: 1,
                    selected_rules: vec![String::from("readme")],
                    automatic_count: 1,
                    guided_count: 0,
                    manual_count: 0,
                    preserve: Vec::new(),
                    issues: vec![PromptIssue {
                        rule_id: "readme",
                        rule_label: String::from("README"),
                        category: RuleCategory::Docs,
                        status: CheckStatus::Missing,
                        fixability: crate::audit::Fixability::Automatic,
                        gap: 10,
                        blocking: false,
                        primary_cause: None,
                        summary: String::from("Missing"),
                        hint: String::from("Add it"),
                        findings: Vec::new(),
                        locations: Vec::new(),
                        evidence: Vec::new(),
                    }],
                    prompt: String::from("first prompt"),
                },
                GeneratedBugPrompt {
                    strategy: PromptStrategy::Targeted,
                    title: String::from("Two"),
                    issue_count: 1,
                    selected_rules: vec![String::from("license")],
                    automatic_count: 1,
                    guided_count: 0,
                    manual_count: 0,
                    preserve: Vec::new(),
                    issues: vec![PromptIssue {
                        rule_id: "license",
                        rule_label: String::from("License"),
                        category: RuleCategory::Identity,
                        status: CheckStatus::Missing,
                        fixability: crate::audit::Fixability::Automatic,
                        gap: 10,
                        blocking: false,
                        primary_cause: None,
                        summary: String::from("Missing"),
                        hint: String::from("Add it"),
                        findings: Vec::new(),
                        locations: Vec::new(),
                        evidence: Vec::new(),
                    }],
                    prompt: String::from("second prompt"),
                },
            ],
        };

        let text = prompt_report_text(&report);
        assert!(text.contains("first prompt"));
        assert!(text.contains("second prompt"));
        assert!(text.contains("---"));
    }
}
