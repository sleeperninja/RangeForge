# 06 - Development

## Quick start

```powershell
# one-time
winget install Rustlang.Rustup          # installs Rust
winget install Gyan.FFmpeg              # FFmpeg for encoding and comparison tests

# every session (this repo is on a network share, so build locally)
$env:CARGO_TARGET_DIR = 'C:\Local\rangeforge-target'

cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p rangeforge-cli -- --help
```

## The network share

The repo lives at `\\yggdrasil.local\...`.

- **Git** may say "dubious ownership". Fix once: `git config --global --add safe.directory '%(prefix)///yggdrasil.local/yggdrasil/Our_Stuff/Mikey/Projects/Dev/RangeForge'`.
- **Cargo** writes thousands of small files. Always set `CARGO_TARGET_DIR` to a local drive.
- **Memory mapping** must never target files on the share (decision D-02). Test audio should be copied locally first.
- Cloning the repo to a local drive and pushing from there is also fine.

## Git and GitHub

- Remote: `origin` on GitHub. This machine has two SSH identities, so confirm which one `origin` uses before pushing.
- Work happens on `dev`.

## Quality gates

Every change must pass:

1. `cargo fmt --all -- --check`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo test --workspace`

CI (`.github/workflows/ci.yml`) runs these on Windows x64, macOS Apple Silicon, macOS Intel and Linux. Windows ARM is added when a runner is available.

Also planned: `cargo deny` (licences, advisories), `criterion` benchmarks, and a comparison test against FFmpeg's `ebur128`.

## Releases

Target order: Windows x64, macOS Apple Silicon, macOS Intel, Linux GNU, Windows ARM. `cargo-dist` will build the artifacts. Each release bundles an FFmpeg binary per platform. This is planned for Phase 8.

## Secrets

- API keys go in the app's Settings (stored encrypted) or a git-ignored `.env`.
- Never in docs, code, screenshots or mockups.
- **A Last.fm key and secret were committed in the old `PROJECT_PLAN.md`.** They are redacted in the working tree but remain in git history. Rotate them on the Last.fm account.
