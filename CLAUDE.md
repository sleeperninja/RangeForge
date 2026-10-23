# RangeForge

**One tool that takes an album or a pile of audio files, evens out their volume, splits CUE albums, fixes the tags, and writes clean files.**

It replaces about seven tools (MediaHuman, an FFmpeg GUI, Adobe Audition, a PowerShell volume script, Mp3tag, and manual Last.fm/genre lookups). It is a **personal tool first**. Nothing here is built for other users yet.

## The idea in four lines

1. Drop files in. The app reads each file **once** and measures loudness, true peak and range.
2. You see a table of what *would* happen. Changing the target volume updates it instantly, because the new gain is simple arithmetic on measurements we already have.
3. Press **Run**. Files are split, gained, tagged and encoded from the same in-memory audio.
4. Measurements are cached on disk, so dropping the same album again costs nothing.

## Where things stand

- The project is being **rebuilt in Rust**. The old Electron/React version is in git history (commit `01c53ea`). It was a plan more than a product, so we are not porting it.
- The UI mockups in `docs/mockups/` are the source of truth for how the app looks and behaves.
- Current stage: see [docs/05-roadmap.md](docs/05-roadmap.md).

## Read these, in this order

| Document | What it answers |
| --- | --- |
| [docs/01-overview.md](docs/01-overview.md) | Who is this for, what problem, what it does and does not do |
| [docs/02-ui.md](docs/02-ui.md) | What the screen shows and how each control behaves |
| [docs/03-architecture.md](docs/03-architecture.md) | How it works inside, in plain words first, detail later |
| [docs/04-decisions.md](docs/04-decisions.md) | Every past decision, why it was made, and whether it still stands |
| [docs/05-roadmap.md](docs/05-roadmap.md) | What gets built, in what order |
| [docs/06-development.md](docs/06-development.md) | How to build, test and release |
| [docs/faq.md](docs/faq.md) | Plain answers to recurring questions |
| Git history (commit `01c53ea`) | The old plans, market research, wireframes and Electron code. Not kept in the tree. |

## Rules for any AI (or person) working here

- **Docs lead with plain language.** Put the summary first and the detail in later sections.
- **Before suggesting something, check [docs/04-decisions.md](docs/04-decisions.md).** If a decision is marked *Rejected*, its reason is listed. Raise it again only with a new reason.
- **Do not memory-map files on network shares.** Local files only. See decision D-02.
- **Never put API keys or secrets in the repo or docs.** They live in the app's Settings (stored encrypted) and in a git-ignored `.env`.
- **Update the docs in the same change as the code.** There is no separate session-close ritual.
- **Keep one source of truth.** If two documents disagree, fix one of them. Do not add a third.

## Code standards (Rust)

- Layout: `crates/rangeforge-core` is a pure library (no UI, minimal I/O). `crates/rangeforge-cli` is a thin front end. A desktop UI will be a second thin front end on the same core.
- `unsafe` is allowed only inside one small memory-map wrapper, with a comment explaining why it is sound.
- Errors are typed (`thiserror`). No `unwrap()` or `expect()` outside tests.
- Everything must pass `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace`.
- Loudness numbers are verified against FFmpeg's `ebur128` filter on real files. Speed claims are backed by `criterion` benchmarks.
- Targets, in priority order: Windows x64, macOS Apple Silicon, macOS Intel, Linux GNU, Windows ARM.

## Working in this repo

- The folder is on a network share (`\\yggdrasil.local\...`). Git needs `safe.directory` set for it, and `cargo` should use a **local** target directory: set `CARGO_TARGET_DIR` to something like `C:\Local\rangeforge-target`. Details in [docs/06-development.md](docs/06-development.md).
- Two GitHub SSH identities exist on this machine. Check which one `origin` uses before pushing.
