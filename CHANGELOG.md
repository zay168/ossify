# Changelog

All notable changes to this project will be documented in this file.

The project intends to follow SemVer. Breaking changes and experimental areas should be called out clearly in release notes.

## Unreleased

### Added

- `ossify doctor docs` for focused Markdown quality checks, including broken local links, missing anchors, and README hygiene.
- `ossify doctor workflow` for subprocess-based GitHub Actions checks through `actionlint`, with automatic managed-engine bootstrap when the external binary is missing.

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
