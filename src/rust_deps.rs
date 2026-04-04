use std::path::Path;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::doctor::{DoctorEcosystem, DoctorFinding};

const EMBEDDED_RUST_DEPS_PROFILE: &str =
    include_str!("../knowledge/calibration/rust-deps-profile.toml");

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RustAdvisoryClass {
    Critical,
    High,
    Medium,
    Low,
    Unmaintained,
    Yanked,
    Unsound,
    Informational,
    Reported,
}

impl RustAdvisoryClass {
    pub fn code(self) -> &'static str {
        match self {
            Self::Critical => "deps.rust.advisory.critical",
            Self::High => "deps.rust.advisory.high",
            Self::Medium => "deps.rust.advisory.medium",
            Self::Low => "deps.rust.advisory.low",
            Self::Unmaintained => "deps.rust.advisory.unmaintained",
            Self::Yanked => "deps.rust.advisory.yanked",
            Self::Unsound => "deps.rust.advisory.unsound",
            Self::Informational => "deps.rust.advisory.informational",
            Self::Reported => "deps.rust.advisory.reported",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Critical => "critical advisory",
            Self::High => "high-severity advisory",
            Self::Medium => "medium-severity advisory",
            Self::Low => "low-severity advisory",
            Self::Unmaintained => "unmaintained advisory",
            Self::Yanked => "yanked advisory",
            Self::Unsound => "unsound advisory",
            Self::Informational => "informational advisory",
            Self::Reported => "reported advisory",
        }
    }

    fn cap_rank(self) -> u8 {
        match self {
            Self::Critical => 0,
            Self::High => 1,
            Self::Unsound => 2,
            Self::Medium => 3,
            Self::Reported => 4,
            Self::Unmaintained => 5,
            Self::Yanked => 6,
            Self::Low => 7,
            Self::Informational => 8,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RustDepsScoringProfile {
    pub version: u8,
    pub objective: String,
    pub weights: RustDepsWeights,
    pub caps: RustDepsCaps,
    pub combination: RustDepsCombination,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RustDepsWeights {
    pub advisory_critical: u16,
    pub advisory_high: u16,
    pub advisory_medium: u16,
    pub advisory_low: u16,
    pub advisory_unmaintained: u16,
    pub advisory_yanked: u16,
    pub advisory_unsound: u16,
    pub advisory_informational: u16,
    pub advisory_reported: u16,
    pub lockfile_missing: u16,
    pub policy_missing: u16,
    pub license_missing: u16,
    pub version_wildcard: u16,
    pub source_direct: u16,
    pub source_path: u16,
    pub source_registry: u16,
    pub license_reported: u16,
    pub source_reported: u16,
    pub bans_reported: u16,
    pub engine_signal: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RustDepsCaps {
    pub advisory_critical: u8,
    pub advisory_high: u8,
    pub advisory_medium: u8,
    pub advisory_low: u8,
    pub advisory_unmaintained: u8,
    pub advisory_yanked: u8,
    pub advisory_unsound: u8,
    pub advisory_informational: u8,
    pub advisory_reported: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RustDepsCombination {
    pub repeated_advisory_penalty: u16,
    pub low_signal_stack_penalty: u16,
    pub unmaintained_cap_step: u8,
    pub yanked_cap_step: u8,
    pub policy_only_floor: u8,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RustDepsFeatureVector {
    pub advisory_critical: u16,
    pub advisory_high: u16,
    pub advisory_medium: u16,
    pub advisory_low: u16,
    pub advisory_unmaintained: u16,
    pub advisory_yanked: u16,
    pub advisory_unsound: u16,
    pub advisory_informational: u16,
    pub advisory_reported: u16,
    pub lockfile_missing: u16,
    pub policy_missing: u16,
    pub license_missing: u16,
    pub version_wildcard: u16,
    pub source_direct: u16,
    pub source_path: u16,
    pub source_registry: u16,
    pub license_reported: u16,
    pub source_reported: u16,
    pub bans_reported: u16,
    pub engine_signal: u16,
}

impl RustDepsFeatureVector {
    pub fn from_findings(findings: &[DoctorFinding]) -> Self {
        let mut vector = Self::default();
        for finding in findings {
            if finding.ecosystem != Some(DoctorEcosystem::Rust) {
                continue;
            }
            match finding.code.as_str() {
                "deps.rust.advisory.critical" => vector.advisory_critical += 1,
                "deps.rust.advisory.high" => vector.advisory_high += 1,
                "deps.rust.advisory.medium" => vector.advisory_medium += 1,
                "deps.rust.advisory.low" => vector.advisory_low += 1,
                "deps.rust.advisory.unmaintained" => vector.advisory_unmaintained += 1,
                "deps.rust.advisory.yanked" => vector.advisory_yanked += 1,
                "deps.rust.advisory.unsound" => vector.advisory_unsound += 1,
                "deps.rust.advisory.informational" => vector.advisory_informational += 1,
                "deps.rust.advisory.reported" => vector.advisory_reported += 1,
                "deps.rust.lockfile.missing" => vector.lockfile_missing += 1,
                "deps.rust.policy-missing" => vector.policy_missing += 1,
                "deps.rust.license-missing" => vector.license_missing += 1,
                "deps.rust.version.wildcard" => vector.version_wildcard += 1,
                "deps.rust.source.direct-source" => vector.source_direct += 1,
                "deps.rust.source.path" => vector.source_path += 1,
                "deps.rust.source.registry" => vector.source_registry += 1,
                "deps.rust.license.reported" => vector.license_reported += 1,
                "deps.rust.source.reported" => vector.source_reported += 1,
                "deps.rust.bans.reported" => vector.bans_reported += 1,
                "deps.rust.engine.signal" => vector.engine_signal += 1,
                _ => {}
            }
        }
        vector
    }

    pub fn total_advisories(&self) -> u16 {
        self.advisory_critical
            + self.advisory_high
            + self.advisory_medium
            + self.advisory_low
            + self.advisory_unmaintained
            + self.advisory_yanked
            + self.advisory_unsound
            + self.advisory_informational
            + self.advisory_reported
    }

    pub fn policy_only(&self) -> bool {
        self.total_advisories() == 0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RustDepsScoringOutcome {
    pub score: u8,
    pub cap: Option<u8>,
    pub cap_reason: Option<String>,
    pub cap_code: Option<String>,
    pub features: RustDepsFeatureVector,
}

pub fn active_rust_deps_profile() -> &'static RustDepsScoringProfile {
    static PROFILE: OnceLock<RustDepsScoringProfile> = OnceLock::new();
    PROFILE.get_or_init(|| {
        toml::from_str(EMBEDDED_RUST_DEPS_PROFILE)
            .expect("embedded Rust deps scoring profile should parse")
    })
}

pub fn score_rust_deps_findings(findings: &[DoctorFinding]) -> RustDepsScoringOutcome {
    let features = RustDepsFeatureVector::from_findings(findings);
    score_rust_deps_features(&features, findings, active_rust_deps_profile())
}

pub fn score_rust_deps_features(
    features: &RustDepsFeatureVector,
    findings: &[DoctorFinding],
    profile: &RustDepsScoringProfile,
) -> RustDepsScoringOutcome {
    let penalty = rust_deps_penalty(features, profile);
    let mut score = 100u16.saturating_sub(penalty).min(100) as u8;
    if features.policy_only() {
        score = score.max(profile.combination.policy_only_floor);
    }

    let (cap, cap_class) = rust_deps_cap(features, profile);
    if let Some(cap) = cap {
        score = score.min(cap);
    }

    let cap_code = cap_class.map(|class| class.code().to_owned());
    let cap_reason = cap
        .zip(cap_class)
        .map(|(cap, class)| describe_cap_reason(class, cap, findings));

    RustDepsScoringOutcome {
        score,
        cap,
        cap_reason,
        cap_code,
        features: features.clone(),
    }
}

pub fn load_rust_deps_profile(path: &Path) -> std::io::Result<RustDepsScoringProfile> {
    let profile_text = std::fs::read_to_string(path)?;
    toml::from_str(&profile_text).map_err(|error| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("invalid Rust deps profile: {error}"),
        )
    })
}

pub fn render_rust_deps_profile_toml(profile: &RustDepsScoringProfile) -> String {
    toml::to_string_pretty(profile).expect("Rust deps profile should serialize")
}

fn rust_deps_penalty(features: &RustDepsFeatureVector, profile: &RustDepsScoringProfile) -> u16 {
    let weights = &profile.weights;
    let mut penalty = 0u16;
    penalty += features.advisory_critical * weights.advisory_critical;
    penalty += features.advisory_high * weights.advisory_high;
    penalty += features.advisory_medium * weights.advisory_medium;
    penalty += features.advisory_low * weights.advisory_low;
    penalty += features.advisory_unmaintained * weights.advisory_unmaintained;
    penalty += features.advisory_yanked * weights.advisory_yanked;
    penalty += features.advisory_unsound * weights.advisory_unsound;
    penalty += features.advisory_informational * weights.advisory_informational;
    penalty += features.advisory_reported * weights.advisory_reported;
    penalty += features.lockfile_missing * weights.lockfile_missing;
    penalty += features.policy_missing * weights.policy_missing;
    penalty += features.license_missing * weights.license_missing;
    penalty += features.version_wildcard * weights.version_wildcard;
    penalty += features.source_direct * weights.source_direct;
    penalty += features.source_path * weights.source_path;
    penalty += features.source_registry * weights.source_registry;
    penalty += features.license_reported * weights.license_reported;
    penalty += features.source_reported * weights.source_reported;
    penalty += features.bans_reported * weights.bans_reported;
    penalty += features.engine_signal * weights.engine_signal;

    let advisory_total = features.total_advisories();
    if advisory_total > 1 {
        penalty += (advisory_total - 1) * profile.combination.repeated_advisory_penalty;
    }

    let low_signal_stack = features.advisory_low + features.advisory_informational;
    if low_signal_stack > 1 {
        penalty += (low_signal_stack - 1) * profile.combination.low_signal_stack_penalty;
    }

    penalty
}

fn rust_deps_cap(
    features: &RustDepsFeatureVector,
    profile: &RustDepsScoringProfile,
) -> (Option<u8>, Option<RustAdvisoryClass>) {
    let mut candidates = Vec::new();
    if features.advisory_critical > 0 {
        candidates.push((profile.caps.advisory_critical, RustAdvisoryClass::Critical));
    }
    if features.advisory_high > 0 {
        candidates.push((profile.caps.advisory_high, RustAdvisoryClass::High));
    }
    if features.advisory_unsound > 0 {
        candidates.push((profile.caps.advisory_unsound, RustAdvisoryClass::Unsound));
    }
    if features.advisory_medium > 0 {
        candidates.push((profile.caps.advisory_medium, RustAdvisoryClass::Medium));
    }
    if features.advisory_reported > 0 {
        candidates.push((profile.caps.advisory_reported, RustAdvisoryClass::Reported));
    }
    if features.advisory_unmaintained > 0 {
        let reduction = u8::try_from(
            features
                .advisory_unmaintained
                .saturating_sub(1)
                .saturating_mul(u16::from(profile.combination.unmaintained_cap_step)),
        )
        .unwrap_or(u8::MAX);
        candidates.push((
            profile.caps.advisory_unmaintained.saturating_sub(reduction),
            RustAdvisoryClass::Unmaintained,
        ));
    }
    if features.advisory_yanked > 0 {
        let reduction = u8::try_from(
            features
                .advisory_yanked
                .saturating_sub(1)
                .saturating_mul(u16::from(profile.combination.yanked_cap_step)),
        )
        .unwrap_or(u8::MAX);
        candidates.push((
            profile.caps.advisory_yanked.saturating_sub(reduction),
            RustAdvisoryClass::Yanked,
        ));
    }
    if features.advisory_low > 0 {
        candidates.push((profile.caps.advisory_low, RustAdvisoryClass::Low));
    }
    if features.advisory_informational > 0 {
        candidates.push((
            profile.caps.advisory_informational,
            RustAdvisoryClass::Informational,
        ));
    }

    candidates
        .into_iter()
        .min_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then_with(|| left.1.cap_rank().cmp(&right.1.cap_rank()))
        })
        .map_or((None, None), |(cap, class)| (Some(cap), Some(class)))
}

fn describe_cap_reason(class: RustAdvisoryClass, cap: u8, findings: &[DoctorFinding]) -> String {
    let prefix = finding_for_class(findings, class)
        .map(|finding| {
            let advisory_id = find_evidence_value(finding, "advisory.id");
            let package = find_evidence_value(finding, "crate")
                .or_else(|| find_evidence_value(finding, "package"));
            let version = find_evidence_value(finding, "version");

            match (advisory_id, package, version) {
                (Some(id), Some(package), Some(version)) => {
                    format!("{} {} for {} {}", class.label(), id, package, version)
                }
                (Some(id), Some(package), None) => {
                    format!("{} {} for {}", class.label(), id, package)
                }
                (Some(id), None, None) => format!("{} {}", class.label(), id),
                _ => finding.message.clone(),
            }
        })
        .unwrap_or_else(|| class.label().to_owned());

    format!("{prefix} capped Rust dependency score at {cap}/100")
}

fn finding_for_class(
    findings: &[DoctorFinding],
    class: RustAdvisoryClass,
) -> Option<&DoctorFinding> {
    findings
        .iter()
        .find(|finding| finding.code == class.code())
        .or_else(|| {
            findings
                .iter()
                .find(|finding| finding.code.starts_with(class.code()))
        })
}

fn find_evidence_value<'a>(finding: &'a DoctorFinding, key: &str) -> Option<&'a str> {
    let prefix = format!("{key}=");
    finding
        .evidence
        .iter()
        .find_map(|entry| entry.strip_prefix(&prefix))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctor::{DoctorDomain, DoctorSeverity};

    fn finding(code: &str) -> DoctorFinding {
        DoctorFinding {
            domain: DoctorDomain::Deps,
            ecosystem: Some(DoctorEcosystem::Rust),
            severity: DoctorSeverity::Error,
            code: String::from(code),
            message: String::from(code),
            file: None,
            help: None,
            evidence: Vec::new(),
            fix_hint: None,
            engine: None,
        }
    }

    #[test]
    fn policy_only_score_stays_above_floor() {
        let findings = vec![finding("deps.rust.policy-missing")];
        let outcome = score_rust_deps_findings(&findings);
        assert!(outcome.score >= active_rust_deps_profile().combination.policy_only_floor);
    }

    #[test]
    fn advisory_caps_remain_ordered() {
        let critical = score_rust_deps_findings(&[finding("deps.rust.advisory.critical")]).score;
        let high = score_rust_deps_findings(&[finding("deps.rust.advisory.high")]).score;
        let medium = score_rust_deps_findings(&[finding("deps.rust.advisory.medium")]).score;
        let unmaintained =
            score_rust_deps_findings(&[finding("deps.rust.advisory.unmaintained")]).score;
        let policy_only = score_rust_deps_findings(&[finding("deps.rust.policy-missing")]).score;

        assert!(critical < high);
        assert!(high < medium);
        assert!(medium < unmaintained);
        assert!(unmaintained < policy_only);
    }
}
