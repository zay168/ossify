# Upstream Absorption Notes

`ossify` is growing by reusing strong upstream ideas and policy models instead of inventing every rule from scratch.

This directory documents, for each upstream source:

- what `ossify` absorbs today
- what stays out of scope for now
- how parity is checked before a rule is treated as credible

## Absorption Waves

1. Findings, policy language, fixtures, and representative edge cases
2. Portable analysis logic that fits `ossify`'s Rust core cleanly
3. Integration into `doctor`, `audit`, scoring, and terminal/JSON rendering

## Current Strategy

- `docs` and `workflow` already combine `ossify` logic with external policy/tooling.
- `deps` and `release` now combine managed subprocess engines with absorbed heuristics inspired by mature upstream projects.
- Direct code vendoring is intentionally conservative; when substantive code or fixtures are copied, they should also be tracked in [`ATTRIBUTION.md`](../../ATTRIBUTION.md).

## Parity Rules

- A new absorbed rule should be traceable to a specific upstream concept.
- `ossify` should keep the user-visible recommendation actionable, even when the upstream tool would emit a lower-level diagnostic.
- When a rule becomes score-impacting, there should be at least one regression test in `ossify` that exercises the intended behavior.
