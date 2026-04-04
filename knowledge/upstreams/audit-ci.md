# audit-ci

Upstream: [IBM/audit-ci](https://github.com/IBM/audit-ci)

## Absorbed Now

- policy-first framing for dependency gating
- expectation that lockfiles and visible audit policy belong in the repo
- skepticism toward direct-source and weakly pinned dependency specs

## Not Absorbed Yet

- full npm audit execution and severity threshold parity
- advisory allowlist parsing
- registry/provider-specific exception semantics

## Parity Check

- `ossify doctor deps --ecosystem node` should surface lockfile gaps, visible policy gaps, and risky source/version specs before a maintainer ever runs CI
