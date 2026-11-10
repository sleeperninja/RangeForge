# RangeForge

**One tool that takes an album or a pile of audio files, evens out their volume, splits CUE albums, fixes the tags, and writes clean files.**

It replaces about seven tools (MediaHuman, an FFmpeg GUI, Adobe Audition, a PowerShell volume script, Mp3tag, and manual Last.fm/genre lookups). It is a **personal tool first**. Nothing here is built for other users yet.

## The idea in four lines

1. Drop files in. The app reads each file **once** and measures loudness, true peak and range.
2. You see a table of what *would* happen. Changing the target volume updates it instantly, because the new gain is simple arithmetic on measurements we already have.
3. Press **Run**. Files are split, gained, tagged and encoded from the same in-memory audio.
4. Measurements are cached on disk, so dropping the same album again costs nothing.

## Where things stand

- The project is **rebuilt in Rust** (the old Electron/React version is in git history, commit `01c53ea`). It is **free to use**, with an optional Ko-fi link in the About window (D-42). Pre-1.0, version in `Cargo.toml`.
- The UI mockups in `docs/mockups/` are the source of truth for how the app looks and behaves.
- Built: measure once, CUE splitting, ALAC/FLAC/WAV(+32-bit float)/AAC/MP3/Opus output, conversions (rate, depth, channel mix), File Strategy, presets, names and tags, Last.fm album genres, Radio tag, About window. Not yet done: a hand click-through of the newest window, the first CI installer build, a licence file. The list is in [ROADMAP.md](ROADMAP.md) under *Before the first release*.

## Two kinds of documents

| For people (plain words, no decision numbers) | For agents (precise, with decision numbers) |
| --- | --- |
| [README.md](README.md), [MANUAL.md](MANUAL.md), [FAQ.md](FAQ.md), [ROADMAP.md](ROADMAP.md), [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) | this file and everything in [docs/](docs/) |

| Agent document | What it answers |
| --- | --- |
| [docs/01-ui.md](docs/01-ui.md) | What the screen shows and how each control behaves (the spec) |
| [docs/02-architecture.md](docs/02-architecture.md) | How it works inside, in plain words first, detail later |
| [docs/03-decisions.md](docs/03-decisions.md) | Every past decision, why, and whether it still stands |
| [docs/04-development.md](docs/04-development.md) | How to build, test and release |
| Git history (commit `01c53ea`) | The old plans, market research, wireframes and Electron code. Not kept in the tree. |

## Rules for any AI (or person) working here

- **Docs lead with plain language.** Put the summary first and the detail in later sections.
- **Before suggesting something, check [docs/03-decisions.md](docs/03-decisions.md).** If a decision is marked *Rejected*, its reason is listed. Raise it again only with a new reason.
- **Do not memory-map files on network shares.** Local files only. See decision D-02.
- **Never put API keys or secrets in the repo or docs.** They live in the app's Settings (stored encrypted) and in a git-ignored `.env`.
- **Never change the waveform except by one gain per song,** and never past the true-peak ceiling. No compression or limiting. See decision D-12.
- **Tests that write or delete files use generated audio or copies in temporary folders.** `testdata/` is read-only, and tests never touch the real Recycle Bin.
- **Update the docs in the same change as the code.** There is no separate session-close ritual.
- **The manual explains, the decisions ledger records.** When a decision changes how something works, update both: [MANUAL.md](MANUAL.md) in plain words (no `D-` numbers, no dates, written for a user who has never seen this repo), and `docs/03-decisions.md` with the date and the reason. Keep the screen spec in `docs/01-ui.md` in step. Questions people ask belong in [FAQ.md](FAQ.md).
- **Keep one source of truth.** If two documents disagree, fix one of them. Do not add a third.

## Code standards (Rust)

- Layout: `crates/rangeforge-core` is a pure library (no UI, minimal I/O). `rangeforge-audio` decodes and measures, `rangeforge-export` writes files, `rangeforge-lookup` matches songs against outside data (the Hot 100), and `rangeforge-session` holds the rules the screens share. `rangeforge-cli` (terminal, a developer tool) and `rangeforge-app` (the desktop window, the real product) are thin front ends on top of those.
- `unsafe` is allowed only inside one small memory-map wrapper, with a comment explaining why it is sound.
- Errors are typed (`thiserror`). No `unwrap()` or `expect()` outside tests.
- Everything must pass `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace`.
- Loudness numbers are verified against FFmpeg's `ebur128` filter on real files. Speed claims are backed by `criterion` benchmarks.
- Targets, in priority order: Windows x64, macOS Apple Silicon, macOS Intel, Linux GNU, Windows ARM.

## Working in this repo

- The folder is on a network share (`\\yggdrasil.local\...`). Git needs `safe.directory` set for it, and `cargo` should use a **local** target directory: set `CARGO_TARGET_DIR` to something like `C:\Local\rangeforge-target`. Details in [docs/04-development.md](docs/04-development.md).
- Two GitHub SSH identities exist on this machine. Check which one `origin` uses before pushing.

## Resuming after a long session or a context reset (for the AI)

Long sessions get summarized, and details are lost. The files, not the summary, are the memory.

1. Run `git log -5` and `git status`, then read **Where things stand** above and [ROADMAP.md](ROADMAP.md). That says what is done and what is next.
2. Check [docs/03-decisions.md](docs/03-decisions.md) before proposing anything. Settled items (for example D-12: gain only, the true-peak ceiling is the limit) are not reopened without a new reason.
3. Before saying a change is done, run the three checks under *Code standards*. Say plainly what passed and what could not be tested (for example the interactive keyboard loop, or CI results the AI cannot see).
4. Commit as you go, and delete only with a go-ahead. Write commit messages to a file and use `git commit -F`. **Bump and push** (D-48): when a group of related commits is finished and the three checks pass, bump the patch version (`Cargo.toml`, `crates/rangeforge-app/tauri.conf.json`, and the version text in `README.md`, `MANUAL.md` and `ROADMAP.md`), commit it as `Version x.y.z`, and push. Do not push single commits mid-group, and do not push with a check failing. If the push is rejected, `git pull --rebase` and push again; never force. A change of minor version (0.x) is the user's call.
5. Keep **Where things stand** short and [ROADMAP.md](ROADMAP.md) friendly. If either grows into a wall of text, shorten it.
