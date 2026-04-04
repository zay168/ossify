use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use clap::Parser;
use ignore::WalkBuilder;
use ossify::doctor::{doctor_deps, DoctorEcosystem};
use ossify::engines::ManagedEngineStatus;
use ossify::rust_deps::{
    active_rust_deps_profile, render_rust_deps_profile_toml, score_rust_deps_features,
    RustDepsFeatureVector, RustDepsScoringProfile,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Parser)]
#[command(
    name = "ossify-calibrate",
    about = "Calibrate Rust dependency scoring profiles deterministically."
)]
struct Args {
    #[arg(long = "root")]
    roots: Vec<PathBuf>,
    #[arg(long, default_value = "knowledge/calibration/rust-deps-fixtures")]
    fixtures_dir: PathBuf,
    #[arg(long, default_value = "target/calibration/rust-deps/cache.json")]
    cache: PathBuf,
    #[arg(long, default_value = "target/calibration/rust-deps/report.md")]
    report: PathBuf,
    #[arg(
        long,
        default_value = "target/calibration/rust-deps/tuned-profile.toml"
    )]
    output_profile: PathBuf,
    #[arg(long)]
    write_profile: Option<PathBuf>,
    #[arg(long, default_value_t = 24)]
    max_repos: usize,
}

#[derive(Debug, Clone, Deserialize)]
struct FixtureDefinition {
    name: String,
    description: String,
    score_min: u8,
    score_max: u8,
    cap: Option<u8>,
    dominant_cap_code: Option<String>,
    #[serde(default)]
    features: RustDepsFeatureVector,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct CalibrationCache {
    repos: BTreeMap<String, CachedRepo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedRepo {
    fingerprint: String,
    features: RustDepsFeatureVector,
    engine_status: String,
    cap_code: Option<String>,
    cap_reason: Option<String>,
}

#[derive(Debug, Clone)]
struct LocalRepoSample {
    path: PathBuf,
    features: RustDepsFeatureVector,
    engine_status: ManagedEngineStatus,
}

#[derive(Debug, Clone)]
struct FixtureResult {
    name: String,
    description: String,
    score: u8,
    cap: Option<u8>,
    cap_code: Option<String>,
}

#[derive(Debug, Clone)]
struct TuningOutcome {
    profile: RustDepsScoringProfile,
    loss: i64,
    fixture_results: Vec<FixtureResult>,
}

#[derive(Debug, Clone, Copy)]
enum TunableField {
    WeightCritical,
    WeightHigh,
    WeightMedium,
    WeightLow,
    WeightUnmaintained,
    WeightYanked,
    WeightUnsound,
    WeightInformational,
    WeightReported,
    CapCritical,
    CapHigh,
    CapMedium,
    CapLow,
    CapUnmaintained,
    CapYanked,
    CapUnsound,
    CapInformational,
    CapReported,
    RepeatAdvisoryPenalty,
    LowSignalStackPenalty,
    UnmaintainedCapStep,
    YankedCapStep,
    PolicyOnlyFloor,
}

impl TunableField {
    const ALL: [Self; 23] = [
        Self::WeightCritical,
        Self::WeightHigh,
        Self::WeightMedium,
        Self::WeightLow,
        Self::WeightUnmaintained,
        Self::WeightYanked,
        Self::WeightUnsound,
        Self::WeightInformational,
        Self::WeightReported,
        Self::CapCritical,
        Self::CapHigh,
        Self::CapMedium,
        Self::CapLow,
        Self::CapUnmaintained,
        Self::CapYanked,
        Self::CapUnsound,
        Self::CapInformational,
        Self::CapReported,
        Self::RepeatAdvisoryPenalty,
        Self::LowSignalStackPenalty,
        Self::UnmaintainedCapStep,
        Self::YankedCapStep,
        Self::PolicyOnlyFloor,
    ];

    fn get(self, profile: &RustDepsScoringProfile) -> i32 {
        match self {
            Self::WeightCritical => profile.weights.advisory_critical.into(),
            Self::WeightHigh => profile.weights.advisory_high.into(),
            Self::WeightMedium => profile.weights.advisory_medium.into(),
            Self::WeightLow => profile.weights.advisory_low.into(),
            Self::WeightUnmaintained => profile.weights.advisory_unmaintained.into(),
            Self::WeightYanked => profile.weights.advisory_yanked.into(),
            Self::WeightUnsound => profile.weights.advisory_unsound.into(),
            Self::WeightInformational => profile.weights.advisory_informational.into(),
            Self::WeightReported => profile.weights.advisory_reported.into(),
            Self::CapCritical => profile.caps.advisory_critical.into(),
            Self::CapHigh => profile.caps.advisory_high.into(),
            Self::CapMedium => profile.caps.advisory_medium.into(),
            Self::CapLow => profile.caps.advisory_low.into(),
            Self::CapUnmaintained => profile.caps.advisory_unmaintained.into(),
            Self::CapYanked => profile.caps.advisory_yanked.into(),
            Self::CapUnsound => profile.caps.advisory_unsound.into(),
            Self::CapInformational => profile.caps.advisory_informational.into(),
            Self::CapReported => profile.caps.advisory_reported.into(),
            Self::RepeatAdvisoryPenalty => profile.combination.repeated_advisory_penalty.into(),
            Self::LowSignalStackPenalty => profile.combination.low_signal_stack_penalty.into(),
            Self::UnmaintainedCapStep => profile.combination.unmaintained_cap_step.into(),
            Self::YankedCapStep => profile.combination.yanked_cap_step.into(),
            Self::PolicyOnlyFloor => profile.combination.policy_only_floor.into(),
        }
    }

    fn set(self, profile: &mut RustDepsScoringProfile, value: i32) {
        match self {
            Self::WeightCritical => profile.weights.advisory_critical = value as u16,
            Self::WeightHigh => profile.weights.advisory_high = value as u16,
            Self::WeightMedium => profile.weights.advisory_medium = value as u16,
            Self::WeightLow => profile.weights.advisory_low = value as u16,
            Self::WeightUnmaintained => profile.weights.advisory_unmaintained = value as u16,
            Self::WeightYanked => profile.weights.advisory_yanked = value as u16,
            Self::WeightUnsound => profile.weights.advisory_unsound = value as u16,
            Self::WeightInformational => profile.weights.advisory_informational = value as u16,
            Self::WeightReported => profile.weights.advisory_reported = value as u16,
            Self::CapCritical => profile.caps.advisory_critical = value as u8,
            Self::CapHigh => profile.caps.advisory_high = value as u8,
            Self::CapMedium => profile.caps.advisory_medium = value as u8,
            Self::CapLow => profile.caps.advisory_low = value as u8,
            Self::CapUnmaintained => profile.caps.advisory_unmaintained = value as u8,
            Self::CapYanked => profile.caps.advisory_yanked = value as u8,
            Self::CapUnsound => profile.caps.advisory_unsound = value as u8,
            Self::CapInformational => profile.caps.advisory_informational = value as u8,
            Self::CapReported => profile.caps.advisory_reported = value as u8,
            Self::RepeatAdvisoryPenalty => {
                profile.combination.repeated_advisory_penalty = value as u16
            }
            Self::LowSignalStackPenalty => {
                profile.combination.low_signal_stack_penalty = value as u16
            }
            Self::UnmaintainedCapStep => profile.combination.unmaintained_cap_step = value as u8,
            Self::YankedCapStep => profile.combination.yanked_cap_step = value as u8,
            Self::PolicyOnlyFloor => profile.combination.policy_only_floor = value as u8,
        }
    }

    fn bounds(self) -> (i32, i32) {
        match self {
            Self::WeightCritical => (32, 60),
            Self::WeightHigh => (22, 40),
            Self::WeightMedium => (14, 28),
            Self::WeightLow => (4, 12),
            Self::WeightUnmaintained => (10, 24),
            Self::WeightYanked => (8, 20),
            Self::WeightUnsound => (18, 34),
            Self::WeightInformational => (2, 10),
            Self::WeightReported => (18, 36),
            Self::CapCritical => (18, 28),
            Self::CapHigh => (32, 45),
            Self::CapMedium => (48, 60),
            Self::CapLow => (82, 92),
            Self::CapUnmaintained => (58, 75),
            Self::CapYanked => (65, 78),
            Self::CapUnsound => (38, 50),
            Self::CapInformational => (88, 96),
            Self::CapReported => (45, 60),
            Self::RepeatAdvisoryPenalty => (1, 8),
            Self::LowSignalStackPenalty => (0, 4),
            Self::UnmaintainedCapStep => (1, 6),
            Self::YankedCapStep => (1, 5),
            Self::PolicyOnlyFloor => (82, 92),
        }
    }
}

fn main() -> io::Result<()> {
    let args = Args::parse();
    let roots = calibration_roots(&args.roots);
    let fixtures = load_fixtures(&args.fixtures_dir)?;
    let mut cache = load_cache(&args.cache);
    let local_repos = collect_local_rust_samples(&roots, args.max_repos, &mut cache)?;
    write_cache(&args.cache, &cache)?;

    let baseline = active_rust_deps_profile().clone();
    let tuned = tune_profile(&baseline, &fixtures, &local_repos);
    write_string(
        &args.output_profile,
        &render_rust_deps_profile_toml(&tuned.profile),
    )?;
    if let Some(path) = &args.write_profile {
        write_string(path, &render_rust_deps_profile_toml(&tuned.profile))?;
    }
    write_string(
        &args.report,
        &render_report(&baseline, &tuned, &fixtures, &local_repos),
    )?;

    println!("Rust deps calibration complete.");
    println!("Fixtures: {}", fixtures.len());
    println!("Local Rust repos: {}", local_repos.len());
    println!(
        "Baseline loss: {}",
        evaluate_loss(&baseline, &fixtures, &local_repos)
    );
    println!("Tuned loss: {}", tuned.loss);
    println!("Tuned profile: {}", args.output_profile.display());
    println!("Report: {}", args.report.display());
    if let Some(path) = args.write_profile {
        println!("Wrote active profile to {}", path.display());
    }

    Ok(())
}

fn calibration_roots(explicit: &[PathBuf]) -> Vec<PathBuf> {
    if !explicit.is_empty() {
        return explicit.to_vec();
    }

    let mut roots = Vec::new();
    if let Ok(current) = std::env::current_dir() {
        roots.push(current);
    }
    if cfg!(windows) {
        let github = PathBuf::from(r"C:\GitHub");
        if github.is_dir() {
            roots.push(github);
        }
    }
    if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
        let downloads = PathBuf::from(home).join("Downloads");
        if downloads.is_dir() {
            roots.push(downloads);
        }
    }

    let mut unique = Vec::new();
    let mut seen = BTreeSet::new();
    for root in roots {
        let key = root.to_string_lossy().to_string();
        if seen.insert(key) {
            unique.push(root);
        }
    }
    unique
}

fn load_fixtures(dir: &Path) -> io::Result<Vec<FixtureDefinition>> {
    let mut fixtures = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_file()
            && entry
                .path()
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value.eq_ignore_ascii_case("toml"))
        {
            let text = fs::read_to_string(entry.path())?;
            let fixture: FixtureDefinition = toml::from_str(&text).map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("invalid fixture {}: {error}", entry.path().display()),
                )
            })?;
            fixtures.push(fixture);
        }
    }
    fixtures.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(fixtures)
}

fn load_cache(path: &Path) -> CalibrationCache {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn write_cache(path: &Path, cache: &CalibrationCache) -> io::Result<()> {
    write_string(
        path,
        &serde_json::to_string_pretty(cache)
            .map_err(|error| io::Error::other(error.to_string()))?,
    )
}

fn collect_local_rust_samples(
    roots: &[PathBuf],
    max_repos: usize,
    cache: &mut CalibrationCache,
) -> io::Result<Vec<LocalRepoSample>> {
    let mut repos = discover_rust_roots(roots);
    repos.truncate(max_repos);

    let mut samples = Vec::new();
    for repo in repos {
        let fingerprint = repo_fingerprint(&repo)?;
        let key = repo.to_string_lossy().to_string();
        if let Some(cached) = cache.repos.get(&key) {
            if cached.fingerprint == fingerprint {
                let status = deserialize_engine_status(&cached.engine_status);
                samples.push(LocalRepoSample {
                    path: repo.clone(),
                    features: cached.features.clone(),
                    engine_status: status,
                });
                continue;
            }
        }

        let report = doctor_deps(&repo, DoctorEcosystem::Rust)?;
        let rust_entry = report
            .ecosystems
            .iter()
            .find(|entry| entry.ecosystem == DoctorEcosystem::Rust)
            .ok_or_else(|| {
                io::Error::other("Rust deps doctor did not return a Rust ecosystem row")
            })?;
        let features = RustDepsFeatureVector::from_findings(&report.findings);
        cache.repos.insert(
            key,
            CachedRepo {
                fingerprint,
                features: features.clone(),
                engine_status: rust_entry.engine_status.label().to_owned(),
                cap_code: rust_entry.cap_code.clone(),
                cap_reason: rust_entry.cap_reason.clone(),
            },
        );
        samples.push(LocalRepoSample {
            path: repo,
            features,
            engine_status: rust_entry.engine_status,
        });
    }

    Ok(samples)
}

fn discover_rust_roots(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut discovered = Vec::new();
    let mut seen = BTreeSet::new();

    for root in roots {
        let walker = WalkBuilder::new(root)
            .hidden(false)
            .ignore(false)
            .git_ignore(true)
            .git_exclude(true)
            .parents(true)
            .max_depth(Some(5))
            .build();
        for entry in walker.filter_map(Result::ok) {
            let path = entry.path();
            if !entry.file_type().is_some_and(|kind| kind.is_file()) {
                continue;
            }
            if path.file_name().and_then(|value| value.to_str()) != Some("Cargo.toml") {
                continue;
            }
            if path.components().any(|component| {
                matches!(
                    component.as_os_str().to_str(),
                    Some("target" | "node_modules" | ".git" | "vendor")
                )
            }) {
                continue;
            }

            let repo = path.parent().unwrap_or(path).to_path_buf();
            let canonical = fs::canonicalize(&repo).unwrap_or(repo);
            let key = canonical.to_string_lossy().to_string();
            if seen.insert(key) {
                discovered.push(canonical);
            }
        }
    }

    discovered.sort();
    discovered
}

fn repo_fingerprint(root: &Path) -> io::Result<String> {
    let mut hasher = blake3::Hasher::new();
    for path in [
        root.join("Cargo.toml"),
        root.join("Cargo.lock"),
        root.join("deny.toml"),
    ] {
        hasher.update(path.to_string_lossy().as_bytes());
        if let Ok(metadata) = fs::metadata(&path) {
            hasher.update(&metadata.len().to_le_bytes());
            if let Ok(modified) = metadata.modified() {
                if let Ok(duration) = modified.duration_since(UNIX_EPOCH) {
                    hasher.update(&duration.as_secs().to_le_bytes());
                    hasher.update(&duration.subsec_nanos().to_le_bytes());
                }
            }
        }
    }
    Ok(hasher.finalize().to_hex().to_string())
}

fn deserialize_engine_status(value: &str) -> ManagedEngineStatus {
    match value {
        "managed" => ManagedEngineStatus::Managed,
        "heuristic-fallback" => ManagedEngineStatus::HeuristicFallback,
        "bootstrap-failed" => ManagedEngineStatus::BootstrapFailed,
        "runtime-missing" => ManagedEngineStatus::RuntimeMissing,
        "execution-failed" => ManagedEngineStatus::ExecutionFailed,
        "parse-failed" => ManagedEngineStatus::ParseFailed,
        _ => ManagedEngineStatus::HeuristicFallback,
    }
}

fn tune_profile(
    baseline: &RustDepsScoringProfile,
    fixtures: &[FixtureDefinition],
    local_repos: &[LocalRepoSample],
) -> TuningOutcome {
    let mut best = baseline.clone();
    let mut best_loss = evaluate_loss(&best, fixtures, local_repos);

    for step in [4, 2, 1] {
        let mut improved = true;
        while improved {
            improved = false;
            for field in TunableField::ALL {
                let base_value = field.get(&best);
                for candidate in [base_value - step, base_value + step] {
                    let (min, max) = field.bounds();
                    if !(min..=max).contains(&candidate) {
                        continue;
                    }
                    let mut trial = best.clone();
                    field.set(&mut trial, candidate);
                    let loss = evaluate_loss(&trial, fixtures, local_repos);
                    if loss < best_loss {
                        best = trial;
                        best_loss = loss;
                        improved = true;
                    }
                }
            }
        }
    }

    TuningOutcome {
        fixture_results: evaluate_fixtures(&best, fixtures),
        profile: best,
        loss: best_loss,
    }
}

fn evaluate_loss(
    profile: &RustDepsScoringProfile,
    fixtures: &[FixtureDefinition],
    local_repos: &[LocalRepoSample],
) -> i64 {
    let mut loss = 0i64;

    for fixture in fixtures {
        let outcome = score_rust_deps_features(&fixture.features, &[], profile);
        if outcome.score < fixture.score_min {
            let delta = i64::from(fixture.score_min - outcome.score);
            loss += delta * delta * 12;
        }
        if outcome.score > fixture.score_max {
            let delta = i64::from(outcome.score - fixture.score_max);
            loss += delta * delta * 12;
        }
        if fixture.cap != outcome.cap {
            loss += 120;
            if let (Some(expected), Some(actual)) = (fixture.cap, outcome.cap) {
                loss += i64::from(expected.abs_diff(actual)) * 6;
            }
        }
        if fixture.dominant_cap_code != outcome.cap_code {
            loss += 150;
        }
    }

    let fixture_scores = evaluate_fixtures(profile, fixtures);
    let score_by_name = fixture_scores
        .iter()
        .map(|entry| (entry.name.as_str(), entry.score))
        .collect::<BTreeMap<_, _>>();
    loss += ordering_loss(
        score_by_name.get("critical").copied(),
        score_by_name.get("high").copied(),
        score_by_name.get("medium").copied(),
        score_by_name.get("unmaintained").copied(),
        score_by_name.get("yanked").copied(),
        score_by_name.get("policy-only").copied(),
    );

    for repo in local_repos {
        if repo.engine_status != ManagedEngineStatus::Managed {
            continue;
        }
        let outcome = score_rust_deps_features(&repo.features, &[], profile);
        if repo.features.policy_only()
            && outcome.score < i32::from(profile.combination.policy_only_floor) as u8
        {
            loss += i64::from(profile.combination.policy_only_floor - outcome.score) * 20;
        }
        if repo.features.advisory_unmaintained > 0
            && repo.features.total_advisories() == repo.features.advisory_unmaintained
            && !(58..=75).contains(&outcome.score)
        {
            loss += distance_to_band(outcome.score, 58, 75) * 6;
        }
        if repo.features.advisory_yanked > 0
            && repo.features.total_advisories() == repo.features.advisory_yanked
            && !(65..=78).contains(&outcome.score)
        {
            loss += distance_to_band(outcome.score, 65, 78) * 6;
        }
        let severe = repo.features.advisory_critical
            + repo.features.advisory_high
            + repo.features.advisory_medium
            + repo.features.advisory_unsound
            + repo.features.advisory_reported;
        if severe == 0 && !repo.features.policy_only() && outcome.score < 60 {
            loss += i64::from(60 - outcome.score) * 4;
        }
    }

    loss
}

fn evaluate_fixtures(
    profile: &RustDepsScoringProfile,
    fixtures: &[FixtureDefinition],
) -> Vec<FixtureResult> {
    fixtures
        .iter()
        .map(|fixture| {
            let outcome = score_rust_deps_features(&fixture.features, &[], profile);
            FixtureResult {
                name: fixture.name.clone(),
                description: fixture.description.clone(),
                score: outcome.score,
                cap: outcome.cap,
                cap_code: outcome.cap_code,
            }
        })
        .collect()
}

fn ordering_loss(
    critical: Option<u8>,
    high: Option<u8>,
    medium: Option<u8>,
    unmaintained: Option<u8>,
    yanked: Option<u8>,
    policy_only: Option<u8>,
) -> i64 {
    let mut loss = 0i64;
    let chain = [critical, high, medium, unmaintained, yanked, policy_only];
    for window in chain.windows(2) {
        if let [Some(left), Some(right)] = window {
            if left >= right {
                loss += i64::from(left.saturating_sub(*right) + 1) * 25;
            }
        }
    }
    loss
}

fn distance_to_band(score: u8, min: u8, max: u8) -> i64 {
    if score < min {
        i64::from(min - score)
    } else if score > max {
        i64::from(score - max)
    } else {
        0
    }
}

fn render_report(
    baseline: &RustDepsScoringProfile,
    tuned: &TuningOutcome,
    fixtures: &[FixtureDefinition],
    local_repos: &[LocalRepoSample],
) -> String {
    let baseline_loss = evaluate_loss(baseline, fixtures, local_repos);
    let baseline_fixture_results = evaluate_fixtures(baseline, fixtures);
    let managed = local_repos
        .iter()
        .filter(|repo| repo.engine_status == ManagedEngineStatus::Managed)
        .count();
    let degraded = local_repos.len().saturating_sub(managed);

    let mut lines = vec![
        String::from("# Rust deps calibration report"),
        String::new(),
        format!("- baseline loss: `{baseline_loss}`"),
        format!("- tuned loss: `{}`", tuned.loss),
        format!("- fixtures: `{}`", fixtures.len()),
        format!("- local Rust repos: `{}`", local_repos.len()),
        format!("- managed local repos: `{managed}`"),
        format!("- degraded local repos: `{degraded}`"),
    ];

    if local_repos.len() < 3 {
        lines.push(String::from(
            "- warning: local Rust corpus is small, so fixture expectations are carrying most of the precision.",
        ));
    }

    lines.push(String::new());
    lines.push(String::from("## Fixture results"));
    lines.push(String::new());
    lines.push(String::from(
        "| fixture | target | baseline | tuned | cap | code | note |",
    ));
    lines.push(String::from("| --- | --- | --- | --- | --- | --- | --- |"));
    for (fixture, baseline_result, tuned_result) in fixtures
        .iter()
        .zip(baseline_fixture_results.iter())
        .zip(tuned.fixture_results.iter())
        .map(|((fixture, baseline_result), tuned_result)| (fixture, baseline_result, tuned_result))
    {
        lines.push(format!(
            "| {} | {}-{} | {} | {} | {} | {} | {} |",
            fixture.name,
            fixture.score_min,
            fixture.score_max,
            baseline_result.score,
            tuned_result.score,
            tuned_result
                .cap
                .map(|value| value.to_string())
                .unwrap_or_else(|| String::from("-")),
            tuned_result
                .cap_code
                .clone()
                .unwrap_or_else(|| String::from("-")),
            tuned_result.description
        ));
    }

    lines.push(String::new());
    lines.push(String::from("## Tuned profile"));
    lines.push(String::new());
    lines.push(String::from("```toml"));
    lines.push(render_rust_deps_profile_toml(&tuned.profile));
    lines.push(String::from("```"));
    lines.push(String::new());
    lines.push(String::from("## Local corpus"));
    lines.push(String::new());
    for repo in local_repos {
        let outcome = score_rust_deps_features(&repo.features, &[], &tuned.profile);
        lines.push(format!(
            "- `{}` -> `{}` ({})",
            repo.path.display(),
            outcome.score,
            repo.engine_status.label()
        ));
    }

    lines.join("\n")
}

fn write_string(path: &Path, contents: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)
}
