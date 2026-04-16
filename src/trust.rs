use crate::doctor::{DoctorDomainScore, EngineSource};

#[derive(Debug, Clone, Copy)]
pub struct TrustKernelConfig {
    pub base_weight: f32,
}

impl Default for TrustKernelConfig {
    fn default() -> Self {
        Self { base_weight: 0.60 }
    }
}

pub fn aggregate_trust_score(
    base_score: u8,
    domain_scores: &[DoctorDomainScore],
    config: TrustKernelConfig,
) -> u8 {
    let scored = domain_scores
        .iter()
        .filter_map(|entry| {
            entry
                .score
                .map(|score| (score as f32, reliability(entry.engine_source)))
        })
        .collect::<Vec<_>>();

    let mut blended = if scored.is_empty() {
        base_score
    } else {
        let weighted_sum = scored
            .iter()
            .map(|(score, weight)| score * weight)
            .sum::<f32>();
        let weight_total = scored
            .iter()
            .map(|(_, weight)| weight)
            .sum::<f32>()
            .max(1e-6);
        let domain_center = weighted_sum / weight_total;
        let base = f32::from(base_score);
        let trust = (config.base_weight * base) + ((1.0 - config.base_weight) * domain_center);
        trust.round().clamp(0.0, 100.0) as u8
    };

    for cap in domain_scores.iter().filter_map(|entry| entry.cap) {
        blended = blended.min(cap);
    }

    blended
}

fn reliability(source: EngineSource) -> f32 {
    match source {
        EngineSource::ManagedTool => 1.0,
        EngineSource::OssifyNative => 0.9,
        EngineSource::AbsorbedPolicy => 0.6,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctor::{DoctorDomain, DoctorEcosystem};

    fn score(domain: DoctorDomain, score: u8, source: EngineSource) -> DoctorDomainScore {
        DoctorDomainScore {
            domain,
            score: Some(score),
            cap: None,
            cap_reason: None,
            cap_code: None,
            engine: String::from("test"),
            engine_source: source,
            ecosystems: vec![DoctorEcosystem::Auto],
            error_count: 0,
            warning_count: 0,
            info_count: 0,
            summary: String::from("ok"),
        }
    }

    #[test]
    fn aggregate_respects_domain_reliability() {
        let base = 90;
        let domains = vec![
            score(DoctorDomain::Docs, 50, EngineSource::AbsorbedPolicy),
            score(DoctorDomain::Deps, 50, EngineSource::ManagedTool),
        ];

        let blended = aggregate_trust_score(base, &domains, TrustKernelConfig::default());
        assert!(blended > 70);
        assert!(blended < 90);
    }

    #[test]
    fn aggregate_applies_hard_caps() {
        let mut docs = score(DoctorDomain::Docs, 100, EngineSource::OssifyNative);
        docs.cap = Some(49);
        let blended = aggregate_trust_score(95, &[docs], TrustKernelConfig::default());
        assert_eq!(blended, 49);
    }
}
