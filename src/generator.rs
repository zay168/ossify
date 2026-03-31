use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::audit::{
    audit_repository, estimate_report_after_upgrades, AuditReport, CheckStatus, PlannedCheckUpgrade,
};
use crate::config::OssifyConfig;
use crate::project::{detect_project, ProjectContext};
use crate::templates;

#[derive(Debug, Clone)]
pub struct InitOptions {
    pub overwrite: bool,
    pub license: LicenseKind,
    pub owner: String,
    pub funding: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub enum LicenseKind {
    Mit,
    Apache2,
}

impl LicenseKind {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "mit" | "MIT" => Ok(Self::Mit),
            "apache-2.0" | "Apache-2.0" | "apache" => Ok(Self::Apache2),
            _ => Err(format!(
                "Unsupported license: {value}. Use `mit` or `apache-2.0`."
            )),
        }
    }
}

impl std::fmt::Display for LicenseKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Mit => write!(f, "mit"),
            Self::Apache2 => write!(f, "apache-2.0"),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FileAction {
    Created,
    Updated,
    Skipped,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ScaffoldMode {
    Init,
    Fix,
    Plan,
}

impl ScaffoldMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Init => "init",
            Self::Fix => "fix",
            Self::Plan => "plan",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct GeneratedFile {
    pub path: PathBuf,
    pub action: FileAction,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InitReport {
    pub target: PathBuf,
    pub mode: ScaffoldMode,
    pub project: ProjectContext,
    pub files: Vec<GeneratedFile>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FixReport {
    pub target: PathBuf,
    pub before: AuditReport,
    pub generated: InitReport,
    pub after: AuditReport,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlanReport {
    pub target: PathBuf,
    pub before: AuditReport,
    pub planned: InitReport,
    pub estimated_after: AuditReport,
}

#[derive(Debug, Clone)]
struct PlannedFileChange {
    item: ScaffoldItem,
    generated: GeneratedFile,
    content: Option<String>,
}

#[derive(Debug, Clone)]
struct PreparedScaffold {
    report: InitReport,
    operations: Vec<PlannedFileChange>,
}

pub fn generate_missing_files(
    path: &Path,
    options: &InitOptions,
    config: &OssifyConfig,
) -> io::Result<InitReport> {
    let prepared = plan_scaffold(
        path,
        options,
        config,
        ScaffoldMode::Init,
        &ScaffoldSelection::All,
    )?;
    apply_scaffold_plan(&prepared.operations)?;

    Ok(prepared.report)
}

pub fn fix_repository(
    path: &Path,
    options: &InitOptions,
    config: &OssifyConfig,
) -> io::Result<FixReport> {
    let before = audit_repository(path, config)?;
    let selection = ScaffoldSelection::FromAudit {
        report: Box::new(before.clone()),
        overwrite_partials: options.overwrite,
    };

    let prepared = plan_scaffold(path, options, config, ScaffoldMode::Fix, &selection)?;
    apply_scaffold_plan(&prepared.operations)?;
    let after = audit_repository(path, config)?;

    Ok(FixReport {
        target: after.target.clone(),
        before,
        generated: prepared.report,
        after,
    })
}

pub fn plan_fix_repository(
    path: &Path,
    options: &InitOptions,
    config: &OssifyConfig,
) -> io::Result<PlanReport> {
    let before = audit_repository(path, config)?;
    let selection = ScaffoldSelection::FromAudit {
        report: Box::new(before.clone()),
        overwrite_partials: options.overwrite,
    };
    let prepared = plan_scaffold(path, options, config, ScaffoldMode::Plan, &selection)?;
    let upgrades = planned_upgrades(&prepared.operations);
    let estimated_after = estimate_report_after_upgrades(&before, &upgrades);

    Ok(PlanReport {
        target: before.target.clone(),
        before,
        planned: prepared.report,
        estimated_after,
    })
}

fn plan_scaffold(
    path: &Path,
    options: &InitOptions,
    config: &OssifyConfig,
    mode: ScaffoldMode,
    selection: &ScaffoldSelection,
) -> io::Result<PreparedScaffold> {
    ensure_directory(path)?;

    let canonical = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let project = detect_project(&canonical)?.with_profile_override(config.profile_override());
    let year = current_year_fallback();
    let items = selection.items();

    let mut operations = Vec::new();
    for item in items {
        append_item_plan(&canonical, &project, year, options, item, &mut operations)?;
    }

    let files = operations
        .iter()
        .map(|operation| operation.generated.clone())
        .collect::<Vec<GeneratedFile>>();

    Ok(PreparedScaffold {
        report: InitReport {
            target: canonical,
            mode,
            project,
            files,
        },
        operations,
    })
}

fn apply_scaffold_plan(operations: &[PlannedFileChange]) -> io::Result<()> {
    for operation in operations {
        let should_write = matches!(
            operation.generated.action,
            FileAction::Created | FileAction::Updated
        );
        if !should_write {
            continue;
        }

        let Some(content) = &operation.content else {
            continue;
        };

        if let Some(parent) = operation.generated.path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&operation.generated.path, content)?;
    }

    Ok(())
}

fn append_item_plan(
    root: &Path,
    project: &ProjectContext,
    year: i32,
    options: &InitOptions,
    item: ScaffoldItem,
    operations: &mut Vec<PlannedFileChange>,
) -> io::Result<()> {
    let mut files = Vec::new();
    match item {
        ScaffoldItem::Readme => files.push(plan_file(
            item,
            &root.join("README.md"),
            Some(templates::readme(project)),
            options.overwrite,
            None,
        )),
        ScaffoldItem::License => files.push(plan_file(
            item,
            &root.join("LICENSE"),
            Some(templates::license_text(
                options.license,
                &options.owner,
                year,
            )),
            options.overwrite,
            None,
        )),
        ScaffoldItem::Contributing => files.push(plan_file(
            item,
            &root.join("CONTRIBUTING.md"),
            Some(templates::contributing(project)),
            options.overwrite,
            None,
        )),
        ScaffoldItem::CodeOfConduct => files.push(plan_file(
            item,
            &root.join("CODE_OF_CONDUCT.md"),
            Some(templates::code_of_conduct(&project.name)),
            options.overwrite,
            None,
        )),
        ScaffoldItem::Security => files.push(plan_file(
            item,
            &root.join("SECURITY.md"),
            Some(templates::security_policy(&project.name)),
            options.overwrite,
            None,
        )),
        ScaffoldItem::Changelog => files.push(plan_file(
            item,
            &root.join("CHANGELOG.md"),
            Some(templates::changelog()),
            options.overwrite,
            None,
        )),
        ScaffoldItem::IssueTemplates => {
            files.push(plan_file(
                item,
                &root.join(".github/ISSUE_TEMPLATE/bug_report.md"),
                Some(templates::bug_report_template(&project.name)),
                options.overwrite,
                None,
            ));
            files.push(plan_file(
                item,
                &root.join(".github/ISSUE_TEMPLATE/feature_request.md"),
                Some(templates::feature_request_template(&project.name)),
                options.overwrite,
                None,
            ));
        }
        ScaffoldItem::PullRequestTemplate => files.push(plan_file(
            item,
            &root.join(".github/PULL_REQUEST_TEMPLATE.md"),
            Some(templates::pull_request_template()),
            options.overwrite,
            None,
        )),
        ScaffoldItem::CiWorkflow => files.push(plan_file(
            item,
            &root.join(".github/workflows/ci.yml"),
            Some(templates::ci_workflow(project)),
            options.overwrite,
            None,
        )),
        ScaffoldItem::Codeowners => {
            files.push(plan_file(
                item,
                &root.join(".github/CODEOWNERS"),
                templates::codeowners(&options.owner),
                options.overwrite,
                Some(
                    "CODEOWNERS generation requires `--owner` to look like a GitHub handle such as @acme.",
                ),
            ));
        }
        ScaffoldItem::Funding => {
            files.push(plan_file(
                item,
                &root.join(".github/FUNDING.yml"),
                templates::funding_file(options.funding.as_deref()),
                options.overwrite,
                Some(
                    "FUNDING.yml generation requires `--funding`, for example `github:acme` or `custom:https://example.com/sponsor`.",
                ),
            ));
        }
        ScaffoldItem::Dependabot => files.push(plan_file(
            item,
            &root.join(".github/dependabot.yml"),
            Some(templates::dependabot(project)),
            options.overwrite,
            None,
        )),
        ScaffoldItem::ReleaseWorkflow => files.push(plan_file(
            item,
            &root.join(".github/workflows/release.yml"),
            Some(templates::release_workflow(project)),
            options.overwrite,
            None,
        )),
    }

    operations.extend(files);

    Ok(())
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

fn plan_file(
    item: ScaffoldItem,
    path: &Path,
    content: Option<String>,
    overwrite: bool,
    missing_reason: Option<&str>,
) -> PlannedFileChange {
    let generated = match &content {
        Some(_) if path.exists() && !overwrite => GeneratedFile {
            path: path.to_path_buf(),
            action: FileAction::Skipped,
            reason: Some(String::from(
                "File already exists. Re-run with --overwrite to replace it.",
            )),
        },
        Some(_) => GeneratedFile {
            path: path.to_path_buf(),
            action: if path.exists() {
                FileAction::Updated
            } else {
                FileAction::Created
            },
            reason: None,
        },
        None => GeneratedFile {
            path: path.to_path_buf(),
            action: FileAction::Skipped,
            reason: Some(
                missing_reason
                    .unwrap_or("Missing required inputs.")
                    .to_owned(),
            ),
        },
    };

    let should_keep_content = matches!(generated.action, FileAction::Created | FileAction::Updated);

    PlannedFileChange {
        item,
        generated,
        content: if should_keep_content { content } else { None },
    }
}

fn planned_upgrades(operations: &[PlannedFileChange]) -> Vec<PlannedCheckUpgrade> {
    let mut upgrades = Vec::new();

    for item in all_fixable_items() {
        let relevant = operations
            .iter()
            .filter(|operation| operation.item == item)
            .collect::<Vec<&PlannedFileChange>>();
        if relevant.is_empty() {
            continue;
        }

        let improved = relevant.iter().all(|operation| {
            matches!(
                operation.generated.action,
                FileAction::Created | FileAction::Updated
            )
        });
        if !improved {
            continue;
        }

        let evidence = relevant
            .iter()
            .map(|operation| operation.generated.path.display().to_string())
            .collect::<Vec<String>>();
        let location = relevant
            .first()
            .map(|operation| operation.generated.path.clone());

        upgrades.push(PlannedCheckUpgrade {
            rule_id: check_for_item(item),
            message: format!(
                "Planned scaffolding would satisfy this rule by adding {}.",
                evidence.join(", ")
            ),
            evidence,
            location,
        });
    }

    upgrades
}

fn current_year_fallback() -> i32 {
    const SECONDS_PER_YEAR: u64 = 31_556_952;
    let years_since_1970 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() / SECONDS_PER_YEAR)
        .unwrap_or(56);

    1970 + years_since_1970 as i32
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScaffoldItem {
    Readme,
    License,
    Contributing,
    CodeOfConduct,
    Security,
    Changelog,
    IssueTemplates,
    PullRequestTemplate,
    CiWorkflow,
    Codeowners,
    Funding,
    Dependabot,
    ReleaseWorkflow,
}

#[derive(Debug, Clone)]
enum ScaffoldSelection {
    All,
    FromAudit {
        report: Box<AuditReport>,
        overwrite_partials: bool,
    },
}

impl ScaffoldSelection {
    fn items(&self) -> Vec<ScaffoldItem> {
        match self {
            Self::All => all_fixable_items(),
            Self::FromAudit {
                report,
                overwrite_partials,
            } => {
                let mut items = Vec::new();
                for check in &report.checks {
                    let should_include = match check.status {
                        CheckStatus::Missing => check.fixable,
                        CheckStatus::Partial => *overwrite_partials && check.fixable,
                        CheckStatus::Strong => false,
                    };
                    if !should_include {
                        continue;
                    }

                    if let Some(item) = item_for_check(check.id) {
                        if !items.contains(&item) {
                            items.push(item);
                        }
                    }
                }
                items
            }
        }
    }
}

fn all_fixable_items() -> Vec<ScaffoldItem> {
    vec![
        ScaffoldItem::Readme,
        ScaffoldItem::License,
        ScaffoldItem::Contributing,
        ScaffoldItem::CodeOfConduct,
        ScaffoldItem::Security,
        ScaffoldItem::Changelog,
        ScaffoldItem::IssueTemplates,
        ScaffoldItem::PullRequestTemplate,
        ScaffoldItem::CiWorkflow,
        ScaffoldItem::Codeowners,
        ScaffoldItem::Funding,
        ScaffoldItem::Dependabot,
        ScaffoldItem::ReleaseWorkflow,
    ]
}

fn item_for_check(id: &str) -> Option<ScaffoldItem> {
    match id {
        "readme" => Some(ScaffoldItem::Readme),
        "license" => Some(ScaffoldItem::License),
        "contributing_guide" => Some(ScaffoldItem::Contributing),
        "code_of_conduct" => Some(ScaffoldItem::CodeOfConduct),
        "security_policy" => Some(ScaffoldItem::Security),
        "changelog" => Some(ScaffoldItem::Changelog),
        "issue_templates" => Some(ScaffoldItem::IssueTemplates),
        "pull_request_template" => Some(ScaffoldItem::PullRequestTemplate),
        "ci_workflow" => Some(ScaffoldItem::CiWorkflow),
        "codeowners" => Some(ScaffoldItem::Codeowners),
        "funding" => Some(ScaffoldItem::Funding),
        "dependabot" => Some(ScaffoldItem::Dependabot),
        "release_workflow" => Some(ScaffoldItem::ReleaseWorkflow),
        _ => None,
    }
}

fn check_for_item(item: ScaffoldItem) -> &'static str {
    match item {
        ScaffoldItem::Readme => "readme",
        ScaffoldItem::License => "license",
        ScaffoldItem::Contributing => "contributing_guide",
        ScaffoldItem::CodeOfConduct => "code_of_conduct",
        ScaffoldItem::Security => "security_policy",
        ScaffoldItem::Changelog => "changelog",
        ScaffoldItem::IssueTemplates => "issue_templates",
        ScaffoldItem::PullRequestTemplate => "pull_request_template",
        ScaffoldItem::CiWorkflow => "ci_workflow",
        ScaffoldItem::Codeowners => "codeowners",
        ScaffoldItem::Funding => "funding",
        ScaffoldItem::Dependabot => "dependabot",
        ScaffoldItem::ReleaseWorkflow => "release_workflow",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_repo(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(name);
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create temp directory");
        path
    }

    #[test]
    fn guided_files_skip_when_inputs_are_missing() {
        let root = temp_repo("ossify-generator-guided-skip");
        let options = InitOptions {
            overwrite: false,
            license: LicenseKind::Mit,
            owner: String::from("Open Source Maintainers"),
            funding: None,
        };
        let report = generate_missing_files(&root, &options, &OssifyConfig::default())
            .expect("generate files");

        assert!(report
            .files
            .iter()
            .any(|file| file.path.ends_with(Path::new(".github/CODEOWNERS"))
                && matches!(file.action, FileAction::Skipped)));
        assert!(report
            .files
            .iter()
            .any(|file| file.path.ends_with(Path::new(".github/FUNDING.yml"))
                && matches!(file.action, FileAction::Skipped)));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn fix_generates_dependabot_and_release_workflow() {
        let root = temp_repo("ossify-generator-fix");
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
        let report = fix_repository(&root, &options, &OssifyConfig::default()).expect("fix repo");

        assert!(report
            .generated
            .files
            .iter()
            .any(|file| file.path.ends_with(Path::new(".github/dependabot.yml"))));
        assert!(report.generated.files.iter().any(|file| file
            .path
            .ends_with(Path::new(".github/workflows/release.yml"))));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn plan_fix_does_not_write_files() {
        let root = temp_repo("ossify-generator-plan-no-write");
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

        assert!(report
            .planned
            .files
            .iter()
            .any(|file| file.path.ends_with(Path::new("README.md"))));
        assert!(!root.join("README.md").exists());
        assert!(!root.join(".github/workflows/ci.yml").exists());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn plan_fix_respects_overwrite_for_partial_files() {
        let root = temp_repo("ossify-generator-plan-overwrite");
        fs::create_dir_all(root.join("src")).expect("create src");
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n",
        )
        .expect("write Cargo.toml");
        fs::write(root.join("src/main.rs"), "fn main() {}\n").expect("write main.rs");
        fs::write(
            root.join("README.md"),
            "# Demo\n\n## Install\n\n```bash\ncargo build\n```\n\n## Usage\n\n```bash\ncargo run -- --help\n```\n",
        )
        .expect("write README");

        let base_options = InitOptions {
            overwrite: false,
            license: LicenseKind::Mit,
            owner: String::from("@acme"),
            funding: Some(String::from("github:acme")),
        };
        let report =
            plan_fix_repository(&root, &base_options, &OssifyConfig::default()).expect("plan fix");
        assert!(!report
            .planned
            .files
            .iter()
            .any(|file| file.path.ends_with(Path::new("README.md"))));

        let overwrite_options = InitOptions {
            overwrite: true,
            ..base_options
        };
        let overwrite_report =
            plan_fix_repository(&root, &overwrite_options, &OssifyConfig::default())
                .expect("plan fix with overwrite");
        assert!(overwrite_report
            .planned
            .files
            .iter()
            .any(|file| file.path.ends_with(Path::new("README.md"))
                && matches!(file.action, FileAction::Updated)));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn plan_fix_surfaces_guided_skips_and_estimated_score() {
        let root = temp_repo("ossify-generator-plan-guided");
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
            owner: String::from("Open Source Maintainers"),
            funding: None,
        };
        let report =
            plan_fix_repository(&root, &options, &OssifyConfig::default()).expect("plan fix");

        assert!(report
            .planned
            .files
            .iter()
            .any(|file| file.path.ends_with(Path::new(".github/CODEOWNERS"))
                && matches!(file.action, FileAction::Skipped)));
        assert!(report.estimated_after.score > report.before.score);
        assert!(report
            .estimated_after
            .checks
            .iter()
            .any(|check| check.id == "manifest_metadata" && check.status != CheckStatus::Strong));
        assert!(report
            .estimated_after
            .checks
            .iter()
            .any(|check| check.id == "readme"
                && check.status == CheckStatus::Strong
                && check.primary_cause.is_none()
                && !check.proof.is_empty()));

        let _ = fs::remove_dir_all(&root);
    }
}
