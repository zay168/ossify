use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::audit::{audit_repository, AuditReport};
use crate::templates;

#[derive(Debug, Clone)]
pub struct InitOptions {
    pub overwrite: bool,
    pub license: LicenseKind,
    pub owner: String,
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

#[derive(Debug, Clone)]
pub enum FileAction {
    Created,
    Skipped,
}

impl FileAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Skipped => "skipped",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ScaffoldMode {
    Init,
    Fix,
}

impl ScaffoldMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Init => "init",
            Self::Fix => "fix",
        }
    }
}

#[derive(Debug, Clone)]
pub struct GeneratedFile {
    pub path: PathBuf,
    pub action: FileAction,
}

#[derive(Debug, Clone)]
pub struct InitReport {
    pub target: PathBuf,
    pub mode: ScaffoldMode,
    pub files: Vec<GeneratedFile>,
}

#[derive(Debug, Clone)]
pub struct FixReport {
    pub target: PathBuf,
    pub before: AuditReport,
    pub generated: InitReport,
    pub after: AuditReport,
}

pub fn generate_missing_files(path: &Path, options: &InitOptions) -> io::Result<InitReport> {
    generate_files(path, options, ScaffoldMode::Init, &ScaffoldSelection::All)
}

pub fn fix_repository(path: &Path, options: &InitOptions) -> io::Result<FixReport> {
    let before = audit_repository(path)?;
    let selection = if options.overwrite {
        ScaffoldSelection::AllFixable
    } else {
        ScaffoldSelection::FromAudit(before.clone())
    };

    let generated = if before.score == 100 && !options.overwrite {
        InitReport {
            target: before.target.clone(),
            mode: ScaffoldMode::Fix,
            files: Vec::new(),
        }
    } else {
        generate_files(path, options, ScaffoldMode::Fix, &selection)?
    };
    let after = audit_repository(path)?;

    Ok(FixReport {
        target: after.target.clone(),
        before,
        generated,
        after,
    })
}

fn generate_files(
    path: &Path,
    options: &InitOptions,
    mode: ScaffoldMode,
    selection: &ScaffoldSelection,
) -> io::Result<InitReport> {
    ensure_directory(path)?;
    fs::create_dir_all(path)?;

    let canonical = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let project_name = canonical
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("my-project")
        .to_owned();
    let year = current_year_fallback();
    let items = selection.items();

    let mut files = Vec::new();
    for item in items {
        append_item_files(
            &canonical,
            &project_name,
            year,
            options,
            item,
            &mut files,
        )?;
    }

    Ok(InitReport {
        target: canonical,
        mode,
        files,
    })
}

fn append_item_files(
    root: &Path,
    project_name: &str,
    year: i32,
    options: &InitOptions,
    item: ScaffoldItem,
    files: &mut Vec<GeneratedFile>,
) -> io::Result<()> {
    match item {
        ScaffoldItem::Readme => files.push(write_file(
            &root.join("README.md"),
            &templates::readme(project_name),
            options.overwrite,
        )?),
        ScaffoldItem::License => files.push(write_file(
            &root.join("LICENSE"),
            &templates::license_text(options.license, &options.owner, year),
            options.overwrite,
        )?),
        ScaffoldItem::Contributing => files.push(write_file(
            &root.join("CONTRIBUTING.md"),
            &templates::contributing(project_name),
            options.overwrite,
        )?),
        ScaffoldItem::CodeOfConduct => files.push(write_file(
            &root.join("CODE_OF_CONDUCT.md"),
            &templates::code_of_conduct(project_name),
            options.overwrite,
        )?),
        ScaffoldItem::Security => files.push(write_file(
            &root.join("SECURITY.md"),
            &templates::security_policy(project_name),
            options.overwrite,
        )?),
        ScaffoldItem::Changelog => files.push(write_file(
            &root.join("CHANGELOG.md"),
            &templates::changelog(),
            options.overwrite,
        )?),
        ScaffoldItem::IssueTemplates => {
            fs::create_dir_all(root.join(".github/ISSUE_TEMPLATE"))?;
            files.push(write_file(
                &root.join(".github/ISSUE_TEMPLATE/bug_report.md"),
                &templates::bug_report_template(project_name),
                options.overwrite,
            )?);
            files.push(write_file(
                &root.join(".github/ISSUE_TEMPLATE/feature_request.md"),
                &templates::feature_request_template(project_name),
                options.overwrite,
            )?);
        }
        ScaffoldItem::PullRequestTemplate => files.push(write_file(
            &root.join(".github/PULL_REQUEST_TEMPLATE.md"),
            &templates::pull_request_template(),
            options.overwrite,
        )?),
        ScaffoldItem::CiWorkflow => {
            fs::create_dir_all(root.join(".github/workflows"))?;
            files.push(write_file(
                &root.join(".github/workflows/ci.yml"),
                &templates::ci_workflow(),
                options.overwrite,
            )?);
        }
    }

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

fn write_file(path: &Path, content: &str, overwrite: bool) -> io::Result<GeneratedFile> {
    if path.exists() && !overwrite {
        return Ok(GeneratedFile {
            path: path.to_path_buf(),
            action: FileAction::Skipped,
        });
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(path, content)?;

    Ok(GeneratedFile {
        path: path.to_path_buf(),
        action: FileAction::Created,
    })
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
}

#[derive(Debug, Clone)]
enum ScaffoldSelection {
    All,
    AllFixable,
    FromAudit(AuditReport),
}

impl ScaffoldSelection {
    fn items(&self) -> Vec<ScaffoldItem> {
        match self {
            Self::All | Self::AllFixable => vec![
                ScaffoldItem::Readme,
                ScaffoldItem::License,
                ScaffoldItem::Contributing,
                ScaffoldItem::CodeOfConduct,
                ScaffoldItem::Security,
                ScaffoldItem::Changelog,
                ScaffoldItem::IssueTemplates,
                ScaffoldItem::PullRequestTemplate,
                ScaffoldItem::CiWorkflow,
            ],
            Self::FromAudit(report) => {
                let mut items = Vec::new();
                for check in report.missing_checks() {
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
        _ => None,
    }
}
