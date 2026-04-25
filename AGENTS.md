# AGENTS.md

## Repository expectations

- Do not change public behavior unless explicitly requested.
- Prefer small, reviewable diffs.
- Before refactoring, identify existing tests or add characterization tests.
- After changes, run:
  - cargo fmt
  - cargo clippy --all-targets --all-features
  - cargo test --all
- Do not edit secrets, .env files, tokens, keys, or local machine configs.
- If unsure, leave a TODO in the report instead of guessing.
