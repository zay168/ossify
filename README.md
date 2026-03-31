<h1 align="center">ossify</h1>

<p align="center"><strong>Audit a repository like a maintainer would, then scaffold the missing trust signals.</strong></p>

<p align="center">
  <a href="https://github.com/zay168/ossify/blob/main/LICENSE"><img alt="License MIT" src="https://img.shields.io/badge/license-MIT-0f172a?style=flat-square&labelColor=0f172a&color=7dd3fc"></a>
  <a href="https://github.com/zay168/ossify/actions/workflows/ci.yml"><img alt="CI status" src="https://img.shields.io/github/actions/workflow/status/zay168/ossify/ci.yml?branch=main&style=flat-square&label=CI&labelColor=0f172a&color=a3e635"></a>
  <img alt="Profiles" src="https://img.shields.io/badge/profiles-library%20%2F%20cli%20%2F%20app-0f172a?style=flat-square&labelColor=0f172a&color=fbbf24">
  <img alt="Output" src="https://img.shields.io/badge/output-human%20%2B%20json-0f172a?style=flat-square&labelColor=0f172a&color=c4b5fd">
</p>

<p align="center"><code>ossify audit . --interactive</code> <span>&nbsp;&nbsp;->&nbsp;&nbsp;</span> <code>ossify fix . --plan --interactive</code> <span>&nbsp;&nbsp;->&nbsp;&nbsp;</span> <code>ossify fix . --license mit --owner "@acme"</code></p>

## Install

Install the latest native binary without Rust:

```powershell
irm https://zay168.github.io/ossify/install.ps1 | iex
```

```sh
curl -fsSL https://zay168.github.io/ossify/install.sh | sh
```

The bootstrap installer also installs the managed workflow engine currently used by `ossify doctor workflow` so that GitHub Actions checks work immediately after install.
If that engine is missing later, `ossify doctor workflow` will try to bootstrap the managed copy automatically before falling back to a warning.

Landing page: [zay168.github.io/ossify](https://zay168.github.io/ossify/)

## Project Status

`ossify` is actively maintained by [@zay168](https://github.com/zay168).

The project is ready for real repository audits and scaffolding workflows, but it is still evolving in public. The CLI surface is intended to stay stable when possible, breaking changes are called out explicitly, and serious contributors are welcome.

## Why

`ossify` focuses on the open source trust layer of a repository:

- package identity and manifest metadata
- README quality and examples
- contributor and security docs
- CI, tests, lint, and release hygiene
- GitHub-aware files like `CODEOWNERS`, `dependabot.yml`, and release workflows

The goal is simple: turn a vague repo surface into something that looks maintained, legible, and safer to adopt.

## Compatibility and Support

`ossify` targets:

- Windows, with PowerShell-first installer and CLI flows
- macOS, with a POSIX shell installer
- Linux, with a POSIX shell installer

Support is provided on a best-effort basis. The current release line is the priority, and older versions may not receive the same level of compatibility attention.

## Versioning

`ossify` intends to follow Semantic Versioning.

- breaking changes are called out in the changelog and release notes
- new functionality may ship as experimental when the workflow still needs real-world validation
- installer, CLI, and output improvements are expected to stay conservative unless a release explicitly says otherwise

## V4

V4 makes `ossify` much more precise than the earlier checklist model.

- audit rules are now structured and scored by category
- repos are profiled as `library`, `cli`, `app`, or `generic`
- scoring can be tuned with a local `ossify.toml`
- `audit --strict` can fail CI when the score or required rules do not pass
- `fix` can scaffold more GitHub-aware files while staying conservative on content it cannot safely rewrite

## V4.2 Terminal UX

V4.2 makes `ossify` feel much more at home in a real terminal workflow.

- human output is now laid out as a richer terminal dashboard
- `audit --interactive` opens a keyboard-first explorer for checks and diagnostics
- `fix --plan --interactive` adds a plan view so you can inspect scaffold actions before touching the repo
- `--json` still bypasses the visual layer for CI, scripts, and machine consumers
- `--no-color` remains available when you want a quieter fallback

## Commands

```text
ossify audit [path] [--config ossify.toml] [--strict] [--interactive]
ossify doctor docs [path]
ossify doctor workflow [path]
ossify init [path] [--overwrite] [--license mit|apache-2.0] [--owner "@acme"] [--funding github:acme]
ossify fix [path] [--plan] [--interactive] [--overwrite] [--license mit|apache-2.0] [--owner "@acme"] [--funding github:acme] [--config ossify.toml]
ossify prompt [path] [--rule readme] [--count 0] [--config ossify.toml]
ossify version
```

Global flags:

```text
--json
--color
--no-color
```

## Usage

Start with the trust-layer audit, then drill into documentation quality when you want a more focused pass:

```text
ossify audit .
ossify doctor docs .
ossify doctor workflow .
ossify fix . --plan
ossify prompt .
```

## Example

```text
> ossify audit . --interactive

OSSIFY REPORT
Project: ossify
Target: .
Score: 84/100
Tier: promising

Top gaps
- partial | README | install and usage flow still feel thin
- missing | Dependabot | no update policy is present
- partial | lint and format signals | commands exist but CI does not surface them yet
```

```text
> ossify fix . --plan --interactive

OSSIFY PLAN
Current score: 63/100 -> Estimated: 91/100 (+28)

Would scaffold files
  [created] README.md
  [created] .github/workflows/ci.yml
  [created] .github/dependabot.yml

Blocked or skipped
  [skipped] .github/FUNDING.yml | FUNDING.yml generation requires --funding, for example github:acme.

Still manual after plan
  [partial] README examples and docs quality still need maintainer review
  [missing] Real release notes still need editorial ownership
```

```text
> ossify fix . --plan --license mit --owner "@acme" --funding github:acme --json

{"command":"fix","mode":"plan","before_score":63,"estimated_after_score":91,"score_delta":28}
```

```text
> ossify prompt .

OSSIFY PROMPT
Prompt style: one-shot
Issues in scope: 6

Copy/Paste Prompt
You are taking one repository-wide bug-fix pass on `C:\repo`.

Mission
- Resolve the prioritized gaps below in one coherent change set.
- Preserve working behavior and the strong signals already present.

Prioritized issues to fix
1. README (`readme`)
2. License (`license`)
3. CI workflow (`ci_workflow`)
...
```

## `ossify.toml`

```toml
version = 1
profile = "cli"
minimum_score = 90

[defaults]
owner = "@acme"
license = "mit"
funding = "github:acme"

[weights]
docs = 1.2
automation = 1.1

[rules.readme]
required_level = "strong"

[rules.release_workflow]
weight = 10
```

## What it can scaffold

- `README.md`
- `LICENSE`
- `CONTRIBUTING.md`
- `CODE_OF_CONDUCT.md`
- `SECURITY.md`
- `CHANGELOG.md`
- `.github/ISSUE_TEMPLATE/bug_report.md`
- `.github/ISSUE_TEMPLATE/feature_request.md`
- `.github/PULL_REQUEST_TEMPLATE.md`
- `.github/CODEOWNERS`
- `.github/FUNDING.yml`
- `.github/dependabot.yml`
- `.github/workflows/ci.yml`
- `.github/workflows/release.yml`

`fix --plan` is a dry run: it previews the files `fix` would write, keeps guided skips visible, and estimates the score impact without touching the repo.

`fix` remains conservative: it will not invent real tests, rewrite your manifest metadata, or replace editorial docs unless that file is already supported and you explicitly use `--overwrite`.

`prompt` turns the current audit into a copy-pasteable fix prompt for an external coding agent, a teammate, or another AI workflow. By default it generates one long repo-wide prompt that bundles the current non-strong rules into a one-shot fix pass. Use `--rule` to target one gap precisely, or `--count` to limit how many issues the one-shot prompt includes. `--count 0` means "include every current gap".

## Contributing

Contributions are welcome, especially when they improve audit precision, keep scaffolding conservative, or tighten the maintainer experience.

Start here:

- [CONTRIBUTING.md](CONTRIBUTING.md)
- [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md)

The project is maintained primarily by [@zay168](https://github.com/zay168), and recurring contributors who consistently improve the project are welcome to grow into a more active role over time.

## Security

Please do not open a public issue for an exploitable vulnerability. Report security concerns privately first and include enough detail to reproduce and assess impact.

- [SECURITY.md](SECURITY.md)

## Support

Bug reports, feature requests, and usage questions are welcome through the public issue tracker, with support handled on a best-effort basis.

- [SUPPORT.md](SUPPORT.md)

## Changelog

Release history and breaking-change notes live in:

- [CHANGELOG.md](CHANGELOG.md)
