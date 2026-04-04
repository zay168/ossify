# Releasing `ossify`

This checklist is for maintainers cutting a public release.

## 1. Prepare the tree

- Decide the release version.
- Update [`CHANGELOG.md`](CHANGELOG.md) with a dated section for that version.
- Update `Cargo.toml` if the CLI version changes.
- Check that public landing/install links still point to the intended live surface.

## 2. Validate locally

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test`
- `cargo run --quiet -- audit . --no-color`
- `cargo run --quiet -- deps --ecosystem rust . --no-color`
- `cargo run --quiet -- release --ecosystem rust . --no-color`
- `cd frontend-demo && npm ci && npm run build:netlify`

If the release meaningfully touches landing or install flow, also verify:

- `pwsh -File docs/install.ps1 -PrintOnly`
- `bash -n docs/install.sh`

## 3. Tag and publish

- Commit the release prep.
- Create and push a version tag like `v0.2.0`.
- Let [`.github/workflows/release.yml`](.github/workflows/release.yml) build archives and checksums.
- Review the generated GitHub release notes, which are shaped by [`.github/release.yml`](.github/release.yml).

## 4. Verify public surfaces

- Check the latest GitHub release page.
- Verify installer URLs:
  - `https://ossify-react.netlify.app/install.ps1`
  - `https://ossify-react.netlify.app/install.sh`
- Verify the landing:
  - `https://ossify-react.netlify.app/ossify/`

## 5. Post-release cleanup

- Move the released notes out of `Unreleased` in [`CHANGELOG.md`](CHANGELOG.md).
- Start a new `Unreleased` section immediately for the next tranche.
- If the release changed messaging or public commands, align the landing and README in the next follow-up commit rather than letting drift accumulate.
