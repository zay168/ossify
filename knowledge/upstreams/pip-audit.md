# pip-audit

Upstream: [pypa/pip-audit](https://github.com/pypa/pip-audit)

## Absorbed Now

- dependency-audit framing for Python repos
- visibility expectations around lockfiles and repeatable environments
- direct-source and editable install surfacing
- packaging metadata expectations that make release and dependency surfaces auditable

## Not Absorbed Yet

- vulnerability database integration
- fix version suggestions
- resolver-specific dependency graph semantics

## Parity Check

- `ossify doctor deps --ecosystem python` should tell the truth about reproducibility and dependency boundary visibility even before live vulnerability lookup exists
