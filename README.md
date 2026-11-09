# RangeForge

Drop in albums and audio files. RangeForge measures their loudness once, shows you what it would change, then splits CUE albums, balances volume, tags, and writes clean files.

It is a personal tool, rebuilt in Rust. The design lives in [CLAUDE.md](CLAUDE.md) and [docs/](docs/).

```powershell
$env:CARGO_TARGET_DIR = 'C:\Local\rangeforge-target'   # build locally, not on a network share
cargo test --workspace
cargo run -p rangeforge-cli -- cue path\to\album.cue
```

Status: working and pre-1.0 (0.3.x). The desktop app measures, plans gain, splits CUE albums and writes tagged files; a personal tool first. See [ROADMAP.md](ROADMAP.md) for the plain-words plan and [docs/05-roadmap.md](docs/05-roadmap.md) for the detail.
