use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::project::{detect_project, ProjectContext, ProjectKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

    pub fn earned_points(self, weight: u8) -> u8 {
        match self {
            Self::Strong => weight,
            Self::Partial => ((u16::from(weight) * 60 + 99) / 100) as u8,
            Self::Missing => 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug, Clone)]
pub struct AuditCheck {
    pub id: &'static str,
    pub label: &'static str,
    pub weight: u8,
    pub earned: u8,
    pub status: CheckStatus,
    pub fixable: bool,
    pub hint: &'static str,
    pub detail: Option<String>,
    pub location: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct AuditReport {
    pub target: PathBuf,
    pub project: ProjectContext,
    pub readiness: ReadinessTier,
    pub score: u8,
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
}

pub fn audit_repository(path: &Path) -> io::Result<AuditReport> {
    ensure_directory(path)?;

    let canonical = if path.exists() {
        fs::canonicalize(path)?
    } else {
        path.to_path_buf()
    };
    let project = detect_project(&canonical)?;
    let files = collect_files(&canonical, 5)?;
    let workflow_files = workflow_files(&canonical, &files);
    let workflow_text = read_many(&workflow_files);

    let checks = vec![
        assess_readme(&canonical),
        assess_license(&canonical),
        assess_contributing(&canonical),
        assess_code_of_conduct(&canonical),
        assess_security(&canonical),
        assess_changelog(&canonical),
        assess_issue_templates(&canonical),
        assess_pull_request_template(&canonical),
        assess_ci_workflow(&canonical, &project, &workflow_files, &workflow_text),
        assess_project_manifest(&project),
        assess_tests(&canonical, &project, &files, &workflow_text),
    ];

    let total: u16 = checks.iter().map(|check| u16::from(check.weight)).sum();
    let earned: u16 = checks.iter().map(|check| u16::from(check.earned)).sum();
    let score = ((earned * 100) / total) as u8;

    Ok(AuditReport {
        target: canonical,
        project,
        readiness: readiness_tier(score),
        score,
        checks,
    })
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

fn ensure_directory(path: &Path) -> io::Result<()> {
    if path.exists() && !path.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{} is not a directory", path.display()),
        ));
    }

    Ok(())
}

fn assess_readme(root: &Path) -> AuditCheck {
    let weight = 14;
    let hint = "Write a README that explains the project, how to install it, and how to use it.";

    match first_existing_file(root, &["README.md", "README"]) {
        Some(path) => {
            let contents = read_text(&path);
            let normalized = normalize(&contents);
            let has_install = contains_any(&normalized, &["## install", "### install", " install"]);
            let has_usage = contains_any(&normalized, &["## usage", "## quick start", "### usage", " example"]);
            let has_context = contains_any(&normalized, &["## why", "## overview", "## what it does"]);

            let status = if normalized.len() >= 260
                && has_install
                && has_usage
                && (has_context || normalized.len() >= 420)
                && !looks_placeholder(&normalized)
            {
                CheckStatus::Strong
            } else {
                CheckStatus::Partial
            };

            let detail = if status == CheckStatus::Strong {
                Some(relative_display(root, &path))
            } else if looks_placeholder(&normalized) {
                Some(format!(
                    "{} exists, but it still reads like scaffold copy.",
                    relative_display(root, &path)
                ))
            } else {
                Some(format!(
                    "{} exists, but it needs stronger install and usage guidance.",
                    relative_display(root, &path)
                ))
            };

            build_check(
                "readme",
                "README",
                weight,
                status,
                true,
                hint,
                detail,
                Some(path),
            )
        }
        None => build_check(
            "readme",
            "README",
            weight,
            CheckStatus::Missing,
            true,
            hint,
            None,
            None,
        ),
    }
}

fn assess_license(root: &Path) -> AuditCheck {
    let weight = 16;
    let hint = "Choose a clear license so adopters know exactly how they can use the project.";

    match first_existing_file(root, &["LICENSE", "LICENSE.md", "COPYING"]) {
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
            let detail = if status == CheckStatus::Strong {
                Some(format!(
                    "{} contains recognized license text.",
                    relative_display(root, &path)
                ))
            } else {
                Some(format!(
                    "{} exists, but the license text was not clearly recognized.",
                    relative_display(root, &path)
                ))
            };

            build_check(
                "license",
                "License",
                weight,
                status,
                true,
                hint,
                detail,
                Some(path),
            )
        }
        None => build_check(
            "license",
            "License",
            weight,
            CheckStatus::Missing,
            true,
            hint,
            None,
            None,
        ),
    }
}

fn assess_contributing(root: &Path) -> AuditCheck {
    let weight = 9;
    let hint = "Document how contributors should branch, test, and open pull requests.";

    assess_quality_document(
        root,
        "contributing_guide",
        "Contributing guide",
        &["CONTRIBUTING.md"],
        weight,
        true,
        hint,
        180,
        &["pull request", "issue", "test", "branch"],
    )
}

fn assess_code_of_conduct(root: &Path) -> AuditCheck {
    let weight = 8;
    let hint = "Define expected behavior and reporting steps so the project feels safe to join.";

    assess_quality_document(
        root,
        "code_of_conduct",
        "Code of conduct",
        &["CODE_OF_CONDUCT.md"],
        weight,
        true,
        hint,
        160,
        &["expected behavior", "unacceptable behavior", "report"],
    )
}

fn assess_security(root: &Path) -> AuditCheck {
    let weight = 10;
    let hint = "Explain how vulnerabilities should be reported and what private disclosure path to use.";

    assess_quality_document(
        root,
        "security_policy",
        "Security policy",
        &["SECURITY.md"],
        weight,
        true,
        hint,
        150,
        &["vulnerability", "report", "private"],
    )
}

fn assess_changelog(root: &Path) -> AuditCheck {
    let weight = 7;
    let hint = "Keep a changelog so releases and project evolution are easier to trust.";

    match first_existing_file(root, &["CHANGELOG.md"]) {
        Some(path) => {
            let contents = normalize(&read_text(&path));
            let has_versions = contents.contains("## unreleased")
                || contents.contains("## [")
                || contents.contains("## 0.")
                || contents.contains("## 1.");

            let status = if has_versions && contents.len() >= 80 {
                CheckStatus::Strong
            } else {
                CheckStatus::Partial
            };

            let detail = if status == CheckStatus::Strong {
                Some(format!(
                    "{} already tracks release history.",
                    relative_display(root, &path)
                ))
            } else {
                Some(format!(
                    "{} exists, but it does not yet look like a maintained release log.",
                    relative_display(root, &path)
                ))
            };

            build_check(
                "changelog",
                "Changelog",
                weight,
                status,
                true,
                hint,
                detail,
                Some(path),
            )
        }
        None => build_check(
            "changelog",
            "Changelog",
            weight,
            CheckStatus::Missing,
            true,
            hint,
            None,
            None,
        ),
    }
}

fn assess_issue_templates(root: &Path) -> AuditCheck {
    let weight = 8;
    let hint = "Issue templates raise report quality by making bug reports and requests more structured.";
    let directory = root.join(".github/ISSUE_TEMPLATE");

    if !directory.is_dir() {
        return build_check(
            "issue_templates",
            "Issue templates",
            weight,
            CheckStatus::Missing,
            true,
            hint,
            None,
            None,
        );
    }

    let templates = list_files(&directory);
    let names: Vec<String> = templates
        .iter()
        .filter_map(|path| path.file_name().and_then(|value| value.to_str()).map(str::to_owned))
        .collect();
    let normalized_names = names.join(" ").to_lowercase();
    let has_bug = normalized_names.contains("bug");
    let has_feature = normalized_names.contains("feature") || normalized_names.contains("enhancement");

    let status = if has_bug && has_feature {
        CheckStatus::Strong
    } else {
        CheckStatus::Partial
    };

    let detail = if status == CheckStatus::Strong {
        Some(format!(
            "Found multiple issue templates in {}.",
            relative_display(root, &directory)
        ))
    } else {
        Some(format!(
            "{} exists, but it would be stronger with separate bug and feature templates.",
            relative_display(root, &directory)
        ))
    };

    build_check(
        "issue_templates",
        "Issue templates",
        weight,
        status,
        true,
        hint,
        detail,
        Some(directory),
    )
}

fn assess_pull_request_template(root: &Path) -> AuditCheck {
    let weight = 6;
    let hint = "A pull request template keeps reviews focused on change summary, impact, and verification.";

    match first_existing_file(root, &[".github/PULL_REQUEST_TEMPLATE.md"]) {
        Some(path) => {
            let contents = normalize(&read_text(&path));
            let strong = contains_any(&contents, &["checklist", "what changed", "why it matters", "testing"]);
            let status = if strong && contents.len() >= 100 {
                CheckStatus::Strong
            } else {
                CheckStatus::Partial
            };
            let detail = if status == CheckStatus::Strong {
                Some(format!(
                    "{} sets review expectations clearly.",
                    relative_display(root, &path)
                ))
            } else {
                Some(format!(
                    "{} exists, but it should ask for more context or verification details.",
                    relative_display(root, &path)
                ))
            };

            build_check(
                "pull_request_template",
                "Pull request template",
                weight,
                status,
                true,
                hint,
                detail,
                Some(path),
            )
        }
        None => build_check(
            "pull_request_template",
            "Pull request template",
            weight,
            CheckStatus::Missing,
            true,
            hint,
            None,
            None,
        ),
    }
}

fn assess_ci_workflow(
    root: &Path,
    project: &ProjectContext,
    workflow_files: &[PathBuf],
    workflow_text: &str,
) -> AuditCheck {
    let weight = 10;
    let hint = "A healthy repo should have CI that matches the project type and runs the most important checks.";

    if workflow_files.is_empty() {
        return build_check(
            "ci_workflow",
            "CI workflow",
            weight,
            CheckStatus::Missing,
            true,
            hint,
            None,
            None,
        );
    }

    let normalized = normalize(workflow_text);
    let strong = if project.kind == ProjectKind::Unknown {
        false
    } else {
        project
            .kind
            .ci_keywords()
            .iter()
            .any(|keyword| normalized.contains(keyword))
    };

    let status = if strong {
        CheckStatus::Strong
    } else {
        CheckStatus::Partial
    };

    let detail = if status == CheckStatus::Strong {
        Some(format!(
            "Detected {} CI signals in {} workflow file(s).",
            project.kind.display_name(),
            workflow_files.len()
        ))
    } else {
        Some(format!(
            "Found workflow files, but none clearly match this {} project yet.",
            project.kind.display_name()
        ))
    };

    build_check(
        "ci_workflow",
        "CI workflow",
        weight,
        status,
        true,
        hint,
        detail,
        Some(root.join(".github/workflows")),
    )
}

fn assess_project_manifest(project: &ProjectContext) -> AuditCheck {
    let weight = 6;
    let hint = "A detected manifest helps ossify infer project type and give project-specific guidance.";

    match &project.manifest_path {
        Some(path) if project.kind != ProjectKind::Unknown => build_check(
            "project_manifest",
            "Project manifest",
            weight,
            CheckStatus::Strong,
            false,
            hint,
            Some(format!(
                "Detected {} project metadata in {}.",
                project.kind.display_name(),
                path.display()
            )),
            Some(path.clone()),
        ),
        Some(path) => build_check(
            "project_manifest",
            "Project manifest",
            weight,
            CheckStatus::Partial,
            false,
            hint,
            Some(format!(
                "Found {}, but the project type is still ambiguous.",
                path.display()
            )),
            Some(path.clone()),
        ),
        None => build_check(
            "project_manifest",
            "Project manifest",
            weight,
            CheckStatus::Missing,
            false,
            hint,
            None,
            None,
        ),
    }
}

fn assess_tests(
    root: &Path,
    project: &ProjectContext,
    files: &[PathBuf],
    workflow_text: &str,
) -> AuditCheck {
    let weight = 6;
    let hint = "Tests make the public surface more trustworthy and keep future fixes safer.";
    let detected = test_files(root, files, project.kind);

    if !detected.is_empty() {
        return build_check(
            "tests",
            "Tests",
            weight,
            CheckStatus::Strong,
            false,
            hint,
            Some(format!(
                "Detected {} test file(s), including {}.",
                detected.len(),
                detected[0]
            )),
            None,
        );
    }

    let normalized = normalize(workflow_text);
    let ci_mentions_tests = contains_any(
        &normalized,
        &["cargo test", "npm test", "pnpm test", "pytest", "go test", "test"],
    );

    let status = if ci_mentions_tests {
        CheckStatus::Partial
    } else {
        CheckStatus::Missing
    };
    let detail = if status == CheckStatus::Partial {
        Some(String::from(
            "CI appears to run tests, but no test files were detected in the repository.",
        ))
    } else {
        None
    };

    build_check("tests", "Tests", weight, status, false, hint, detail, None)
}

fn assess_quality_document(
    root: &Path,
    id: &'static str,
    label: &'static str,
    candidates: &[&str],
    weight: u8,
    fixable: bool,
    hint: &'static str,
    min_strong_len: usize,
    keywords: &[&str],
) -> AuditCheck {
    match first_existing_file(root, candidates) {
        Some(path) => {
            let contents = normalize(&read_text(&path));
            let hits = keywords
                .iter()
                .filter(|keyword| contents.contains(**keyword))
                .count();
            let status = if contents.len() >= min_strong_len
                && hits >= (keywords.len().min(3))
                && !looks_placeholder(&contents)
            {
                CheckStatus::Strong
            } else {
                CheckStatus::Partial
            };
            let detail = if status == CheckStatus::Strong {
                Some(format!(
                    "{} looks established enough to guide contributors.",
                    relative_display(root, &path)
                ))
            } else if looks_placeholder(&contents) {
                Some(format!(
                    "{} exists, but it still looks like starter copy.",
                    relative_display(root, &path)
                ))
            } else {
                Some(format!(
                    "{} exists, but it could be more specific or complete.",
                    relative_display(root, &path)
                ))
            };

            build_check(id, label, weight, status, fixable, hint, detail, Some(path))
        }
        None => build_check(id, label, weight, CheckStatus::Missing, fixable, hint, None, None),
    }
}

fn build_check(
    id: &'static str,
    label: &'static str,
    weight: u8,
    status: CheckStatus,
    fixable: bool,
    hint: &'static str,
    detail: Option<String>,
    location: Option<PathBuf>,
) -> AuditCheck {
    AuditCheck {
        id,
        label,
        weight,
        earned: status.earned_points(weight),
        status,
        fixable,
        hint,
        detail,
        location,
    }
}

fn first_existing_file(root: &Path, candidates: &[&str]) -> Option<PathBuf> {
    candidates
        .iter()
        .map(|candidate| root.join(candidate))
        .find(|candidate| candidate.is_file())
}

fn workflow_files(root: &Path, files: &[PathBuf]) -> Vec<PathBuf> {
    files.iter()
        .filter(|path| {
            path.extension()
                .and_then(|value| value.to_str())
                .map(|ext| matches!(ext, "yml" | "yaml"))
                .unwrap_or(false)
                && path
                    .strip_prefix(root)
                    .ok()
                    .map(|relative| relative.starts_with(Path::new(".github").join("workflows")))
                    .unwrap_or(false)
        })
        .cloned()
        .collect()
}

fn test_files(root: &Path, files: &[PathBuf], kind: ProjectKind) -> Vec<String> {
    files.iter()
        .filter_map(|path| {
            let relative = path.strip_prefix(root).ok()?;
            let display = relative.to_string_lossy().replace('\\', "/");
            let file_name = relative.file_name()?.to_string_lossy().to_lowercase();
            let relative_lower = display.to_lowercase();

            let matches = match kind {
                ProjectKind::Rust => {
                    relative.starts_with("tests")
                        || file_name.ends_with("_test.rs")
                        || display.ends_with(".rs")
                            && read_text(path).contains("#[cfg(test)]")
                }
                ProjectKind::Node => {
                    relative_lower.contains("__tests__/")
                        || file_name.contains(".test.")
                        || file_name.contains(".spec.")
                }
                ProjectKind::Python => {
                    relative_lower.starts_with("tests/")
                        || file_name.starts_with("test_") && file_name.ends_with(".py")
                }
                ProjectKind::Go => file_name.ends_with("_test.go"),
                ProjectKind::Unknown => {
                    relative_lower.starts_with("tests/")
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

fn collect_files(root: &Path, max_depth: usize) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_files_recursive(root, max_depth, &mut files)?;
    Ok(files)
}

fn collect_files_recursive(root: &Path, depth: usize, files: &mut Vec<PathBuf>) -> io::Result<()> {
    if depth == 0 {
        return Ok(());
    }

    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if is_ignored_dir(&name) {
                continue;
            }
            collect_files_recursive(&path, depth - 1, files)?;
        } else if path.is_file() {
            files.push(path);
        }
    }

    Ok(())
}

fn is_ignored_dir(name: &str) -> bool {
    matches!(
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

fn read_many(paths: &[PathBuf]) -> String {
    paths.iter()
        .map(|path| read_text(path))
        .collect::<Vec<String>>()
        .join("\n")
}

fn read_text(path: &Path) -> String {
    fs::read(path)
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .unwrap_or_default()
}

fn normalize(contents: &str) -> String {
    contents.to_lowercase()
}

fn contains_any(contents: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|pattern| contents.contains(pattern))
}

fn looks_placeholder(contents: &str) -> bool {
    contains_any(
        contents,
        &[
            "one-line value proposition",
            "add installation instructions here",
            "describe the bug clearly",
            "what is painful today",
            "explain the pain point this project solves",
            "add your test command here",
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

    #[test]
    fn score_for_empty_repository_is_zero() {
        let root = std::env::temp_dir().join("ossify-empty-audit");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create temp directory");

        let report = audit_repository(&root).expect("audit repository");
        assert_eq!(report.score, 0);
        assert_eq!(report.missing_count(), 11);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn placeholder_readme_is_partial_not_strong() {
        let root = std::env::temp_dir().join("ossify-partial-readme");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create temp directory");
        fs::write(
            root.join("README.md"),
            "# Demo\n\nOne-line value proposition for Demo.\n\nAdd installation instructions here.\n",
        )
        .expect("write readme");

        let report = audit_repository(&root).expect("audit repository");
        let readme = report
            .checks
            .iter()
            .find(|check| check.id == "readme")
            .expect("readme check");
        assert_eq!(readme.status, CheckStatus::Partial);

        let _ = fs::remove_dir_all(&root);
    }
}
