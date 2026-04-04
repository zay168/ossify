# cargo-deny

Upstream: [EmbarkStudios/cargo-deny](https://github.com/EmbarkStudios/cargo-deny)

## Absorbed Now

- visible dependency policy expectation via `deny.toml`
- license visibility checks
- git/path/custom-registry dependency surfacing
- wildcard version skepticism for Cargo dependencies
- managed advisory execution through `cargo-deny`
- normalized Rust advisory classes inside `ossify` (`critical`, `high`, `medium`, `unmaintained`, `yanked`, `unsound`, `informational`, `reported`)
- fixture-driven Rust deps calibration profile layered on top of managed `cargo-deny` findings

## Not Absorbed Yet

- full advisory database integration
- SPDX expression solving and license allow/deny semantics
- source allowlists, bans, and target-specific policy evaluation

## Parity Check

- `ossify doctor deps --ecosystem rust` should flag the same broad trust boundaries that a maintainer would expect before adopting `cargo-deny`
- fixtures should cover missing `Cargo.lock`, wildcard versions, non-crates.io sources, and advisory-class ordering/caps
- a single `unmaintained` advisory should not score like a `critical` or `high` exploit path
