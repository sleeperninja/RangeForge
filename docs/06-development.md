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

Useful options: `--target` and `--ceiling` set the volume, `--codec alac|flac|wav|aac`, `--bit-depth source|16|24`, `--container native|mka`, `--sample-rate original|44100|48000|88200|96000|176400|192000` (AAC stops at 96000), `--out <folder>` (everything into one folder) or `--subfolder <name>` (a folder of that name inside each original's folder), `--on-conflict number|rename-existing|overwrite` with `--suffix`, `--delete-original`, and `--yes` to confirm overwriting or deleting. File names follow `--name "{track:02} - {title}"` for CUE albums and `--single-name "{name}"` for single files (the command line does not read the window's saved settings). With none of these, files are written next to the sources under their new names, a number such as `_00` is added only if that name is taken, and nothing existing is touched.

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

Use a real terminal such as Windows Terminal for the interactive version. The keys are listed in [02-ui.md](02-ui.md). Starting values can be set with `--target`, `--ceiling`, `--no-peak-protection`, `--codec`, `--bit-depth`, `--container`, `--sample-rate`, `--out`, `--subfolder`, `--on-conflict`, `--suffix` and `--delete-original` (the same options as `run`), and `--show-logs` opens the log panel. It uses the analysis cache below, so an album you have measured before appears instantly.

Pressing Run needs FFmpeg. The program looks, in order, at `RANGEFORGE_FFMPEG`, an `ffmpeg` file next to the program, the system path, and the usual package folders (`/opt/homebrew/bin`, `/usr/local/bin`, `/usr/bin`). If it cannot be found, the screen says so and nothing is written. On a Mac with no Homebrew, drop an `ffmpeg` binary in the same folder as `rangeforge-app`.

Presets live in `presets.json` in your settings folder (`%APPDATA%\RangeForge\config` on Windows). Use `--config-dir <folder>` to keep them somewhere else, which is handy for experiments. A file that cannot be read is set aside as `presets.json.bad` and the built-in presets are used, so a damaged file never stops the app and is never silently overwritten.

To see a whole run without a terminal, write to a scratch folder and print the finished screen:

```powershell
cargo run --release -p rangeforge-cli -- ui --snapshot --run --show-logs --out "C:\temp\out" --config-dir "C:\temp\cfg" "testdata\<album folder>"
```

## The desktop window

```powershell
cargo run --release -p rangeforge-app                      # opens the window
cargo run --release -p rangeforge-app -- "testdata\<album folder>"   # opens it with files already added
node --test crates/rangeforge-app/frontend/render.test.js  # tests of how rows are displayed
```

The window is a Tauri program. The page lives in `crates/rangeforge-app/frontend/` as plain HTML, CSS and JavaScript, so building needs only Rust. It shows what `rangeforge-session` describes (a `Snapshot`) and sends the user's actions back; every rule stays in Rust. Drop files or folders on it, or use Add files. Outside the program, `index.html` shows sample data from `demo-state.json`, which is how the layout is checked without a window. On Linux it needs the WebKitGTK libraries (see `ci.yml`). It also needs FFmpeg for Run, like the terminal view.

Downloads to try come from the Release workflow (see Releases below), not from CI.

## Metadata lookups

Turn on **Metadata API** in the window and the Hot 100 list is fetched and kept in the cache folder (`hot100.json`, next to the `analysis` folder). Songs that charted get a blue **Radio** chip.

For Last.fm, put `LASTFM_API=<your key>` in a `.env` file in the repository folder. It is read in debug builds only (`cargo run -p rangeforge-app`) and is already ignored by git. A release build, or the installer, uses the key saved in Settings, or `LASTFM_API` if it is set in the environment. Then tick **Use Last.fm** in Settings (and Metadata API on the main screen). Its answers are remembered in `lastfm.json` in the cache folder; delete that file to ask everything again.

The Last.fm code has only been tested with canned answers, never with the live service, so the first real run is worth watching: open the log panel and look for lines such as "Could not reach Last.fm" or "does not accept the API key". Reading the log is also the way to see what was asked.

The Hot 100 matching has one test against the real chart, which is not in git. Download `all.json` (see the FAQ for its address) and run:

```powershell
$env:RANGEFORGE_HOT100_JSON = "C:\temp\all.json"
cargo test -p rangeforge-lookup --release -- --ignored
```

All other tests use small made-up charts and never touch the network.

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

Checks are run on your own machine before merging. CI (`.github/workflows/ci.yml`) is only started by hand from the Actions tab (Windows x64 and macOS Apple Silicon), so free Actions minutes are kept for releases.

Also planned: `cargo deny` (licences, advisories), `criterion` benchmarks, and a comparison test against FFmpeg's `ebur128`.

## Releases

Installers are built by the **Release** workflow (`.github/workflows/release.yml`). It runs when something is merged into `main`, and puts the installers on a draft pre-release named after the version in `Cargo.toml` (so raise the version before merging). Start it by hand from the Actions tab to build without making a release. It makes:

| Platform | What you get |
| --- | --- |
| Windows x64 | `RangeForge_<version>_x64_en-US.msi` |
| macOS Apple Silicon and Intel | a `.dmg` holding `RangeForge.app`, one per chip |
| Linux GNU, Windows ARM | a `.deb` and an `.msi`, allowed to fail (not hand-tested) |

The installers are **not signed**. On Windows, SmartScreen says the publisher is unknown: choose More info, then Run anyway. On macOS the app is signed ad-hoc (which Apple Silicon requires), but a downloaded app is still quarantined: open the DMG, drag the app out, then right-click it and choose Open the first time. Proper signing would need an Apple Developer account and a Windows certificate; that is a later decision (D-23).

**FFmpeg is inside the installer** (decision D-23), as an LGPL build named in `THIRD_PARTY_NOTICES.md`. `scripts/fetch-ffmpeg.sh <rust-target> <folder>` fetches it: the prebuilt LGPL build on Windows, and a build from the pinned source tag on macOS and Linux (about ten minutes the first time, then cached). The release workflow runs the export tests against that very FFmpeg before packaging anything. The file lands in `crates/rangeforge-app/binaries/` (not in git).

**Building on your own machine:** run `scriptsbuild-local.bat` on Windows (or `scripts/build-local.sh` in a terminal on a Mac or Linux). Each machine builds one thing: an MSI on Windows x64, an app and DMG on a Mac with Apple Silicon, a .deb on Linux. It first lists every requirement with ok or MISSING and how to install what is missing (Rust, the Rust target, Visual Studio C++ Build Tools, Xcode tools, the Tauri build tool, and so on) then fetches FFmpeg, runs the export tests, builds, and copies the installer to `dist/<version>/`. `--check` stops after that list. Below is the same thing by hand.

To build an installer on your own Windows machine, install the Tauri build tool once (`cargo install tauri-cli --version "^2" --locked`), put an FFmpeg at `crates\rangeforge-app\binaries\ffmpeg-x86_64-pc-windows-msvc.exe`, and run, from `crates\rangeforge-app`:

```powershell
cargo tauri build --config tauri.bundle.conf.json --bundles msi
```

The extra `--config` file is what adds FFmpeg and the notices, so an ordinary `cargo build` does not need them.

## Secrets

- API keys go in the app's Settings (stored encrypted) or a git-ignored `.env`.
- Never in docs, code, screenshots or mockups.
- **A Last.fm key and secret were committed in the old `PROJECT_PLAN.md`.** They are no longer in the tree but remain in git history (commit `01c53ea` and earlier). Rotate them on the Last.fm account.
