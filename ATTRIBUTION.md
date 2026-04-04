# Attribution

This file tracks substantive upstream reuse in `ossify`.

## Current State

As of this tranche, `ossify` still does **not** vendor large blocks of upstream source code for `deps` and `release`.

Instead, it now orchestrates managed subprocess engines and absorbs/document upstream policy models from:

- `cargo-deny`
- `audit-ci`
- `pip-audit`
- `release-plz`
- `git-cliff`
- `cargo-dist`
- `release-please`

Those sources are documented in [`knowledge/upstreams/README.md`](knowledge/upstreams/README.md) and the sibling notes in that directory.

## When To Update This File

Update this file whenever `ossify` begins to:

- vendor upstream code
- vendor upstream test fixtures
- port a substantive algorithm line-for-line or nearly line-for-line
- ship copied templates or policy files derived directly from an upstream repository

At that point, record:

- upstream repository and URL
- license
- exact files or logic reused
- any local modifications
