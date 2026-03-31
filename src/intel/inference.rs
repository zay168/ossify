use std::path::{Path, PathBuf};

use crate::audit::FindingSeverity;
use crate::project::ProjectContext;

use super::explain::finalize_rule_intelligence;
use super::index::RepoIndex;
use super::knowledge::RuleKnowledge;
use super::{ContextRef, ProofItem, ProofKind, RuleIntelligence};

#[derive(Debug, Clone)]
pub struct FindingSignal {
    pub id: String,
    pub severity: FindingSeverity,
    pub message: String,
    pub help: String,
    pub location: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct RuleInput {
    pub rule_id: &'static str,
    pub label: &'static str,
    pub coverage: u8,
    pub message: String,
    pub evidence: Vec<String>,
    pub findings: Vec<FindingSignal>,
    pub location: Option<PathBuf>,
}

pub fn infer_rule(
    input: &RuleInput,
    project: &ProjectContext,
    index: &RepoIndex,
    knowledge: &RuleKnowledge,
) -> RuleIntelligence {
    let mut proof = Vec::new();
    add_evidence_proof(input, index, knowledge, &mut proof);
    add_findings_proof(input, index, knowledge, &mut proof);

    match input.rule_id {
        "project_manifest" => infer_manifest_presence(input, project, index, knowledge, &mut proof),
        "manifest_metadata" => {
            infer_manifest_metadata(input, project, index, knowledge, &mut proof)
        }
        "readme" => infer_readme(input, index, knowledge, &mut proof),
        "ci_workflow" => infer_workflow(input, index, knowledge, &mut proof),
        "tests" => infer_tests(input, index, knowledge, &mut proof),
        "lint_and_format" => infer_lint_and_format(input, index, knowledge, &mut proof),
        "release_workflow" => infer_release(input, index, knowledge, &mut proof),
        "changelog" => infer_changelog(input, index, knowledge, &mut proof),
        _ => infer_generic(input, index, knowledge, &mut proof),
    }

    if proof.is_empty() {
        proof.push(ProofItem {
            expectation: format!("baseline {}", input.label),
            kind: if input.coverage >= 85 {
                ProofKind::Satisfied
            } else {
                ProofKind::Missing
            },
            weight: 4,
            confidence: 0.7,
            detail: input.message.clone(),
            context: contexts_for(
                index,
                knowledge,
                &input.evidence,
                input.location.as_deref(),
                3,
            ),
        });
    }

    let mut history_refs = index.history.refs_for_rule(input.rule_id);
    if matches!(input.rule_id, "release_workflow" | "changelog")
        && !index.history.recent_tags.is_empty()
    {
        proof.push(ProofItem {
            expectation: String::from("repository has release intent in git history"),
            kind: ProofKind::Historical,
            weight: 4,
            confidence: 0.8,
            detail: format!(
                "{} recent tag(s) detected in local git history.",
                index.history.recent_tags.len()
            ),
            context: Vec::new(),
        });
    }
    if matches!(
        input.rule_id,
        "readme" | "security_policy" | "contributing_guide"
    ) && input.coverage < 85
        && !index.history.recent_commits.is_empty()
    {
        proof.push(ProofItem {
            expectation: String::from("active repository history raises documentation expectations"),
            kind: ProofKind::Contradiction,
            weight: 3,
            confidence: 0.7,
            detail: format!(
                "{} recent commit(s) suggest the repo is active, but the documentation surface still lags behind.",
                index.history.recent_commits.len()
            ),
            context: Vec::new(),
        });
    }
    history_refs.truncate(6);

    finalize_rule_intelligence(proof, history_refs, index.cache_state())
}

fn add_evidence_proof(
    input: &RuleInput,
    index: &RepoIndex,
    knowledge: &RuleKnowledge,
    proof: &mut Vec<ProofItem>,
) {
    if input.evidence.is_empty() {
        return;
    }

    let contexts = contexts_for(
        index,
        knowledge,
        &input.evidence,
        input.location.as_deref(),
        3,
    );
    proof.push(ProofItem {
        expectation: format!("observable evidence for {}", input.label),
        kind: if input.coverage >= 85 {
            ProofKind::Satisfied
        } else {
            ProofKind::Historical
        },
        weight: 2,
        confidence: 0.6,
        detail: input.evidence.join(", "),
        context: contexts,
    });
}

fn add_findings_proof(
    input: &RuleInput,
    index: &RepoIndex,
    knowledge: &RuleKnowledge,
    proof: &mut Vec<ProofItem>,
) {
    for finding in &input.findings {
        proof.push(ProofItem {
            expectation: finding.id.replace('.', " "),
            kind: if matches!(finding.severity, FindingSeverity::Info)
                && finding.message.to_lowercase().contains("placeholder")
            {
                ProofKind::Contradiction
            } else if matches!(finding.severity, FindingSeverity::Info) {
                ProofKind::Missing
            } else {
                ProofKind::Contradiction
            },
            weight: severity_weight(finding.severity),
            confidence: severity_confidence(finding.severity),
            detail: format!("{} {}", finding.message, finding.help),
            context: contexts_for(
                index,
                knowledge,
                &[finding.message.clone(), finding.help.clone()],
                finding.location.as_deref().or(input.location.as_deref()),
                3,
            ),
        });
    }
}

fn infer_manifest_presence(
    input: &RuleInput,
    project: &ProjectContext,
    index: &RepoIndex,
    knowledge: &RuleKnowledge,
    proof: &mut Vec<ProofItem>,
) {
    let context = contexts_for(
        index,
        knowledge,
        std::slice::from_ref(&project.name),
        input.location.as_deref(),
        2,
    );
    proof.push(ProofItem {
        expectation: String::from("project manifest exists"),
        kind: if project.manifest_path.is_some() {
            ProofKind::Satisfied
        } else {
            ProofKind::Missing
        },
        weight: 6,
        confidence: 0.95,
        detail: match &project.manifest_path {
            Some(path) => format!("Detected manifest at {}", path.display()),
            None => String::from("No supported root manifest was found."),
        },
        context,
    });
}

fn infer_manifest_metadata(
    input: &RuleInput,
    project: &ProjectContext,
    index: &RepoIndex,
    knowledge: &RuleKnowledge,
    proof: &mut Vec<ProofItem>,
) {
    let manifest_path = input
        .location
        .as_deref()
        .or(project.manifest_path.as_deref());
    for (expectation, exists) in [
        ("description", project.metadata.description.is_some()),
        ("license", project.metadata.license.is_some()),
        ("repository", project.metadata.repository.is_some()),
        ("homepage", project.metadata.homepage.is_some()),
        ("version", project.metadata.version.is_some()),
        (
            "discoverability metadata",
            !project.metadata.keywords.is_empty() || !project.metadata.categories.is_empty(),
        ),
    ] {
        proof.push(ProofItem {
            expectation: format!("manifest {}", expectation),
            kind: if exists {
                ProofKind::Satisfied
            } else {
                ProofKind::Missing
            },
            weight: 3,
            confidence: 0.9,
            detail: if exists {
                format!("The manifest exposes {}.", expectation)
            } else {
                format!("The manifest is missing {}.", expectation)
            },
            context: contexts_for(
                index,
                knowledge,
                &[expectation.to_owned()],
                manifest_path,
                2,
            ),
        });
    }
}

fn infer_readme(
    input: &RuleInput,
    index: &RepoIndex,
    knowledge: &RuleKnowledge,
    proof: &mut Vec<ProofItem>,
) {
    let canonical = index.first_existing(&["README.md", "README"]);
    let alternate = index.first_existing(&["CONTEXT.md", "docs/README.md", "docs/index.md"]);
    proof.push(ProofItem {
        expectation: String::from("canonical repository README"),
        kind: if canonical.is_some() {
            ProofKind::Satisfied
        } else {
            ProofKind::Missing
        },
        weight: 5,
        confidence: 0.95,
        detail: canonical
            .as_ref()
            .map(|path| format!("Found README at {}", path.display()))
            .unwrap_or_else(|| {
                String::from("No canonical README was found in the repository root.")
            }),
        context: contexts_for(
            index,
            knowledge,
            &[String::from("readme")],
            canonical.as_deref().or(input.location.as_deref()),
            2,
        ),
    });
    if let Some(path) = alternate {
        proof.push(ProofItem {
            expectation: String::from("supporting documentation fallback"),
            kind: ProofKind::Satisfied,
            weight: 2,
            confidence: 0.7,
            detail: format!(
                "Found substantial supporting documentation at {}",
                path.display()
            ),
            context: contexts_for(
                index,
                knowledge,
                &[String::from("context"), String::from("overview")],
                Some(&path),
                2,
            ),
        });
    }

    for (section, aliases) in &knowledge.section_aliases {
        let contexts = contexts_for(index, knowledge, aliases, input.location.as_deref(), 2);
        proof.push(ProofItem {
            expectation: format!("readme section {}", section),
            kind: if contexts.is_empty() {
                ProofKind::Missing
            } else {
                ProofKind::Satisfied
            },
            weight: 2,
            confidence: 0.75,
            detail: if contexts.is_empty() {
                format!("No section matching `{}` was located.", section)
            } else {
                format!("Found README context for `{}`.", section)
            },
            context: contexts,
        });
    }
}

fn infer_workflow(
    input: &RuleInput,
    index: &RepoIndex,
    knowledge: &RuleKnowledge,
    proof: &mut Vec<ProofItem>,
) {
    let workflows = index.workflow_files();
    proof.push(ProofItem {
        expectation: String::from("workflow file exists"),
        kind: if workflows.is_empty() {
            ProofKind::Missing
        } else {
            ProofKind::Satisfied
        },
        weight: 5,
        confidence: 0.95,
        detail: if workflows.is_empty() {
            String::from("No workflow file exists under .github/workflows.")
        } else {
            format!("Detected {} workflow file(s).", workflows.len())
        },
        context: contexts_for(
            index,
            knowledge,
            &[String::from("workflow")],
            input.location.as_deref(),
            3,
        ),
    });

    for trigger in ["pull_request", "push", "workflow_dispatch"] {
        let contexts = contexts_for(
            index,
            knowledge,
            &[trigger.to_owned()],
            input.location.as_deref(),
            2,
        );
        proof.push(ProofItem {
            expectation: format!("workflow trigger {}", trigger),
            kind: if contexts.is_empty() {
                ProofKind::Missing
            } else {
                ProofKind::Satisfied
            },
            weight: if trigger == "workflow_dispatch" { 1 } else { 3 },
            confidence: 0.8,
            detail: if contexts.is_empty() {
                format!("No `{}` trigger was found.", trigger)
            } else {
                format!("A `{}` trigger was found.", trigger)
            },
            context: contexts,
        });
    }

    for command in &knowledge.valid_commands {
        let contexts = contexts_for(
            index,
            knowledge,
            std::slice::from_ref(command),
            input.location.as_deref(),
            1,
        );
        if !contexts.is_empty() {
            proof.push(ProofItem {
                expectation: format!("workflow command {}", command),
                kind: ProofKind::Satisfied,
                weight: 2,
                confidence: 0.75,
                detail: format!("Workflow references `{}`.", command),
                context: contexts,
            });
        }
    }
}

fn infer_tests(
    input: &RuleInput,
    index: &RepoIndex,
    knowledge: &RuleKnowledge,
    proof: &mut Vec<ProofItem>,
) {
    let test_contexts = index
        .chunks()
        .iter()
        .filter(|chunk| chunk.kind == super::ChunkKind::TestPath)
        .map(|chunk| chunk.to_context_ref())
        .take(3)
        .collect::<Vec<_>>();
    proof.push(ProofItem {
        expectation: String::from("test files exist"),
        kind: if test_contexts.is_empty() {
            ProofKind::Missing
        } else {
            ProofKind::Satisfied
        },
        weight: 5,
        confidence: 0.9,
        detail: if test_contexts.is_empty() {
            String::from("No dedicated test paths were indexed.")
        } else {
            format!("Indexed {} test path(s).", test_contexts.len())
        },
        context: test_contexts,
    });

    for command in &knowledge.valid_commands {
        let contexts = contexts_for(
            index,
            knowledge,
            std::slice::from_ref(command),
            input.location.as_deref(),
            1,
        );
        if !contexts.is_empty() {
            proof.push(ProofItem {
                expectation: format!("test command {}", command),
                kind: ProofKind::Satisfied,
                weight: 2,
                confidence: 0.75,
                detail: format!("Found test command `{}`.", command),
                context: contexts,
            });
        }
    }
}

fn infer_lint_and_format(
    input: &RuleInput,
    index: &RepoIndex,
    knowledge: &RuleKnowledge,
    proof: &mut Vec<ProofItem>,
) {
    for command in &knowledge.valid_commands {
        let contexts = contexts_for(
            index,
            knowledge,
            std::slice::from_ref(command),
            input.location.as_deref(),
            1,
        );
        proof.push(ProofItem {
            expectation: format!("maintenance command {}", command),
            kind: if contexts.is_empty() {
                ProofKind::Missing
            } else {
                ProofKind::Satisfied
            },
            weight: 2,
            confidence: 0.7,
            detail: if contexts.is_empty() {
                format!("No sign of `{}` was found.", command)
            } else {
                format!("Found `{}` in the indexed repo surface.", command)
            },
            context: contexts,
        });
    }
}

fn infer_release(
    input: &RuleInput,
    index: &RepoIndex,
    knowledge: &RuleKnowledge,
    proof: &mut Vec<ProofItem>,
) {
    infer_workflow(input, index, knowledge, proof);
    for command in &knowledge.valid_commands {
        let contexts = contexts_for(
            index,
            knowledge,
            std::slice::from_ref(command),
            input.location.as_deref(),
            1,
        );
        if !contexts.is_empty() {
            proof.push(ProofItem {
                expectation: format!("release command {}", command),
                kind: ProofKind::Satisfied,
                weight: 3,
                confidence: 0.8,
                detail: format!("Release path references `{}`.", command),
                context: contexts,
            });
        }
    }
}

fn infer_changelog(
    input: &RuleInput,
    index: &RepoIndex,
    knowledge: &RuleKnowledge,
    proof: &mut Vec<ProofItem>,
) {
    infer_generic(input, index, knowledge, proof);
    let version_contexts = contexts_for(
        index,
        knowledge,
        &[String::from("unreleased"), String::from("## [")],
        input.location.as_deref(),
        2,
    );
    proof.push(ProofItem {
        expectation: String::from("structured changelog sections"),
        kind: if version_contexts.is_empty() {
            ProofKind::Missing
        } else {
            ProofKind::Satisfied
        },
        weight: 3,
        confidence: 0.7,
        detail: if version_contexts.is_empty() {
            String::from("No versioned or unreleased section was found in the changelog.")
        } else {
            String::from("Detected structured changelog headings.")
        },
        context: version_contexts,
    });
}

fn infer_generic(
    input: &RuleInput,
    index: &RepoIndex,
    knowledge: &RuleKnowledge,
    proof: &mut Vec<ProofItem>,
) {
    let contexts = contexts_for(
        index,
        knowledge,
        &knowledge.all_terms(),
        input.location.as_deref(),
        3,
    );
    proof.push(ProofItem {
        expectation: format!("context for {}", input.label),
        kind: if contexts.is_empty() {
            ProofKind::Missing
        } else if input.coverage >= 85 {
            ProofKind::Satisfied
        } else {
            ProofKind::Historical
        },
        weight: 3,
        confidence: 0.65,
        detail: input.message.clone(),
        context: contexts,
    });
}

fn contexts_for(
    index: &RepoIndex,
    knowledge: &RuleKnowledge,
    evidence: &[String],
    location: Option<&Path>,
    limit: usize,
) -> Vec<ContextRef> {
    let mut terms = knowledge.all_terms();
    for value in evidence {
        if !terms.iter().any(|existing| existing == value) {
            terms.push(value.clone());
        }
    }
    if location.is_some() {
        let mut anchored = index.find_nearest_context(location, &terms);
        if !anchored.is_empty() {
            anchored.truncate(limit.max(1));
            return anchored;
        }
    }
    let mut contexts = index.find_contexts(&knowledge.related_paths, &terms, limit);
    if contexts.is_empty() {
        contexts = index.find_nearest_context(location, &terms);
    }
    contexts.truncate(limit.max(1));
    contexts
}

fn severity_weight(severity: FindingSeverity) -> u16 {
    match severity {
        FindingSeverity::Error => 5,
        FindingSeverity::Warning => 4,
        FindingSeverity::Info => 3,
    }
}

fn severity_confidence(severity: FindingSeverity) -> f32 {
    match severity {
        FindingSeverity::Error => 0.95,
        FindingSeverity::Warning => 0.8,
        FindingSeverity::Info => 0.65,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intel::knowledge::KnowledgePack;
    use crate::project::{ProjectKind, RepoProfile};
    use std::fs;

    fn temp_repo(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(name);
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create temp directory");
        path
    }

    #[test]
    fn inference_yields_primary_cause_for_missing_readme() {
        let root = temp_repo("ossify-inference-readme");
        let project = ProjectContext {
            kind: ProjectKind::Rust,
            profile: RepoProfile::Cli,
            name: String::from("demo"),
            manifest_path: None,
            metadata: Default::default(),
        };
        let index = RepoIndex::build(&root, &project).expect("index");
        let intel = infer_rule(
            &RuleInput {
                rule_id: "readme",
                label: "README",
                coverage: 10,
                message: String::from("No README was found."),
                evidence: Vec::new(),
                findings: vec![FindingSignal {
                    id: String::from("readme.missing"),
                    severity: FindingSeverity::Warning,
                    message: String::from("README is missing."),
                    help: String::from("Add a canonical README."),
                    location: None,
                }],
                location: None,
            },
            &project,
            &index,
            &KnowledgePack::load(ProjectKind::Rust).rule("readme"),
        );

        assert!(intel.primary_cause.is_some());
        assert!(!intel.proof.is_empty());

        let _ = fs::remove_dir_all(&root);
    }
}
