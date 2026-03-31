# Contributing to ossify

Thanks for helping shape `ossify`.

## What we want from contributions

- Clear, focused improvements
- Good developer experience for maintainers
- Practical value for real open source repositories

## Local setup

For the Rust CLI:

```powershell
cargo check
cargo test
cargo run -- audit .
cargo run -- fix . --plan
```

If you touch the landing or other frontend assets in `frontend-demo`:

```powershell
cd frontend-demo
npm ci
npm run build
```

## Workflow

1. Open an issue for larger ideas so we can align early.
2. Create a focused branch.
3. Keep the diff small and explain the reason for the change.
4. Add or update tests when behavior changes.
5. Open a pull request with context and tradeoffs.

## Contribution conventions

- Prefer small, reviewable pull requests over mixed refactors.
- If behavior changes, add or update tests.
- If user-facing output, install behavior, or workflow behavior changes, update the docs in the same pull request.
- If you add a new rule, keep it explainable and deterministic.
- If you touch generated files or templates, keep them conservative and maintainer-friendly.

## Product bar

If you add a feature, it should satisfy at least one of these:

- helps a maintainer look more professional quickly
- saves repetitive repository setup work
- improves contributor trust and clarity
- keeps the CLI simple and scriptable

## Join the project

`ossify` is currently maintained primarily by [`@zay168`](https://github.com/zay168).

If you are seriously interested in the project, regular contributions are welcome. The best way to get involved is to show up with focused pull requests, careful bug reports, documentation improvements, or thoughtful review feedback. People who contribute consistently and help the project move forward can grow into a more active maintainer role over time.
