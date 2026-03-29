<h1 align="center">ossify</h1>

<p align="center"><strong>Make a repository feel trustworthy before people judge it.</strong></p>

<p align="center">
  Audit what matters. Fix what is missing. Ship a cleaner open source surface.
</p>

<p align="center">
  <a href="https://github.com/zay168/ossify/blob/main/LICENSE"><img alt="License MIT" src="https://img.shields.io/badge/license-MIT-0f172a?style=flat-square&labelColor=0f172a&color=7dd3fc"></a>
  <a href="https://github.com/zay168/ossify/actions/workflows/ci.yml"><img alt="CI status" src="https://img.shields.io/github/actions/workflow/status/zay168/ossify/ci.yml?branch=main&style=flat-square&label=CI&labelColor=0f172a&color=a3e635"></a>
  <img alt="JSON output" src="https://img.shields.io/badge/output-human%20%2B%20json-0f172a?style=flat-square&labelColor=0f172a&color=c4b5fd">
</p>

<p align="center"><code>ossify audit .</code> <span>&nbsp;&nbsp;->&nbsp;&nbsp;</span> <code>ossify fix . --license mit --owner "Acme Maintainers"</code></p>

## Why

`ossify` focuses on the trust layer of a repository:

- license clarity
- contributor guidance
- issue and pull request templates
- security and changelog basics
- CI and release hygiene

The goal is simple: help a repo look maintained, understandable, and safe to contribute to.

## Commands

```text
ossify audit [path]
ossify init [path] [--overwrite] [--license mit|apache-2.0] [--owner "Your Name"]
ossify fix [path] [--overwrite] [--license mit|apache-2.0] [--owner "Your Name"]
ossify version
ossify help
```

Global flags:

```text
--json
--color
--no-color
```

## Example

```text
> ossify audit .

OSSIFY REPORT
Target: .
Open source readiness score: 47/100

Healthy
  [ok] README (+15)
  [ok] License (+20)
  [ok] Project manifest (+5)

Missing or weak
  [missing] Contributing guide (+10, autofixable)
  [missing] Code of conduct (+10, autofixable)
  [missing] Security policy (+10, autofixable)
  [missing] Issue templates (+8, autofixable)
  [missing] Pull request template (+7, autofixable)
  [missing] CI workflow (+7, autofixable)

Next move
  ossify fix . --license mit --owner "Acme Maintainers"
```

```text
> ossify fix . --json

{"command":"fix","target":"C:\\repo","before_score":47,"after_score":95,"score_delta":48}
```

## What it scaffolds

- `README.md`
- `LICENSE`
- `CONTRIBUTING.md`
- `CODE_OF_CONDUCT.md`
- `SECURITY.md`
- `CHANGELOG.md`
- `.github/ISSUE_TEMPLATE/bug_report.md`
- `.github/ISSUE_TEMPLATE/feature_request.md`
- `.github/PULL_REQUEST_TEMPLATE.md`
- `.github/workflows/ci.yml`

## Release-ready

The repository already includes:

- CI for `cargo check` and `cargo test`
- GitHub release packaging for Linux, macOS, and Windows
- theme-aware README artwork that stays restrained and readable

Once Rust is installed:

```bash
cargo build
cargo run -- audit .
cargo run -- fix . --license mit --owner "Acme Maintainers"
```

## Roadmap

- `ossify fix --check` for CI gatekeeping without writing files
- presets for libraries, CLIs, SDKs, and SaaS repos
- score badges for README integration
- GitHub and GitLab-specific hygiene packs

## Notes

This workspace did not have Rust installed when the project was scaffolded, so the code has been authored but not compiled locally yet.
