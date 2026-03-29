use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectKind {
    Rust,
    Node,
    Python,
    Go,
    Unknown,
}

impl ProjectKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Node => "node",
            Self::Python => "python",
            Self::Go => "go",
            Self::Unknown => "unknown",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Rust => "Rust",
            Self::Node => "Node.js",
            Self::Python => "Python",
            Self::Go => "Go",
            Self::Unknown => "Unknown",
        }
    }

    pub fn ci_keywords(self) -> &'static [&'static str] {
        match self {
            Self::Rust => &["cargo check", "cargo test", "cargo build"],
            Self::Node => &["npm test", "pnpm test", "yarn test", "npm run build", "pnpm build", "yarn build"],
            Self::Python => &["pytest", "python -m pytest", "ruff", "uv run pytest"],
            Self::Go => &["go test ./...", "go build ./..."],
            Self::Unknown => &[],
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProjectContext {
    pub kind: ProjectKind,
    pub name: String,
    pub manifest_path: Option<PathBuf>,
}

impl ProjectContext {
    pub fn summary(&self) -> String {
        match &self.manifest_path {
            Some(path) => format!("{} via {}", self.kind.display_name(), path.display()),
            None => self.kind.display_name().to_owned(),
        }
    }

    pub fn install_snippet(&self) -> String {
        match self.kind {
            ProjectKind::Rust => String::from("cargo build"),
            ProjectKind::Node => String::from("npm install"),
            ProjectKind::Python => String::from("python -m pip install -e ."),
            ProjectKind::Go => String::from("go build ./..."),
            ProjectKind::Unknown => String::from("# add installation instructions for your project"),
        }
    }

    pub fn usage_snippet(&self) -> String {
        match self.kind {
            ProjectKind::Rust => String::from("cargo run -- --help"),
            ProjectKind::Node => String::from("npm run start"),
            ProjectKind::Python => format!("python -m {}", self.module_name()),
            ProjectKind::Go => String::from("go run ."),
            ProjectKind::Unknown => format!("{} --help", self.binary_name()),
        }
    }

    pub fn test_snippet(&self) -> String {
        match self.kind {
            ProjectKind::Rust => String::from("cargo test"),
            ProjectKind::Node => String::from("npm test"),
            ProjectKind::Python => String::from("python -m pytest"),
            ProjectKind::Go => String::from("go test ./..."),
            ProjectKind::Unknown => String::from("# add your test command here"),
        }
    }

    pub fn binary_name(&self) -> String {
        self.name
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                    ch.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect::<String>()
            .trim_matches('-')
            .to_owned()
    }

    pub fn module_name(&self) -> String {
        self.binary_name().replace('-', "_")
    }
}

pub fn detect_project(path: &Path) -> io::Result<ProjectContext> {
    let fallback_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("project")
        .to_owned();

    let manifests = [
        ("Cargo.toml", ProjectKind::Rust),
        ("package.json", ProjectKind::Node),
        ("pyproject.toml", ProjectKind::Python),
        ("go.mod", ProjectKind::Go),
    ];

    for (manifest, kind) in manifests {
        let manifest_path = path.join(manifest);
        if manifest_path.is_file() {
            let contents = read_text(&manifest_path)?;
            let parsed_name = match kind {
                ProjectKind::Rust => parse_toml_name_in_section(&contents, "package"),
                ProjectKind::Node => parse_json_name(&contents),
                ProjectKind::Python => parse_toml_name_in_section(&contents, "project")
                    .or_else(|| parse_toml_name_in_section(&contents, "tool.poetry")),
                ProjectKind::Go => parse_go_module(&contents),
                ProjectKind::Unknown => None,
            };

            return Ok(ProjectContext {
                kind,
                name: parsed_name.unwrap_or_else(|| fallback_name.clone()),
                manifest_path: Some(manifest_path),
            });
        }
    }

    Ok(ProjectContext {
        kind: ProjectKind::Unknown,
        name: fallback_name,
        manifest_path: None,
    })
}

fn read_text(path: &Path) -> io::Result<String> {
    let bytes = fs::read(path)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn parse_toml_name_in_section(contents: &str, section: &str) -> Option<String> {
    let mut in_section = false;

    for line in contents.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let current = trimmed.trim_matches(&['[', ']'][..]);
            in_section = current == section;
            continue;
        }

        if in_section && trimmed.starts_with("name") {
            return parse_quoted_value(trimmed);
        }
    }

    None
}

fn parse_json_name(contents: &str) -> Option<String> {
    let key = "\"name\"";
    let start = contents.find(key)?;
    let remainder = &contents[start + key.len()..];
    let colon = remainder.find(':')?;
    let after_colon = remainder[colon + 1..].trim_start();
    let quoted = after_colon.strip_prefix('"')?;
    let end = quoted.find('"')?;
    Some(quoted[..end].to_owned())
}

fn parse_go_module(contents: &str) -> Option<String> {
    contents
        .lines()
        .find_map(|line| line.trim().strip_prefix("module "))
        .map(|value| value.trim().trim_matches('"').to_owned())
}

fn parse_quoted_value(line: &str) -> Option<String> {
    let first_quote = line.find('"')?;
    let remainder = &line[first_quote + 1..];
    let second_quote = remainder.find('"')?;
    Some(remainder[..second_quote].to_owned())
}
