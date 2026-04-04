# Changelog

All notable changes to this project will be documented in this file.

The project intends to follow SemVer. Breaking changes and experimental areas should be called out clearly in release notes.

## Unreleased

## 0.2.0 - 2026-04-04

### Added

- `ossify doctor docs` for focused Markdown quality checks, including broken local links, missing anchors, and README hygiene.
- `ossify doctor workflow` for subprocess-based GitHub Actions checks through `actionlint`, with automatic managed-engine bootstrap when the external binary is missing.
- `ossify doctor deps` with managed-engine paths for Rust (`cargo-deny`), Node (`audit-ci`), and Python (`pip-audit`), plus explicit degraded fallback when those engines cannot run cleanly.
- `ossify doctor release` with managed-engine verification for Rust (`release-plz`, `git-cliff`, `cargo-dist`) and Node/Python (`release-please`) on top of the retained release heuristics.
- `audit` domain scores for `docs`, `workflow`, `deps`, and `release`, exposed in both human and JSON output.
- upstream governance notes under `knowledge/upstreams/` plus `ATTRIBUTION.md` for tracking substantive reuse.
- `.github/release.yml` for grouped GitHub release notes with install links and stable public framing.
- `RELEASING.md` for the local maintainer release checklist, validation order, and release-editing responsibilities.

### Changed

- `audit` now treats domain doctors as downward pressure and hard caps, rather than allowing them to inflate weak repositories artificially.
- terminal audit output now includes domain-level score context in addition to rule-category scoring.
- managed Node and Python tool sandboxes are now bootstrapped lazily on first use instead of requiring global installs.
- Rust dependency advisories are now normalized into finer-grained `cargo-deny` classes (`critical`, `high`, `medium`, `unmaintained`, `yanked`, `unsound`, `informational`, `reported`) instead of the older blunt vulnerability bucket.
- Rust deps scoring is now driven by a versioned profile at `knowledge/calibration/rust-deps-profile.toml`, and `doctor deps` exposes the advisory class that actually capped the score.
- maintainers now have a deterministic `ossify-calibrate` tool for tuning Rust deps weights and caps locally from fixtures plus nearby Rust repos, with cached feature extraction and no runtime AI.
- the Netlify landing now presents install, proof, and release surfaces as one coherent public entrypoint instead of separating delivery, docs, and release story.
- installer examples and public homepage metadata now point to the active Netlify landing surface.
- release artifacts are now prepared to publish with checksum sidecars in addition to archives.

## 0.1.0 - 2026-03-30

### Added

- Maintainer-grade repository audits with category scoring and repo profiling.
- Optional interactive terminal exploration for `audit` and `fix --plan`.
- One-shot and rule-specific prompt generation for external coding workflows.
- Offline causal diagnostics with repo indexing, knowledge packs, and local Git history signals.
- Native installer scripts for PowerShell and POSIX shells.
- Public landing assets and a React-based landing workspace for product presentation.

### Changed

- Human-readable output was upgraded from a classic ANSI CLI layout to a richer terminal UX.
- JSON and interactive detail views now expose more precise diagnostic causes, evidence, and context.
- Repository scaffolding flows now cover more GitHub-aware trust files while staying conservative on manual content.
