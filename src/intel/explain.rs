use std::collections::BTreeSet;

use super::{
    CacheState, ConfidenceBreakdown, ContextRef, HistoryRef, ProofItem, ProofKind, RetrievalScope,
    RootCause, RuleIntelligence,
};

pub fn finalize_rule_intelligence(
    proof: Vec<ProofItem>,
    history_refs: Vec<HistoryRef>,
    cache_state: CacheState,
) -> RuleIntelligence {
    let causes = rank_causes(&proof, &history_refs);
    let primary_cause = causes.first().cloned();
    let secondary_causes = causes.iter().skip(1).cloned().collect::<Vec<_>>();
    let context_refs = collect_context_refs(&proof);

    RuleIntelligence {
        confidence_breakdown: confidence_breakdown(&proof),
        retrieval_scope: retrieval_scope(&context_refs, &proof, cache_state),
        primary_cause,
        secondary_causes,
        causes,
        proof,
        context_refs,
        history_refs,
    }
}

fn rank_causes(proof: &[ProofItem], history_refs: &[HistoryRef]) -> Vec<RootCause> {
    let mut causes = proof
        .iter()
        .filter(|item| matches!(item.kind, ProofKind::Missing | ProofKind::Contradiction))
        .map(|item| RootCause {
            id: slugify(&item.expectation),
            title: item.expectation.clone(),
            detail: item.detail.clone(),
            impact: item.weight as f32 * item.confidence * blast_radius(item),
            context: item.context.clone(),
            history: if matches!(item.kind, ProofKind::Contradiction) {
                history_refs.iter().take(2).cloned().collect()
            } else {
                Vec::new()
            },
        })
        .collect::<Vec<_>>();

    causes.sort_by(|left, right| right.impact.total_cmp(&left.impact));
    causes
}

fn blast_radius(item: &ProofItem) -> f32 {
    let base = if item.context.is_empty() { 1.2 } else { 1.0 };
    base + (item.context.len().min(3) as f32 * 0.1)
}

fn collect_context_refs(proof: &[ProofItem]) -> Vec<ContextRef> {
    let mut seen = BTreeSet::new();
    let mut refs = Vec::new();

    for item in proof {
        for context in &item.context {
            let key = format!(
                "{}:{}:{}",
                context.path.display(),
                context.line_start.unwrap_or_default(),
                context.line_end.unwrap_or_default()
            );
            if seen.insert(key) {
                refs.push(context.clone());
            }
        }
    }

    refs
}

fn retrieval_scope(
    context_refs: &[ContextRef],
    proof: &[ProofItem],
    cache_state: CacheState,
) -> RetrievalScope {
    let consulted_paths = context_refs
        .iter()
        .map(|context| context.path.display().to_string())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let chunk_kinds = context_refs
        .iter()
        .map(|context| context.chunk_kind.as_str().to_owned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    RetrievalScope {
        consulted_paths,
        chunk_kinds,
        used_history: proof
            .iter()
            .any(|item| matches!(item.kind, ProofKind::Historical)),
        cache_state,
    }
}

fn confidence_breakdown(proof: &[ProofItem]) -> ConfidenceBreakdown {
    let support_score = proof
        .iter()
        .filter(|item| matches!(item.kind, ProofKind::Satisfied | ProofKind::Historical))
        .map(|item| item.weight as f32 * item.confidence)
        .sum::<f32>();
    let penalty_score = proof
        .iter()
        .filter(|item| matches!(item.kind, ProofKind::Missing | ProofKind::Contradiction))
        .map(|item| item.weight as f32 * item.confidence)
        .sum::<f32>();
    let total_required_weight = proof
        .iter()
        .map(|item| item.weight as f32)
        .sum::<f32>()
        .max(1.0);
    let derived = ((support_score - penalty_score).max(0.0) / total_required_weight * 100.0)
        .round()
        .clamp(0.0, 100.0) as u8;

    ConfidenceBreakdown {
        support_score,
        penalty_score,
        total_required_weight,
        derived_coverage: derived,
    }
}

fn slugify(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_owned()
}
