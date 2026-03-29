use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckStatus {
    Present,
    Missing,
}

impl CheckStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Present => "present",
            Self::Missing => "missing",
        }
    }
}

#[derive(Debug, Clone)]
pub struct AuditCheck {
    pub id: &'static str,
    pub label: &'static str,
    pub weight: u8,
    pub status: CheckStatus,
    pub fixable: bool,
    pub hint: &'static str,
}

#[derive(Debug, Clone)]
pub struct AuditReport {
    pub target: PathBuf,
    pub score: u8,
    pub checks: Vec<AuditCheck>,
}

impl AuditReport {
    pub fn present_checks(&self) -> impl Iterator<Item = &AuditCheck> {
        self.checks
            .iter()
            .filter(|check| check.status == CheckStatus::Present)
    }

    pub fn missing_checks(&self) -> impl Iterator<Item = &AuditCheck> {
        self.checks
            .iter()
            .filter(|check| check.status == CheckStatus::Missing)
    }

    pub fn present_count(&self) -> usize {
        self.present_checks().count()
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

    let checks = vec![
        check_file(
            path,
            "readme",
            "README",
            &["README.md", "README"],
            15,
            true,
            "Add a README that explains the problem, install, and examples.",
        ),
        check_file(
            path,
            "license",
            "License",
            &["LICENSE", "LICENSE.md", "COPYING"],
            20,
            true,
            "Pick a clear license so adopters know how they can use the project.",
        ),
        check_file(
            path,
            "contributing_guide",
            "Contributing guide",
            &["CONTRIBUTING.md"],
            10,
            true,
            "Document the workflow for contributors and new maintainers.",
        ),
        check_file(
            path,
            "code_of_conduct",
            "Code of conduct",
            &["CODE_OF_CONDUCT.md"],
            10,
            true,
            "Signal healthy community standards from day one.",
        ),
        check_file(
            path,
            "security_policy",
            "Security policy",
            &["SECURITY.md"],
            10,
            true,
            "Tell users how to report vulnerabilities responsibly.",
        ),
        check_file(
            path,
            "changelog",
            "Changelog",
            &["CHANGELOG.md"],
            8,
            true,
            "Make releases and project evolution easier to trust.",
        ),
        check_directory(
            path,
            "issue_templates",
            "Issue templates",
            &[".github/ISSUE_TEMPLATE"],
            8,
            true,
            "Issue templates raise bug report quality fast.",
        ),
        check_file(
            path,
            "pull_request_template",
            "Pull request template",
            &[".github/PULL_REQUEST_TEMPLATE.md"],
            7,
            true,
            "A good PR template keeps reviews focused.",
        ),
        check_directory(
            path,
            "ci_workflow",
            "CI workflow",
            &[".github/workflows"],
            7,
            true,
            "Automation increases confidence in the project.",
        ),
        check_any(
            path,
            "project_manifest",
            "Project manifest",
            &["Cargo.toml", "package.json", "pyproject.toml", "go.mod"],
            5,
            false,
            "A detected manifest helps `ossify` infer project type later.",
        ),
    ];

    let total: u16 = checks.iter().map(|check| u16::from(check.weight)).sum();
    let earned: u16 = checks
        .iter()
        .filter(|check| check.status == CheckStatus::Present)
        .map(|check| u16::from(check.weight))
        .sum();

    let score = ((earned * 100) / total) as u8;

    Ok(AuditReport {
        target: canonical,
        score,
        checks,
    })
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

fn check_file(
    path: &Path,
    id: &'static str,
    label: &'static str,
    candidates: &[&str],
    weight: u8,
    fixable: bool,
    hint: &'static str,
) -> AuditCheck {
    let status = if candidates
        .iter()
        .map(|candidate| path.join(candidate))
        .any(|candidate| candidate.is_file())
    {
        CheckStatus::Present
    } else {
        CheckStatus::Missing
    };

    AuditCheck {
        id,
        label,
        weight,
        status,
        fixable,
        hint,
    }
}

fn check_directory(
    path: &Path,
    id: &'static str,
    label: &'static str,
    candidates: &[&str],
    weight: u8,
    fixable: bool,
    hint: &'static str,
) -> AuditCheck {
    let status = if candidates
        .iter()
        .map(|candidate| path.join(candidate))
        .any(|candidate| candidate.is_dir())
    {
        CheckStatus::Present
    } else {
        CheckStatus::Missing
    };

    AuditCheck {
        id,
        label,
        weight,
        status,
        fixable,
        hint,
    }
}

fn check_any(
    path: &Path,
    id: &'static str,
    label: &'static str,
    candidates: &[&str],
    weight: u8,
    fixable: bool,
    hint: &'static str,
) -> AuditCheck {
    let status = if candidates
        .iter()
        .map(|candidate| path.join(candidate))
        .any(|candidate| candidate.exists())
    {
        CheckStatus::Present
    } else {
        CheckStatus::Missing
    };

    AuditCheck {
        id,
        label,
        weight,
        status,
        fixable,
        hint,
    }
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

        let _ = fs::remove_dir_all(&root);
    }
}
