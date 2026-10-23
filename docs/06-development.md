# 06 - Development

## Quick start

```powershell
# one-time
winget install Rustlang.Rustup          # installs Rust
winget install Gyan.FFmpeg              # FFmpeg for encoding and comparison tests
# Windows only: Rust needs Microsoft's C++ linker. Build Tools is just the compiler and
# SDK (no IDE). Needs admin approval.
winget install Microsoft.VisualStudio.2022.BuildTools --override "--quiet --wait --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"

# every session (this repo is on a network share, so build locally)
$env:CARGO_TARGET_DIR = 'C:\Local\rangeforge-target'

cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p rangeforge-cli -- --help
```

## Tests that use real audio

Two kinds of tests exist:

- **Normal tests** run everywhere, including on GitHub. They use small text fixtures kept in the repo.
- **Real-audio tests** read an album from the git-ignored `testdata/` folder, so they are skipped by default. Run them yourself, in release mode because decoding is slow in debug builds:

```powershell
cargo test -p rangeforge-audio --release -- --ignored
```

They expect the "Houses of the Holy" rip (FLAC + CUE + four separate ALAC tracks). Try the tool by hand:

```powershell
cargo run --release -p rangeforge-cli -- measure "testdata\<album folder>\Houses of the Holy.flac" --cue "testdata\<album folder>\Houses of the Holy.cue"
```

## The analysis cache

`rangeforge measure` remembers its results so a file is only ever decoded once. Entries live in the per-user cache folder, `%LOCALAPPDATA%\RangeForge\cache\analysis` on Windows, one small JSON file per audio file.

- `--no-cache` ignores the cache and always decodes.
- `--cache-dir <folder>` keeps the cache somewhere else, which is handy for experiments.
- Deleting the folder is always safe. It just means everything is measured again.

## If cargo cannot download crates

On a corporate network cargo may fail with `CRYPT_E_NO_REVOCATION_CHECK`. The machine cannot reach the endpoints that check whether a certificate was revoked. Workaround, per terminal session only:

```powershell
$env:CARGO_HTTP_CHECK_REVOKE = 'false'
```

This turns off one security check for cargo's downloads, so prefer asking IT to allow the revocation endpoints. It is deliberately not saved in any config file.

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
- **A Last.fm key and secret were committed in the old `PROJECT_PLAN.md`.** They are no longer in the tree but remain in git history (commit `01c53ea` and earlier). Rotate them on the Last.fm account.
