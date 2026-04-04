# Rust Release Stack

Upstreams:

- [release-plz/release-plz](https://github.com/release-plz/release-plz)
- [orhun/git-cliff](https://github.com/orhun/git-cliff)
- [axodotdev/cargo-dist](https://github.com/axodotdev/cargo-dist)

## Absorbed Now

- visible changelog expectation
- explicit version signal expectation
- release workflow discoverability
- tag-history visibility
- distribution-surface visibility for binary-oriented Rust projects

## Not Absorbed Yet

- release PR orchestration
- changelog generation templating
- cargo-dist manifest and artifact parity
- release-note/body generation

## Parity Check

- `ossify doctor release --ecosystem rust` should answer: can a maintainer explain how this project versions, ships, and publishes from the repo alone?
