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

## Writing files: `rangeforge run`

```powershell
# preview first: every song's gain and where it would be written, nothing written
cargo run --release -p rangeforge-cli -- run "testdata\<album folder>" --out "C:\temp\album-out" --dry-run

# then write it (FFmpeg must be installed, or RANGEFORGE_FFMPEG must point at it)
cargo run --release -p rangeforge-cli -- run "testdata\<album folder>" --out "C:\temp\album-out"
```

Useful options: `--target` and `--ceiling` set the volume, `--codec alac|flac|wav|aac`, `--bit-depth source|16|24`, `--strategy serialize|rename-old|replace` with `--suffix`, `--out <folder>` to write elsewhere, and `--yes` to confirm a strategy that overwrites. With no `--out` and the default strategy, files are written next to the sources with a `_00` suffix, and nothing existing is touched.

**FFmpeg for the tests.** The export tests run the real FFmpeg on generated audio. They skip themselves on your machine if FFmpeg is missing, and fail on the build server (where `CI` is set) so they cannot be skipped there by accident. CI installs FFmpeg for each platform.

```powershell
$env:RANGEFORGE_FFMPEG = 'C:\path\to\ffmpeg.exe'   # only if it is not on your path
cargo test -p rangeforge-export                    # generated-audio tests
cargo test -p rangeforge-export --release --test real_album -- --ignored   # your real album
```

## Timing the decoder

To see how fast a file decodes with nothing else running (no meter, no encoder), use the example program. Put the files on a local disk first, or the network will be what you are timing:

```powershell
cargo run --release -p rangeforge-audio --example decode_speed -- C:\temp\album.flac C:\temp\album.m4a
```

## The interactive view

```powershell
# open it on an album folder (it uses the CUE sheet) and any extra files
cargo run --release -p rangeforge-cli -- ui "testdata\<album folder>"

# print one frame as text instead (no terminal needed), at a chosen target
cargo run --release -p rangeforge-cli -- ui --snapshot --target -18 --width 130 --height 25 "testdata\<album folder>"
```

Use a real terminal such as Windows Terminal for the interactive version. The keys are listed in [02-ui.md](02-ui.md). Starting values can be set with `--target`, `--ceiling`, `--no-peak-protection`, `--codec`, `--bit-depth`, `--strategy`, `--suffix` and `--out` (the same options as `run`), and `--show-logs` opens the log panel. It uses the analysis cache below, so an album you have measured before appears instantly.

Pressing Run needs FFmpeg (on the path, or named in `RANGEFORGE_FFMPEG`). If it cannot be found, the screen says so and nothing is written.

Presets live in `presets.json` in your settings folder (`%APPDATA%\RangeForge\config` on Windows). Use `--config-dir <folder>` to keep them somewhere else, which is handy for experiments. A file that cannot be read is set aside as `presets.json.bad` and the built-in presets are used, so a damaged file never stops the app and is never silently overwritten.

To see a whole run without a terminal, write to a scratch folder and print the finished screen:

```powershell
cargo run --release -p rangeforge-cli -- ui --snapshot --run --show-logs --out "C:\temp\out" --config-dir "C:\temp\cfg" "testdata\<album folder>"
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
