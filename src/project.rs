use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use toml::Value as TomlValue;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectKind {
    Rust,
    Node,
    Python,
    Go,
    Unknown,
}

impl ProjectKind {
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Rust => "Rust",
            Self::Node => "Node.js",
            Self::Python => "Python",
            Self::Go => "Go",
            Self::Unknown => "Unknown",
        }
    }

    pub fn package_ecosystem(self) -> Option<&'static str> {
        match self {
            Self::Rust => Some("cargo"),
            Self::Node => Some("npm"),
            Self::Python => Some("pip"),
            Self::Go => Some("gomod"),
            Self::Unknown => None,
        }
    }

    pub fn ci_keywords(self) -> &'static [&'static str] {
        match self {
            Self::Rust => &["cargo check", "cargo test", "cargo build", "cargo clippy"],
            Self::Node => &[
                "npm test",
                "pnpm test",
                "yarn test",
                "npm run build",
                "pnpm build",
                "yarn build",
            ],
            Self::Python => &["pytest", "python -m pytest", "ruff", "python -m build"],
            Self::Go => &["go test ./...", "go build ./...", "golangci-lint"],
            Self::Unknown => &[],
        }
    }

    pub fn build_keywords(self) -> &'static [&'static str] {
        match self {
            Self::Rust => &["cargo build", "cargo check"],
            Self::Node => &[
                "npm run build",
                "pnpm build",
                "yarn build",
                "next build",
                "vite build",
            ],
            Self::Python => &["python -m build", "pip wheel", "build"],
            Self::Go => &["go build ./...", "go build"],
            Self::Unknown => &["build"],
        }
    }

    pub fn lint_keywords(self) -> &'static [&'static str] {
        match self {
            Self::Rust => &["cargo clippy", "cargo fmt --check", "rustfmt"],
            Self::Node => &[
                "npm run lint",
                "pnpm lint",
                "yarn lint",
                "eslint",
                "prettier",
            ],
            Self::Python => &["ruff", "black --check", "flake8"],
            Self::Go => &["golangci-lint", "gofmt"],
            Self::Unknown => &["lint", "format"],
        }
    }

    pub fn format_keywords(self) -> &'static [&'static str] {
        match self {
            Self::Rust => &["cargo fmt --check", "rustfmt"],
            Self::Node => &["prettier", "npm run format", "pnpm format", "yarn format"],
            Self::Python => &["black", "ruff format"],
            Self::Go => &["gofmt"],
            Self::Unknown => &["format"],
        }
    }

    pub fn test_keywords(self) -> &'static [&'static str] {
        match self {
            Self::Rust => &["cargo test"],
            Self::Node => &["npm test", "pnpm test", "yarn test", "vitest", "jest"],
            Self::Python => &["pytest", "python -m pytest"],
            Self::Go => &["go test ./..."],
            Self::Unknown => &["test"],
        }
    }

    pub fn release_keywords(self) -> &'static [&'static str] {
        match self {
            Self::Rust => &["cargo publish", "cargo package"],
            Self::Node => &["npm publish", "pnpm publish", "yarn npm publish"],
            Self::Python => &["python -m build", "twine upload"],
            Self::Go => &["goreleaser", "go build"],
            Self::Unknown => &["publish", "release"],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RepoProfile {
    Library,
    Cli,
    App,
    Generic,
}

impl RepoProfile {
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Library => "library",
            Self::Cli => "cli",
            Self::App => "app",
            Self::Generic => "generic",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ProjectMetadata {
    pub description: Option<String>,
    pub license: Option<String>,
    pub repository: Option<String>,
    pub homepage: Option<String>,
    pub version: Option<String>,
    pub keywords: Vec<String>,
    pub categories: Vec<String>,
    pub scripts: Vec<String>,
    pub dependencies: Vec<String>,
    pub has_bin: bool,
    pub has_lib: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectContext {
    pub kind: ProjectKind,
    pub profile: RepoProfile,
    pub name: String,
    pub manifest_path: Option<PathBuf>,
    pub metadata: ProjectMetadata,
}

impl ProjectContext {
    pub fn summary(&self) -> String {
        match &self.manifest_path {
            Some(path) => format!(
                "{} {} via {}",
                self.kind.display_name(),
                self.profile.display_name(),
                path.display()
            ),
            None => format!(
                "{} {}",
                self.kind.display_name(),
                self.profile.display_name()
            ),
        }
    }

    pub fn with_profile_override(mut self, override_profile: Option<RepoProfile>) -> Self {
        if let Some(profile) = override_profile {
            self.profile = profile;
        }
        self
    }

    pub fn install_snippet(&self) -> String {
        match self.kind {
            ProjectKind::Rust => String::from("cargo build"),
            ProjectKind::Node => String::from("npm install"),
            ProjectKind::Python => {
                if self
                    .manifest_path
                    .as_ref()
                    .and_then(|path| path.file_name())
                    .and_then(|value| value.to_str())
                    .map(|name| name.eq_ignore_ascii_case("requirements.txt"))
                    .unwrap_or(false)
                {
                    String::from("python -m pip install -r requirements.txt")
                } else {
                    String::from("python -m pip install -e .")
                }
            }
            ProjectKind::Go => String::from("go build ./..."),
            ProjectKind::Unknown => {
                String::from("# add installation instructions for your project")
            }
        }
    }

    pub fn usage_snippet(&self) -> String {
        match (self.kind, self.profile) {
            (ProjectKind::Rust, RepoProfile::Library) => String::from("cargo test"),
            (ProjectKind::Rust, _) => format!("cargo run -- {}", self.default_usage_flag()),
            (ProjectKind::Node, RepoProfile::Library) => {
                format!("node -e \"require('{}')\"", self.name)
            }
            (ProjectKind::Node, _) => String::from("npm run start"),
            (ProjectKind::Python, RepoProfile::Library) => {
                format!("python -c \"import {}\"", self.module_name())
            }
            (ProjectKind::Python, _) => self
                .manifest_path
                .as_ref()
                .and_then(|path| path.parent())
                .and_then(|root| {
                    if root.join("app.py").is_file() {
                        Some(String::from("python app.py"))
                    } else if root.join("main.py").is_file() {
                        Some(String::from("python main.py"))
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| {
                    format!(
                        "python -m {} {}",
                        self.module_name(),
                        self.default_usage_flag()
                    )
                }),
            (ProjectKind::Go, RepoProfile::Library) => String::from("go test ./..."),
            (ProjectKind::Go, _) => format!("go run . {}", self.default_usage_flag()),
            (ProjectKind::Unknown, _) => {
                format!("{} {}", self.binary_name(), self.default_usage_flag())
            }
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

    pub fn lint_snippet(&self) -> String {
        match self.kind {
            ProjectKind::Rust => String::from("cargo clippy --all-targets --all-features"),
            ProjectKind::Node => String::from("npm run lint"),
            ProjectKind::Python => String::from("ruff check ."),
            ProjectKind::Go => String::from("golangci-lint run"),
            ProjectKind::Unknown => String::from("# add your lint command here"),
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

    pub fn script_mentions(&self, needle: &str) -> bool {
        self.metadata
            .scripts
            .iter()
            .any(|script| script.contains(needle))
    }

    pub fn dependency_mentions(&self, needle: &str) -> bool {
        self.metadata
            .dependencies
            .iter()
            .any(|dependency| dependency.contains(needle))
    }

    fn default_usage_flag(&self) -> &'static str {
        if matches!(self.profile, RepoProfile::Library) {
            ""
        } else {
            "--help"
        }
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
            let (name, metadata) = match kind {
                ProjectKind::Rust => parse_rust_project(path, &contents, &fallback_name),
                ProjectKind::Node => parse_node_project(path, &contents, &fallback_name),
                ProjectKind::Python => parse_python_project(path, &contents, &fallback_name),
                ProjectKind::Go => parse_go_project(path, &contents, &fallback_name),
                ProjectKind::Unknown => (fallback_name.clone(), ProjectMetadata::default()),
            };
            let profile = detect_profile(path, kind, &metadata);

            return Ok(ProjectContext {
                kind,
                profile,
                name,
                manifest_path: Some(manifest_path),
                metadata,
            });
        }
    }

    let requirements_path = path.join("requirements.txt");
    if requirements_path.is_file() {
        let contents = read_text(&requirements_path)?;
        let (name, metadata) = parse_python_requirements_project(path, &contents, &fallback_name);
        let profile = detect_profile(path, ProjectKind::Python, &metadata);

        return Ok(ProjectContext {
            kind: ProjectKind::Python,
            profile,
            name,
            manifest_path: Some(requirements_path),
            metadata,
        });
    }

    Ok(ProjectContext {
        kind: ProjectKind::Unknown,
        profile: RepoProfile::Generic,
        name: fallback_name,
        manifest_path: None,
        metadata: ProjectMetadata::default(),
    })
}

fn parse_rust_project(
    root: &Path,
    contents: &str,
    fallback_name: &str,
) -> (String, ProjectMetadata) {
    let value = contents
        .parse::<TomlValue>()
        .unwrap_or_else(|_| TomlValue::Table(Default::default()));
    let package = value.get("package").and_then(|entry| entry.as_table());

    let mut metadata = ProjectMetadata::default();
    metadata.description = package
        .and_then(|table| table.get("description"))
        .and_then(as_toml_string);
    metadata.license = package
        .and_then(|table| table.get("license"))
        .and_then(as_toml_string);
    metadata.repository = package
        .and_then(|table| table.get("repository"))
        .and_then(as_toml_string);
    metadata.homepage = package
        .and_then(|table| table.get("homepage"))
        .and_then(as_toml_string);
    metadata.version = package
        .and_then(|table| table.get("version"))
        .and_then(as_toml_string);
    metadata.keywords = package
        .and_then(|table| table.get("keywords"))
        .map(as_toml_array_strings)
        .unwrap_or_default();
    metadata.categories = package
        .and_then(|table| table.get("categories"))
        .map(as_toml_array_strings)
        .unwrap_or_default();
    metadata.dependencies = collect_toml_keys(
        &value,
        &["dependencies", "dev-dependencies", "build-dependencies"],
    );
    metadata.has_bin = value.get("bin").is_some()
        || root.join("src/main.rs").is_file()
        || root.join("src/bin").is_dir();
    metadata.has_lib = value.get("lib").is_some() || root.join("src/lib.rs").is_file();

    let name = package
        .and_then(|table| table.get("name"))
        .and_then(as_toml_string)
        .unwrap_or_else(|| fallback_name.to_owned());

    (name, metadata)
}

fn parse_node_project(
    root: &Path,
    contents: &str,
    fallback_name: &str,
) -> (String, ProjectMetadata) {
    let value = serde_json::from_str::<JsonValue>(contents).unwrap_or(JsonValue::Null);
    let mut metadata = ProjectMetadata::default();
    metadata.description = value.get("description").and_then(as_json_string);
    metadata.license = value.get("license").and_then(as_json_string);
    metadata.repository = parse_node_repository(value.get("repository"));
    metadata.homepage = value.get("homepage").and_then(as_json_string);
    metadata.version = value.get("version").and_then(as_json_string);
    metadata.keywords = value
        .get("keywords")
        .map(as_json_array_strings)
        .unwrap_or_default();
    metadata.categories = Vec::new();
    metadata.scripts = value
        .get("scripts")
        .and_then(|entry| entry.as_object())
        .map(|table| {
            table
                .values()
                .filter_map(as_json_string)
                .collect::<Vec<String>>()
        })
        .unwrap_or_default();
    metadata.dependencies = collect_json_keys(
        &value,
        &[
            "dependencies",
            "devDependencies",
            "peerDependencies",
            "optionalDependencies",
        ],
    );
    metadata.has_bin = value.get("bin").is_some();
    metadata.has_lib = value.get("main").is_some()
        || value.get("exports").is_some()
        || value.get("types").is_some()
        || root.join("src/index.ts").is_file()
        || root.join("src/index.js").is_file();

    let name = value
        .get("name")
        .and_then(as_json_string)
        .unwrap_or_else(|| fallback_name.to_owned());

    (name, metadata)
}

fn parse_python_project(
    root: &Path,
    contents: &str,
    fallback_name: &str,
) -> (String, ProjectMetadata) {
    let value = contents
        .parse::<TomlValue>()
        .unwrap_or_else(|_| TomlValue::Table(Default::default()));
    let project = value.get("project").and_then(|entry| entry.as_table());
    let poetry = value
        .get("tool")
        .and_then(|entry| entry.get("poetry"))
        .and_then(|entry| entry.as_table());

    let mut metadata = ProjectMetadata::default();
    metadata.description = project
        .and_then(|table| table.get("description"))
        .and_then(as_toml_string)
        .or_else(|| {
            poetry
                .and_then(|table| table.get("description"))
                .and_then(as_toml_string)
        });
    metadata.license = project
        .and_then(|table| table.get("license"))
        .and_then(as_toml_string)
        .or_else(|| {
            poetry
                .and_then(|table| table.get("license"))
                .and_then(as_toml_string)
        });
    metadata.repository = project
        .and_then(|table| table.get("repository"))
        .and_then(as_toml_string)
        .or_else(|| {
            poetry
                .and_then(|table| table.get("repository"))
                .and_then(as_toml_string)
        });
    metadata.homepage = project
        .and_then(|table| table.get("homepage"))
        .and_then(as_toml_string)
        .or_else(|| {
            poetry
                .and_then(|table| table.get("homepage"))
                .and_then(as_toml_string)
        });
    metadata.version = project
        .and_then(|table| table.get("version"))
        .and_then(as_toml_string)
        .or_else(|| {
            poetry
                .and_then(|table| table.get("version"))
                .and_then(as_toml_string)
        });
    metadata.keywords = project
        .and_then(|table| table.get("keywords"))
        .map(as_toml_array_strings)
        .or_else(|| {
            poetry
                .and_then(|table| table.get("keywords"))
                .map(as_toml_array_strings)
        })
        .unwrap_or_default();
    metadata.categories = Vec::new();
    metadata.scripts = collect_python_scripts(&value);
    metadata.dependencies = collect_python_dependencies(&value);
    metadata.has_bin = value
        .get("project")
        .and_then(|entry| entry.get("scripts"))
        .is_some()
        || value
            .get("tool")
            .and_then(|entry| entry.get("poetry"))
            .and_then(|entry| entry.get("scripts"))
            .is_some();
    metadata.has_lib = root.join("src").is_dir()
        || root.join(fallback_name.replace('-', "_")).is_dir()
        || root
            .join(format!("{}.py", fallback_name.replace('-', "_")))
            .is_file();

    let name = project
        .and_then(|table| table.get("name"))
        .and_then(as_toml_string)
        .or_else(|| {
            poetry
                .and_then(|table| table.get("name"))
                .and_then(as_toml_string)
        })
        .unwrap_or_else(|| fallback_name.to_owned());

    (name, metadata)
}

fn parse_python_requirements_project(
    root: &Path,
    contents: &str,
    fallback_name: &str,
) -> (String, ProjectMetadata) {
    let mut metadata = ProjectMetadata::default();
    metadata.dependencies = contents
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| {
            let requirement = line
                .split_once('#')
                .map(|(prefix, _)| prefix.trim())
                .unwrap_or(line);
            let package = requirement
                .split(['=', '<', '>', '!', '~', '['])
                .next()
                .unwrap_or_default()
                .trim()
                .to_lowercase();
            if package.is_empty() {
                None
            } else {
                Some(package)
            }
        })
        .collect();
    metadata.scripts = collect_launcher_scripts(root);
    metadata.has_bin = false;
    metadata.has_lib = root.join("src").is_dir()
        || root.join(fallback_name.replace('-', "_")).is_dir()
        || root
            .join(format!("{}.py", fallback_name.replace('-', "_")))
            .is_file();

    (fallback_name.to_owned(), metadata)
}

fn parse_go_project(root: &Path, contents: &str, fallback_name: &str) -> (String, ProjectMetadata) {
    let mut metadata = ProjectMetadata::default();
    metadata.dependencies = parse_go_dependencies(contents);
    metadata.has_bin = root.join("cmd").is_dir() || root.join("main.go").is_file();
    metadata.has_lib = fs::read_dir(root)
        .ok()
        .into_iter()
        .flat_map(|entries| entries.filter_map(Result::ok))
        .any(|entry| {
            entry
                .path()
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext == "go")
                .unwrap_or(false)
        });

    let module = parse_go_module(contents).unwrap_or_else(|| fallback_name.to_owned());
    let name = module
        .split('/')
        .last()
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback_name)
        .to_owned();

    metadata.repository = parse_go_repository(&module);

    (name, metadata)
}

fn detect_profile(root: &Path, kind: ProjectKind, metadata: &ProjectMetadata) -> RepoProfile {
    match kind {
        ProjectKind::Rust => {
            if metadata.has_lib && !metadata.has_bin {
                RepoProfile::Library
            } else if metadata.has_bin {
                RepoProfile::Cli
            } else {
                RepoProfile::Generic
            }
        }
        ProjectKind::Node => {
            if metadata.has_bin {
                RepoProfile::Cli
            } else if node_looks_like_app(root, metadata) {
                RepoProfile::App
            } else if metadata.has_lib {
                RepoProfile::Library
            } else {
                RepoProfile::Generic
            }
        }
        ProjectKind::Python => {
            if metadata.has_bin {
                RepoProfile::Cli
            } else if python_looks_like_app(root, metadata) {
                RepoProfile::App
            } else if metadata.has_lib {
                RepoProfile::Library
            } else {
                RepoProfile::Generic
            }
        }
        ProjectKind::Go => {
            if root.join("cmd").is_dir() {
                RepoProfile::Cli
            } else if root.join("main.go").is_file() {
                RepoProfile::App
            } else if metadata.has_lib {
                RepoProfile::Library
            } else {
                RepoProfile::Generic
            }
        }
        ProjectKind::Unknown => RepoProfile::Generic,
    }
}

fn node_looks_like_app(root: &Path, metadata: &ProjectMetadata) -> bool {
    let app_dependencies = [
        "next", "react", "express", "fastify", "nest", "vite", "electron",
    ];
    metadata.dependencies.iter().any(|dependency| {
        app_dependencies
            .iter()
            .any(|candidate| dependency.contains(candidate))
    }) || metadata.scripts.iter().any(|script| {
        script.contains("next ") || script.contains("vite ") || script.contains("start")
    }) || root.join("pages").is_dir()
        || root.join("app").is_dir()
        || root.join("server.js").is_file()
        || root.join("src/server.ts").is_file()
}

fn python_looks_like_app(root: &Path, metadata: &ProjectMetadata) -> bool {
    let app_dependencies = ["fastapi", "flask", "django", "streamlit", "uvicorn"];
    metadata.dependencies.iter().any(|dependency| {
        app_dependencies
            .iter()
            .any(|candidate| dependency.contains(candidate))
    }) || root.join("app.py").is_file()
        || root.join("manage.py").is_file()
        || root.join("main.py").is_file()
}

fn read_text(path: &Path) -> io::Result<String> {
    let bytes = fs::read(path)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn parse_node_repository(value: Option<&JsonValue>) -> Option<String> {
    match value {
        Some(JsonValue::String(value)) => Some(value.to_owned()),
        Some(JsonValue::Object(map)) => map
            .get("url")
            .and_then(as_json_string)
            .or_else(|| map.get("directory").and_then(as_json_string)),
        _ => None,
    }
}

fn parse_go_repository(module: &str) -> Option<String> {
    if module.starts_with("github.com/") || module.starts_with("gitlab.com/") {
        Some(format!("https://{module}"))
    } else {
        None
    }
}

fn collect_python_scripts(value: &TomlValue) -> Vec<String> {
    let mut scripts = BTreeSet::new();
    if let Some(entries) = value
        .get("project")
        .and_then(|entry| entry.get("scripts"))
        .and_then(|entry| entry.as_table())
    {
        for script in entries.values().filter_map(as_toml_string) {
            scripts.insert(script);
        }
    }

    if let Some(entries) = value
        .get("tool")
        .and_then(|entry| entry.get("poetry"))
        .and_then(|entry| entry.get("scripts"))
        .and_then(|entry| entry.as_table())
    {
        for script in entries.values().filter_map(as_toml_string) {
            scripts.insert(script);
        }
    }

    scripts.into_iter().collect()
}

fn collect_python_dependencies(value: &TomlValue) -> Vec<String> {
    let mut dependencies = BTreeSet::new();

    if let Some(entries) = value
        .get("project")
        .and_then(|entry| entry.get("dependencies"))
        .and_then(|entry| entry.as_array())
    {
        for dependency in entries.iter().filter_map(as_toml_string) {
            dependencies.insert(dependency_name(&dependency));
        }
    }

    if let Some(entries) = value
        .get("tool")
        .and_then(|entry| entry.get("poetry"))
        .and_then(|entry| entry.get("dependencies"))
        .and_then(|entry| entry.as_table())
    {
        for dependency in entries.keys() {
            if dependency != "python" {
                dependencies.insert(dependency.to_lowercase());
            }
        }
    }

    dependencies.into_iter().collect()
}

fn collect_launcher_scripts(root: &Path) -> Vec<String> {
    let mut scripts = BTreeSet::new();
    for candidate in ["run_game.bat", "run_game.sh", "start.bat", "start.sh"] {
        let path = root.join(candidate);
        if path.is_file() {
            scripts.insert(candidate.to_owned());
        }
    }
    scripts.into_iter().collect()
}

fn parse_go_module(contents: &str) -> Option<String> {
    contents
        .lines()
        .find_map(|line| line.trim().strip_prefix("module "))
        .map(|value| value.trim().trim_matches('"').to_owned())
}

fn parse_go_dependencies(contents: &str) -> Vec<String> {
    contents
        .lines()
        .filter_map(|line| line.trim().strip_prefix("require "))
        .map(|value| {
            value
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .to_lowercase()
        })
        .filter(|value| !value.is_empty())
        .collect()
}

fn collect_toml_keys(value: &TomlValue, keys: &[&str]) -> Vec<String> {
    let mut entries = BTreeSet::new();
    for key in keys {
        if let Some(table) = value.get(*key).and_then(|entry| entry.as_table()) {
            for dependency in table.keys() {
                entries.insert(dependency.to_lowercase());
            }
        }
    }
    entries.into_iter().collect()
}

fn collect_json_keys(value: &JsonValue, keys: &[&str]) -> Vec<String> {
    let mut entries = BTreeSet::new();
    for key in keys {
        if let Some(table) = value.get(*key).and_then(|entry| entry.as_object()) {
            for dependency in table.keys() {
                entries.insert(dependency.to_lowercase());
            }
        }
    }
    entries.into_iter().collect()
}

fn dependency_name(value: &str) -> String {
    value
        .split(|ch: char| matches!(ch, ' ' | '<' | '>' | '=' | '!' | '[' | ';'))
        .next()
        .unwrap_or(value)
        .trim()
        .to_lowercase()
}

fn as_toml_string(value: &TomlValue) -> Option<String> {
    value.as_str().map(str::to_owned)
}

fn as_json_string(value: &JsonValue) -> Option<String> {
    value.as_str().map(str::to_owned)
}

fn as_toml_array_strings(value: &TomlValue) -> Vec<String> {
    value
        .as_array()
        .into_iter()
        .flat_map(|entries| entries.iter())
        .filter_map(as_toml_string)
        .collect()
}

fn as_json_array_strings(value: &JsonValue) -> Vec<String> {
    value
        .as_array()
        .into_iter()
        .flat_map(|entries| entries.iter())
        .filter_map(as_json_string)
        .collect()
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
    fn detects_rust_library_profile() {
        let root = temp_repo("ossify-detect-rust-lib");
        fs::create_dir_all(root.join("src")).expect("create src");
        fs::write(
            root.join("Cargo.toml"),
            r#"[package]
name = "crate-kit"
description = "Toolkit"
license = "MIT"
repository = "https://github.com/acme/crate-kit"
homepage = "https://example.com"
keywords = ["cli", "tooling"]
categories = ["development-tools"]
version = "0.1.0"
"#,
        )
        .expect("write Cargo.toml");
        fs::write(root.join("src/lib.rs"), "pub fn demo() {}\n").expect("write lib.rs");

        let project = detect_project(&root).expect("detect project");
        assert_eq!(project.kind, ProjectKind::Rust);
        assert_eq!(project.profile, RepoProfile::Library);
        assert_eq!(project.metadata.license.as_deref(), Some("MIT"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn detects_node_cli_profile() {
        let root = temp_repo("ossify-detect-node-cli");
        fs::write(
            root.join("package.json"),
            r#"{
  "name": "shipit",
  "bin": "./bin/shipit.js",
  "description": "CLI",
  "license": "MIT",
  "repository": "https://github.com/acme/shipit",
  "homepage": "https://example.com",
  "version": "0.1.0",
  "keywords": ["cli"],
  "scripts": {
    "test": "vitest",
    "lint": "eslint ."
  },
  "dependencies": {
    "chalk": "^5.0.0"
  }
}"#,
        )
        .expect("write package.json");

        let project = detect_project(&root).expect("detect project");
        assert_eq!(project.kind, ProjectKind::Node);
        assert_eq!(project.profile, RepoProfile::Cli);
        assert!(project.script_mentions("vitest"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn detects_node_app_profile() {
        let root = temp_repo("ossify-detect-node-app");
        fs::create_dir_all(root.join("app")).expect("create app");
        fs::write(
            root.join("package.json"),
            r#"{
  "name": "dashkit",
  "scripts": {
    "start": "next dev"
  },
  "dependencies": {
    "next": "^15.0.0",
    "react": "^19.0.0"
  }
}"#,
        )
        .expect("write package.json");

        let project = detect_project(&root).expect("detect project");
        assert_eq!(project.kind, ProjectKind::Node);
        assert_eq!(project.profile, RepoProfile::App);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn detects_python_app_profile() {
        let root = temp_repo("ossify-detect-python-app");
        fs::write(
            root.join("pyproject.toml"),
            r#"[project]
name = "webdock"
description = "App"
license = "MIT"
repository = "https://github.com/acme/webdock"
homepage = "https://example.com"
version = "0.1.0"
dependencies = ["fastapi>=0.1"]
"#,
        )
        .expect("write pyproject");
        fs::write(root.join("app.py"), "print('hi')\n").expect("write app.py");

        let project = detect_project(&root).expect("detect project");
        assert_eq!(project.kind, ProjectKind::Python);
        assert_eq!(project.profile, RepoProfile::App);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn detects_python_app_from_requirements_txt() {
        let root = temp_repo("ossify-detect-python-requirements-app");
        fs::create_dir_all(root.join("templates")).expect("create templates");
        fs::create_dir_all(root.join("static")).expect("create static");
        fs::write(
            root.join("requirements.txt"),
            "flask\nflask-socketio\neventlet\n",
        )
        .expect("write requirements");
        fs::write(root.join("app.py"), "print('hi')\n").expect("write app.py");
        fs::write(root.join("run_game.bat"), "@echo off\npython app.py\n").expect("write bat");

        let project = detect_project(&root).expect("detect project");
        assert_eq!(project.kind, ProjectKind::Python);
        assert_eq!(project.profile, RepoProfile::App);
        assert_eq!(
            project
                .manifest_path
                .as_ref()
                .and_then(|path| path.file_name())
                .and_then(|value| value.to_str()),
            Some("requirements.txt")
        );
        assert!(project
            .metadata
            .dependencies
            .iter()
            .any(|dep| dep == "flask"));
        assert_eq!(
            project.install_snippet(),
            "python -m pip install -r requirements.txt"
        );
        assert_eq!(project.usage_snippet(), "python app.py");

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn detects_python_cli_profile() {
        let root = temp_repo("ossify-detect-python-cli");
        fs::write(
            root.join("pyproject.toml"),
            r#"[project]
name = "shiprun"
version = "0.1.0"

[project.scripts]
shiprun = "shiprun.cli:main"
"#,
        )
        .expect("write pyproject");

        let project = detect_project(&root).expect("detect project");
        assert_eq!(project.kind, ProjectKind::Python);
        assert_eq!(project.profile, RepoProfile::Cli);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn detects_go_cli_profile() {
        let root = temp_repo("ossify-detect-go-cli");
        fs::create_dir_all(root.join("cmd/demo")).expect("create cmd");
        fs::write(root.join("go.mod"), "module github.com/acme/demo\n").expect("write go.mod");

        let project = detect_project(&root).expect("detect project");
        assert_eq!(project.kind, ProjectKind::Go);
        assert_eq!(project.profile, RepoProfile::Cli);
        assert_eq!(
            project.metadata.repository.as_deref(),
            Some("https://github.com/acme/demo")
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn detects_go_library_profile() {
        let root = temp_repo("ossify-detect-go-lib");
        fs::write(root.join("go.mod"), "module github.com/acme/toolkit\n").expect("write go.mod");
        fs::write(root.join("toolkit.go"), "package toolkit\n").expect("write toolkit.go");

        let project = detect_project(&root).expect("detect project");
        assert_eq!(project.kind, ProjectKind::Go);
        assert_eq!(project.profile, RepoProfile::Library);

        let _ = fs::remove_dir_all(&root);
    }
}
