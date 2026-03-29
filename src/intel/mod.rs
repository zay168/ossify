pub mod explain;
pub mod history;
pub mod index;
pub mod inference;
pub mod knowledge;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum CacheState {
    #[default]
    Cold,
    Warm,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChunkKind {
    Manifest,
    ManifestSection,
    MarkdownDocument,
    MarkdownHeading,
    MarkdownCodeFence,
    Workflow,
    WorkflowJob,
    WorkflowStep,
    Script,
    TestPath,
    ExamplePath,
    DocsPath,
    FilePath,
}

impl ChunkKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Manifest => "manifest",
            Self::ManifestSection => "manifest-section",
            Self::MarkdownDocument => "markdown-document",
            Self::MarkdownHeading => "markdown-heading",
            Self::MarkdownCodeFence => "markdown-code-fence",
            Self::Workflow => "workflow",
            Self::WorkflowJob => "workflow-job",
            Self::WorkflowStep => "workflow-step",
            Self::Script => "script",
            Self::TestPath => "test-path",
            Self::ExamplePath => "example-path",
            Self::DocsPath => "docs-path",
            Self::FilePath => "file-path",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextRef {
    pub path: PathBuf,
    pub chunk_kind: ChunkKind,
    pub byte_start: Option<u32>,
    pub byte_end: Option<u32>,
    pub line_start: Option<u32>,
    pub line_end: Option<u32>,
    pub approximate: bool,
    pub excerpt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryRef {
    pub kind: String,
    pub id: String,
    pub summary: String,
    pub commit_time: Option<i64>,
    pub path: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProofKind {
    Satisfied,
    Missing,
    Contradiction,
    Historical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofItem {
    pub expectation: String,
    pub kind: ProofKind,
    pub weight: u16,
    pub confidence: f32,
    pub detail: String,
    pub context: Vec<ContextRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootCause {
    pub id: String,
    pub title: String,
    pub detail: String,
    pub impact: f32,
    pub context: Vec<ContextRef>,
    pub history: Vec<HistoryRef>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RetrievalScope {
    pub consulted_paths: Vec<String>,
    pub chunk_kinds: Vec<String>,
    pub used_history: bool,
    pub cache_state: CacheState,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConfidenceBreakdown {
    pub support_score: f32,
    pub penalty_score: f32,
    pub total_required_weight: f32,
    pub derived_coverage: u8,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuleIntelligence {
    pub primary_cause: Option<RootCause>,
    pub secondary_causes: Vec<RootCause>,
    pub causes: Vec<RootCause>,
    pub proof: Vec<ProofItem>,
    pub context_refs: Vec<ContextRef>,
    pub retrieval_scope: RetrievalScope,
    pub history_refs: Vec<HistoryRef>,
    pub confidence_breakdown: ConfidenceBreakdown,
}
