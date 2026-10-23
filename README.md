# RangeForge

Drop in albums and audio files. RangeForge measures their loudness once, shows you what it would change, then splits CUE albums, balances volume, tags, and writes clean files.

It is a personal tool, rebuilt in Rust. The design lives in [CLAUDE.md](CLAUDE.md) and [docs/](docs/).

```powershell
$env:CARGO_TARGET_DIR = 'C:\Local\rangeforge-target'   # build locally, not on a network share
cargo test --workspace
cargo run -p rangeforge-cli -- cue path\to\album.cue
```

Status: early. The CUE parser and gain planner exist; decoding, measurement and the UI are next. See [docs/05-roadmap.md](docs/05-roadmap.md).
