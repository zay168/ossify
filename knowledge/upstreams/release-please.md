# release-please

Upstream: [googleapis/release-please](https://github.com/googleapis/release-please)

## Absorbed Now

- release automation as an auditable repo surface
- expectation that versioning, changelog, and release flow should be explicit
- Node/Python release readiness heuristics

## Not Absorbed Yet

- conventional commit parsing
- PR synthesis
- manifest releaser behavior
- ecosystem-specific plugin parity

## Parity Check

- `ossify doctor release --ecosystem node|python` should expose whether the repo explains and automates shipping well enough to be trusted by a maintainer or adopter
