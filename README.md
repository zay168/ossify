# ossify

Turn any repository into an open-source-ready project in minutes.

`ossify` is a Rust CLI for maintainers who want their repos to look serious fast. It audits the essentials, scores the repository, and can scaffold or autofix the missing community files so the project feels trustworthy before the first wave of stars.

## Why this is compelling

Most promising repositories lose momentum because they look unfinished:

- no license
- no contribution guide
- no code of conduct
- no issue templates
- no CI workflow

`ossify` makes that gap painfully obvious, then fixes the boring parts for you.

## MVP

- Audit a local repository and compute an "open source readiness" score
- Detect the most important community-health files and workflows
- Scaffold missing files with usable starter content
- Autofix missing repository-health files with one command
- Emit JSON for CI, bots, and automation
- Ship polished terminal output with color-coded status
- Keep everything local, simple, and hackable

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

If you run `ossify` without arguments, it audits the current directory.

## Example

```text
> ossify audit .

OSSIFY REPORT
Target: .
Open source readiness score: 47/100

Healthy
  [ok] README
  [ok] License
  [ok] Project manifest

Missing or weak
  [missing] Contributing guide
  [missing] Code of conduct
  [missing] Security policy
  [missing] Issue templates
  [missing] Pull request template
  [missing] CI workflow

Next move:
  ossify fix . --license mit --owner "Acme Maintainers"
```

```text
> ossify fix . --json

{
  "command":"fix",
  "target":"C:\\repo",
  "before_score":47,
  "after_score":95,
  "score_delta":48
}
```

## Project structure

```text
src/
  audit.rs
  cli.rs
  generator.rs
  report.rs
  templates.rs
  main.rs
```

## Product direction

This can become a very attractive open source tool because the value proposition is immediate:

- maintainers get a cleaner repo in one command
- contributors see a more trustworthy project surface
- teams can standardize repository hygiene across languages

Current V2 direction:

- `ossify fix` for safe autofixes based on audit findings
- JSON output for CI and bots
- GitHub release packaging for Linux, macOS, and Windows

Future directions:

- GitHub/GitLab presets
- score badges for README integration
- template packs for SaaS, libraries, CLIs, and SDKs
- AI-assisted README and release notes generation

This repository is also dogfooding the idea: it already includes the files `ossify` expects healthy open source projects to expose.

## Build

Once Rust is installed:

```bash
cargo build
cargo run -- audit .
cargo run -- fix . --license mit --owner "Acme Maintainers"
```

Tagged releases can be published through GitHub Actions and will attach packaged binaries for:

- Linux `x86_64-unknown-linux-gnu`
- macOS `x86_64-apple-darwin`
- Windows `x86_64-pc-windows-msvc`

## Notes

This workspace did not have Rust installed when the project was scaffolded, so the code has been authored to be straightforward to compile once `cargo` is available.
