use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;
use pulldown_cmark::{Event, Parser, Tag};
use redb::{Database, TableDefinition};
use serde::{Deserialize, Serialize};

use crate::project::{ProjectContext, ProjectKind};

use super::history::HistorySnapshot;
use super::{CacheState, ChunkKind, ContextRef};

const MAX_INDEXED_FILE_BYTES: u64 = 512 * 1024;
const CACHE_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("repo-index");
const CACHE_SCHEMA_VERSION: &str = "v5.0.2";

#[derive(Debug, Clone)]
pub struct RepoIndex {
    root: PathBuf,
    pub files: Vec<PathBuf>,
    file_texts: BTreeMap<PathBuf, String>,
    chunks: Vec<IndexedChunk>,
    cache_state: CacheState,
    pub history: HistorySnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedChunk {
    pub id: String,
    pub path: PathBuf,
    pub relative_path: String,
    pub kind: ChunkKind,
    pub label: String,
    pub normalized_text: String,
    pub byte_start: u32,
    pub byte_end: u32,
    pub line_start: u32,
    pub line_end: u32,
    pub fingerprint: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct CachedIndexPayload {
    signature: String,
    chunks: Vec<IndexedChunk>,
}

impl RepoIndex {
    pub fn build(root: &Path, project: &ProjectContext) -> io::Result<Self> {
        let history = HistorySnapshot::read(root);
        let collected_files = collect_files(root)?;
        let mut files = Vec::new();
        let mut file_texts = BTreeMap::new();
        let mut signatures = Vec::new();

        for path in collected_files {
            let bytes = fs::read(&path)?;
            if looks_binary_bytes(&bytes) {
                continue;
            }
            let digest = blake3::hash(&bytes).to_hex().to_string();
            signatures.push(format!("{}:{digest}", relative_display(root, &path)));
            file_texts.insert(path.clone(), String::from_utf8_lossy(&bytes).into_owned());
            files.push(path);
        }

        let signature = signature_for(root, history.head_rev.as_deref(), &signatures);
        let cache_key = root.to_string_lossy().to_string();

        let (chunks, cache_state) = match load_cached_chunks(&cache_key, &signature) {
            Some(chunks) => (chunks, CacheState::Warm),
            None => {
                let chunks = build_chunks(root, project.kind, &files, &file_texts);
                store_cached_chunks(&cache_key, &signature, &chunks);
                (chunks, CacheState::Cold)
            }
        };

        Ok(Self {
            root: root.to_path_buf(),
            files,
            file_texts,
            chunks,
            cache_state,
            history,
        })
    }

    pub fn cache_state(&self) -> CacheState {
        self.cache_state
    }

    pub fn file_text(&self, path: &Path) -> Option<&str> {
        self.file_texts.get(path).map(String::as_str)
    }

    pub fn first_existing(&self, candidates: &[&str]) -> Option<PathBuf> {
        candidates
            .iter()
            .map(|candidate| self.root.join(candidate))
            .find(|candidate| self.file_texts.contains_key(candidate))
    }

    pub fn chunks(&self) -> &[IndexedChunk] {
        &self.chunks
    }

    pub fn workflow_files(&self) -> Vec<PathBuf> {
        self.files
            .iter()
            .filter(|path| {
                path.extension()
                    .and_then(|value| value.to_str())
                    .map(|ext| matches!(ext, "yml" | "yaml"))
                    .unwrap_or(false)
                    && path
                        .strip_prefix(&self.root)
                        .ok()
                        .map(|relative| {
                            relative.starts_with(Path::new(".github").join("workflows"))
                        })
                        .unwrap_or(false)
            })
            .cloned()
            .collect()
    }

    pub fn find_contexts(
        &self,
        related_paths: &[String],
        terms: &[String],
        limit: usize,
    ) -> Vec<ContextRef> {
        let normalized_terms = terms
            .iter()
            .map(|value| value.to_lowercase())
            .collect::<Vec<_>>();
        let normalized_paths = related_paths
            .iter()
            .map(|value| value.replace('\\', "/").to_lowercase())
            .collect::<Vec<_>>();

        let mut matches = self
            .chunks
            .iter()
            .filter(|chunk| {
                normalized_paths
                    .iter()
                    .any(|path| chunk.relative_path.to_lowercase().contains(path))
                    || normalized_terms
                        .iter()
                        .any(|term| chunk.normalized_text.contains(term))
            })
            .map(IndexedChunk::to_context_ref)
            .collect::<Vec<_>>();

        matches.truncate(limit.max(1));
        matches
    }

    pub fn find_nearest_context(&self, path: Option<&Path>, terms: &[String]) -> Vec<ContextRef> {
        if let Some(path) = path {
            let mut exact = self
                .chunks
                .iter()
                .filter(|chunk| chunk.path == path)
                .map(IndexedChunk::to_context_ref)
                .collect::<Vec<_>>();
            if !exact.is_empty() {
                exact.truncate(3);
                return exact;
            }
        }

        self.find_contexts(&Vec::new(), terms, 3)
    }
}

impl IndexedChunk {
    pub fn to_context_ref(&self) -> ContextRef {
        ContextRef {
            path: self.path.clone(),
            chunk_kind: self.kind,
            byte_start: Some(self.byte_start),
            byte_end: Some(self.byte_end),
            line_start: Some(self.line_start),
            line_end: Some(self.line_end),
            approximate: self.kind == ChunkKind::FilePath,
            excerpt: Some(truncate_excerpt(&self.normalized_text)),
        }
    }
}

fn collect_files(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut builder = WalkBuilder::new(root);
    builder.hidden(false);
    builder.git_ignore(true);
    builder.git_global(true);
    builder.git_exclude(true);

    for entry in builder.build() {
        let entry = entry.map_err(|error| io::Error::other(error.to_string()))?;
        if !entry
            .file_type()
            .map(|value| value.is_file())
            .unwrap_or(false)
        {
            continue;
        }

        let path = entry.into_path();
        let metadata = fs::metadata(&path)?;
        if metadata.len() > MAX_INDEXED_FILE_BYTES {
            continue;
        }
        if should_ignore_path(root, &path) || is_binary_extension(&path) {
            continue;
        }
        files.push(path);
    }

    files.sort();
    Ok(files)
}

fn should_ignore_path(root: &Path, path: &Path) -> bool {
    let relative = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
        .to_lowercase();
    let segments = relative.split('/').collect::<Vec<_>>();

    [
        "target",
        "node_modules",
        ".next",
        "dist",
        "build",
        ".venv",
        "venv",
        "__pycache__",
        ".pytest_cache",
        ".mypy_cache",
        ".ruff_cache",
        ".tox",
        ".nox",
        ".eggs",
    ]
    .iter()
    .any(|needle| segments.iter().any(|segment| segment == needle))
        || relative.ends_with(".egg-info")
}

fn is_binary_extension(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|value| value.to_str())
            .map(str::to_lowercase)
            .as_deref(),
        Some(
            "png"
                | "jpg"
                | "jpeg"
                | "gif"
                | "webp"
                | "ico"
                | "pdf"
                | "zip"
                | "gz"
                | "tar"
                | "pyc"
                | "pyo"
                | "so"
                | "dll"
                | "dylib"
                | "exe"
                | "class"
                | "woff"
                | "woff2"
                | "ttf"
        )
    )
}

fn looks_binary_bytes(bytes: &[u8]) -> bool {
    bytes.iter().take(2048).any(|byte| *byte == 0)
}

fn build_chunks(
    root: &Path,
    kind: ProjectKind,
    files: &[PathBuf],
    file_texts: &BTreeMap<PathBuf, String>,
) -> Vec<IndexedChunk> {
    let mut chunks = Vec::new();

    for path in files {
        let text = file_texts.get(path).cloned().unwrap_or_default();
        let relative = relative_display(root, path);
        let lower = relative.to_lowercase();
        let line_offsets = line_offsets(&text);

        if is_manifest_path(&lower) {
            chunks.extend(chunk_manifest(path, &relative, &text, &line_offsets));
        } else if lower.ends_with(".md") {
            chunks.extend(chunk_markdown(path, &relative, &text, &line_offsets));
        } else if lower.starts_with(".github/workflows/")
            && (lower.ends_with(".yml") || lower.ends_with(".yaml"))
        {
            chunks.extend(chunk_workflow(path, &relative, &text, &line_offsets));
        }

        if chunks
            .last()
            .map(|chunk| chunk.path != *path)
            .unwrap_or(true)
        {
            chunks.push(make_chunk(
                path,
                &relative,
                ChunkKind::FilePath,
                relative.clone(),
                &text,
                1,
                text.lines().count().max(1) as u32,
                0,
                text.len() as u32,
            ));
        }

        if lower.starts_with("tests/") || is_test_file(path, kind) {
            chunks.push(make_chunk(
                path,
                &relative,
                ChunkKind::TestPath,
                relative.clone(),
                &relative,
                1,
                1,
                0,
                relative.len() as u32,
            ));
        }
        if lower.starts_with("examples/") || lower.starts_with("example/") {
            chunks.push(make_chunk(
                path,
                &relative,
                ChunkKind::ExamplePath,
                relative.clone(),
                &relative,
                1,
                1,
                0,
                relative.len() as u32,
            ));
        }
        if lower.starts_with("docs/") {
            chunks.push(make_chunk(
                path,
                &relative,
                ChunkKind::DocsPath,
                relative.clone(),
                &relative,
                1,
                1,
                0,
                relative.len() as u32,
            ));
        }
        if lower.starts_with("scripts/")
            || matches!(
                path.extension().and_then(|value| value.to_str()),
                Some("sh" | "ps1" | "py" | "js" | "ts")
            )
        {
            chunks.push(make_chunk(
                path,
                &relative,
                ChunkKind::Script,
                relative.clone(),
                &text,
                1,
                text.lines().count().max(1) as u32,
                0,
                text.len() as u32,
            ));
        }
    }

    chunks
}

fn chunk_manifest(
    path: &Path,
    relative: &str,
    text: &str,
    line_offsets: &[u32],
) -> Vec<IndexedChunk> {
    let mut chunks = vec![make_chunk(
        path,
        relative,
        ChunkKind::Manifest,
        relative.to_owned(),
        text,
        1,
        text.lines().count().max(1) as u32,
        0,
        text.len() as u32,
    )];

    let lower = relative.to_lowercase();
    if lower.ends_with("cargo.toml") || lower.ends_with("pyproject.toml") {
        let sections = split_toml_sections(text);
        for section in sections {
            let slice = section.slice(text).to_owned();
            chunks.push(make_chunk(
                path,
                relative,
                ChunkKind::ManifestSection,
                section.label,
                &slice,
                section.line_start,
                section.line_end,
                line_offsets[(section.line_start - 1) as usize],
                line_offsets[section.line_end as usize],
            ));
        }
    } else if lower.ends_with("package.json") {
        for key in json_top_level_keys(text) {
            let slice = key.slice(text).to_owned();
            chunks.push(make_chunk(
                path,
                relative,
                ChunkKind::ManifestSection,
                key.label,
                &slice,
                key.line_start,
                key.line_end,
                line_offsets[(key.line_start - 1) as usize],
                line_offsets[key.line_end as usize],
            ));
        }
    } else if lower.ends_with("go.mod") {
        for block in split_go_blocks(text) {
            let slice = block.slice(text).to_owned();
            chunks.push(make_chunk(
                path,
                relative,
                ChunkKind::ManifestSection,
                block.label,
                &slice,
                block.line_start,
                block.line_end,
                line_offsets[(block.line_start - 1) as usize],
                line_offsets[block.line_end as usize],
            ));
        }
    }

    chunks
}

fn chunk_markdown(
    path: &Path,
    relative: &str,
    text: &str,
    line_offsets: &[u32],
) -> Vec<IndexedChunk> {
    let mut chunks = vec![make_chunk(
        path,
        relative,
        ChunkKind::MarkdownDocument,
        relative.to_owned(),
        text,
        1,
        text.lines().count().max(1) as u32,
        0,
        text.len() as u32,
    )];

    let _has_markdown_structure = Parser::new(text).any(|event| {
        matches!(
            event,
            Event::Start(Tag::Heading { .. }) | Event::Start(Tag::CodeBlock(_))
        )
    });

    for section in split_markdown_headings(text) {
        let slice = section.slice(text).to_owned();
        chunks.push(make_chunk(
            path,
            relative,
            ChunkKind::MarkdownHeading,
            section.label,
            &slice,
            section.line_start,
            section.line_end,
            line_offsets[(section.line_start - 1) as usize],
            line_offsets[section.line_end as usize],
        ));
    }
    for fence in markdown_code_fences(text) {
        let slice = fence.slice(text).to_owned();
        chunks.push(make_chunk(
            path,
            relative,
            ChunkKind::MarkdownCodeFence,
            fence.label,
            &slice,
            fence.line_start,
            fence.line_end,
            line_offsets[(fence.line_start - 1) as usize],
            line_offsets[fence.line_end as usize],
        ));
    }

    chunks
}

fn chunk_workflow(
    path: &Path,
    relative: &str,
    text: &str,
    line_offsets: &[u32],
) -> Vec<IndexedChunk> {
    let mut chunks = vec![make_chunk(
        path,
        relative,
        ChunkKind::Workflow,
        relative.to_owned(),
        text,
        1,
        text.lines().count().max(1) as u32,
        0,
        text.len() as u32,
    )];

    for section in split_yaml_jobs(text) {
        let slice = section.slice(text).to_owned();
        chunks.push(make_chunk(
            path,
            relative,
            ChunkKind::WorkflowJob,
            section.label,
            &slice,
            section.line_start,
            section.line_end,
            line_offsets[(section.line_start - 1) as usize],
            line_offsets[section.line_end as usize],
        ));
    }
    for step in split_workflow_steps(text) {
        let slice = step.slice(text).to_owned();
        chunks.push(make_chunk(
            path,
            relative,
            ChunkKind::WorkflowStep,
            step.label,
            &slice,
            step.line_start,
            step.line_end,
            line_offsets[(step.line_start - 1) as usize],
            line_offsets[step.line_end as usize],
        ));
    }

    chunks
}

#[allow(clippy::too_many_arguments)]
fn make_chunk(
    path: &Path,
    relative: &str,
    kind: ChunkKind,
    label: String,
    text: &str,
    line_start: u32,
    line_end: u32,
    byte_start: u32,
    byte_end: u32,
) -> IndexedChunk {
    let normalized_text = text.to_lowercase();
    IndexedChunk {
        id: format!("{relative}:{}:{line_start}", kind.as_str()),
        path: path.to_path_buf(),
        relative_path: relative.to_owned(),
        kind,
        label,
        fingerprint: blake3::hash(normalized_text.as_bytes())
            .to_hex()
            .to_string(),
        normalized_text,
        byte_start,
        byte_end,
        line_start,
        line_end,
    }
}

fn signature_for(root: &Path, head_rev: Option<&str>, signatures: &[String]) -> String {
    let mut payload = format!("{CACHE_SCHEMA_VERSION}:{}", root.to_string_lossy());
    if let Some(head) = head_rev {
        payload.push_str(head);
    }
    for signature in signatures {
        payload.push_str(signature);
    }
    blake3::hash(payload.as_bytes()).to_hex().to_string()
}

fn cache_database_path(cache_key: &str) -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("ossify"));
    let cache_id = blake3::hash(cache_key.as_bytes()).to_hex().to_string();
    base.join("ossify")
        .join("repo-index")
        .join(format!("{cache_id}.redb"))
}

fn load_cached_chunks(cache_key: &str, signature: &str) -> Option<Vec<IndexedChunk>> {
    let path = cache_database_path(cache_key);
    let database = Database::create(path).ok()?;
    let read = database.begin_read().ok()?;
    let table = read.open_table(CACHE_TABLE).ok()?;
    let bytes = table.get(cache_key).ok()??;
    let payload = serde_json::from_slice::<CachedIndexPayload>(bytes.value()).ok()?;
    if payload.signature == signature {
        Some(payload.chunks)
    } else {
        None
    }
}

fn store_cached_chunks(cache_key: &str, signature: &str, chunks: &[IndexedChunk]) {
    let path = cache_database_path(cache_key);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let Ok(database) = Database::create(path) else {
        return;
    };
    let payload = CachedIndexPayload {
        signature: signature.to_owned(),
        chunks: chunks.to_vec(),
    };
    let Ok(bytes) = serde_json::to_vec(&payload) else {
        return;
    };

    let Ok(write) = database.begin_write() else {
        return;
    };
    {
        let Ok(mut table) = write.open_table(CACHE_TABLE) else {
            return;
        };
        let _ = table.insert(cache_key, bytes.as_slice());
    }
    let _ = write.commit();
}

#[derive(Debug)]
struct TextSection {
    label: String,
    line_start: u32,
    line_end: u32,
}

impl TextSection {
    fn slice<'a>(&self, text: &'a str) -> &'a str {
        slice_lines(text, self.line_start, self.line_end)
    }
}

fn split_toml_sections(text: &str) -> Vec<TextSection> {
    let mut starts = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            starts.push((
                index as u32 + 1,
                trimmed.trim_matches(&['[', ']'][..]).to_owned(),
            ));
        }
    }
    finalize_sections(text, starts)
}

fn json_top_level_keys(text: &str) -> Vec<TextSection> {
    let mut starts = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with('"') {
            continue;
        }
        if let Some((key, _)) = trimmed[1..].split_once('"') {
            starts.push((index as u32 + 1, key.to_owned()));
        }
    }
    finalize_sections(text, starts)
}

fn split_go_blocks(text: &str) -> Vec<TextSection> {
    let mut starts = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("module ")
            || trimmed.starts_with("require")
            || trimmed.starts_with("replace")
        {
            starts.push((index as u32 + 1, trimmed.to_owned()));
        }
    }
    finalize_sections(text, starts)
}

fn split_markdown_headings(text: &str) -> Vec<TextSection> {
    let mut starts = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            starts.push((
                index as u32 + 1,
                trimmed.trim_start_matches('#').trim().to_owned(),
            ));
        }
    }
    finalize_sections(text, starts)
}

fn markdown_code_fences(text: &str) -> Vec<TextSection> {
    let mut fences = Vec::new();
    let mut open: Option<(u32, String)> = None;
    for (index, line) in text.lines().enumerate() {
        let line_no = index as u32 + 1;
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("```") {
            match open.take() {
                Some((start, label)) => fences.push(TextSection {
                    label,
                    line_start: start,
                    line_end: line_no,
                }),
                None => open = Some((line_no, format!("code fence {}", rest.trim()))),
            }
        }
    }
    fences
}

fn split_yaml_jobs(text: &str) -> Vec<TextSection> {
    let mut sections = Vec::new();
    let mut in_jobs = false;
    let mut starts = Vec::new();

    for (index, line) in text.lines().enumerate() {
        let line_no = index as u32 + 1;
        let trimmed = line.trim_start();
        if trimmed == "jobs:" {
            in_jobs = true;
            continue;
        }
        if in_jobs && !line.starts_with(' ') && !line.starts_with('\t') && !trimmed.is_empty() {
            in_jobs = false;
        }
        if in_jobs && line.starts_with("  ") && trimmed.ends_with(':') && !trimmed.starts_with('-')
        {
            starts.push((line_no, trimmed.trim_end_matches(':').to_owned()));
        }
    }

    sections.extend(finalize_sections(text, starts));
    sections
}

fn split_workflow_steps(text: &str) -> Vec<TextSection> {
    let mut sections = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let line_no = index as u32 + 1;
        let trimmed = line.trim_start();
        if trimmed.starts_with("- name:")
            || trimmed.starts_with("- run:")
            || trimmed.starts_with("run:")
        {
            sections.push(TextSection {
                label: trimmed.trim_start_matches("- ").to_owned(),
                line_start: line_no,
                line_end: line_no,
            });
        }
    }
    sections
}

fn finalize_sections(text: &str, starts: Vec<(u32, String)>) -> Vec<TextSection> {
    if starts.is_empty() {
        return Vec::new();
    }

    let total_lines = text.lines().count().max(1) as u32;
    starts
        .iter()
        .enumerate()
        .map(|(index, (line_start, label))| TextSection {
            label: label.clone(),
            line_start: *line_start,
            line_end: starts
                .get(index + 1)
                .map(|(line, _)| line.saturating_sub(1))
                .unwrap_or(total_lines),
        })
        .collect()
}

fn slice_lines(text: &str, line_start: u32, line_end: u32) -> &str {
    let offsets = line_offsets(text);
    let start = offsets
        .get((line_start.saturating_sub(1)) as usize)
        .copied()
        .unwrap_or(0) as usize;
    let end = offsets
        .get(line_end as usize)
        .copied()
        .unwrap_or(text.len() as u32) as usize;

    if start >= end || start > text.len() {
        ""
    } else {
        &text[start..end.min(text.len())]
    }
}

fn line_offsets(text: &str) -> Vec<u32> {
    let mut offsets = vec![0];
    for (index, byte) in text.as_bytes().iter().enumerate() {
        if *byte == b'\n' {
            offsets.push((index + 1) as u32);
        }
    }

    let end = text.len() as u32;
    if offsets.last().copied() != Some(end) {
        offsets.push(end);
    }

    offsets
}

fn is_manifest_path(relative: &str) -> bool {
    matches!(
        relative.to_lowercase().as_str(),
        "cargo.toml" | "package.json" | "pyproject.toml" | "go.mod"
    )
}

fn is_test_file(path: &Path, kind: ProjectKind) -> bool {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_lowercase();
    match kind {
        ProjectKind::Rust => file_name.ends_with("_test.rs"),
        ProjectKind::Node => file_name.contains(".test.") || file_name.contains(".spec."),
        ProjectKind::Python => file_name.starts_with("test_") && file_name.ends_with(".py"),
        ProjectKind::Go => file_name.ends_with("_test.go"),
        ProjectKind::Unknown => {
            file_name.contains(".test.")
                || file_name.contains(".spec.")
                || file_name.ends_with("_test.go")
                || file_name.starts_with("test_")
        }
    }
}

fn relative_display(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
        .replace('\\', "/")
}

fn truncate_excerpt(input: &str) -> String {
    let trimmed = input
        .split_whitespace()
        .take(18)
        .collect::<Vec<_>>()
        .join(" ");
    if trimmed.chars().count() > 160 {
        let prefix = trimmed.chars().take(160).collect::<String>();
        format!("{prefix}...")
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_TEMP_ID: AtomicUsize = AtomicUsize::new(0);

    fn temp_repo(name: &str) -> PathBuf {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("{name}-{id}"));
        let _ = fs::remove_dir_all(&path);
        let cache_key = path.to_string_lossy().to_string();
        let _ = fs::remove_file(cache_database_path(&cache_key));
        fs::create_dir_all(&path).expect("create temp directory");
        path
    }

    #[test]
    fn index_chunks_markdown_and_workflow_content() {
        let root = temp_repo("ossify-index-build");
        fs::create_dir_all(root.join(".github/workflows")).expect("create workflows");
        fs::write(
            root.join("README.md"),
            "# Demo\n\n## Install\n\n```bash\ncargo build\n```\n",
        )
        .expect("write README");
        fs::write(
            root.join(".github/workflows/ci.yml"),
            "name: CI\non:\n  pull_request:\n  push:\njobs:\n  verify:\n    steps:\n      - run: cargo test\n",
        )
        .expect("write workflow");

        let project = ProjectContext {
            kind: ProjectKind::Rust,
            profile: crate::project::RepoProfile::Cli,
            name: String::from("demo"),
            manifest_path: None,
            metadata: Default::default(),
        };
        let index = RepoIndex::build(&root, &project).expect("build index");

        assert!(index
            .chunks()
            .iter()
            .any(|chunk| chunk.kind == ChunkKind::MarkdownHeading));
        assert!(index
            .chunks()
            .iter()
            .any(|chunk| chunk.kind == ChunkKind::WorkflowStep));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn cache_warms_on_second_index_build() {
        let root = temp_repo("ossify-index-cache");
        fs::write(root.join("README.md"), "# Demo\n").expect("write README");
        let project = ProjectContext {
            kind: ProjectKind::Unknown,
            profile: crate::project::RepoProfile::Generic,
            name: String::from("demo"),
            manifest_path: None,
            metadata: Default::default(),
        };

        let cold = RepoIndex::build(&root, &project).expect("cold build");
        let warm = RepoIndex::build(&root, &project).expect("warm build");

        assert_eq!(cold.cache_state(), CacheState::Cold);
        assert_eq!(warm.cache_state(), CacheState::Warm);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn truncate_excerpt_handles_lossy_unicode_safely() {
        let input = format!("{}{}", "x".repeat(159), "���");
        let excerpt = truncate_excerpt(&input);
        assert!(excerpt.ends_with("..."));
        assert!(excerpt.is_char_boundary(excerpt.len()));
    }

    #[test]
    fn slice_lines_handles_crlf_and_multibyte_characters() {
        let text =
            "# React Best Practices\r\n\r\nLine with em dash \u{2014} detail\r\nAnother line\r\n";
        let slice = slice_lines(text, 3, 3);

        assert_eq!(slice, "Line with em dash \u{2014} detail\r\n");
        assert!(slice.is_char_boundary(slice.len()));
    }

    #[test]
    fn line_offsets_track_real_crlf_byte_positions() {
        let text = "a\r\nb\u{2014}c\r\n";
        let offsets = line_offsets(text);

        assert_eq!(offsets, vec![0, 3, 10]);
        assert_eq!(slice_lines(text, 2, 2), "b\u{2014}c\r\n");
    }
}
