# Calibration assets

This directory holds maintainer-facing calibration inputs for `ossify`.

## Rust deps

- `rust-deps-profile.toml`
  The active scoring profile embedded by the runtime.
- `rust-deps-fixtures/`
  Fixture expectations that anchor Rust advisory ordering, caps, and score bands.

## Workflow

1. Run `cargo run --bin ossify-calibrate -- --max-repos 12`
2. Review:
   - `target/calibration/rust-deps/report.md`
   - `target/calibration/rust-deps/tuned-profile.toml`
3. If the tuned profile is better, copy it into `rust-deps-profile.toml` or rerun with `--write-profile knowledge/calibration/rust-deps-profile.toml`

The calibrator is deterministic and does not use online learning or runtime AI inference.
