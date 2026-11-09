# RangeForge

**Drop in albums and audio files. RangeForge measures their loudness once, shows you what it would change, then splits CUE albums, evens out the volume, tags, names and writes clean files.**

It is free to use. If it saves you time, you can support it on Ko-fi: [ko-fi.com/sleeperninja42](https://ko-fi.com/sleeperninja42).

- **Measure once.** Loudness, true peak and range, cached so the same album is instant the second time.
- **One gain per song.** Volume only: no compression, no limiting, never past your peak ceiling.
- **Split CUE albums** and write ALAC, FLAC, WAV (including 32-bit float), AAC, MP3 or Opus.
- **Convert when you ask:** sample rate, bit depth, and surround to quad, stereo or mono.
- **Fix names and tags** in one dialog, with genres from Last.fm (using your own free key) and a Radio tag from the Billboard Hot 100.
- **Safe by design.** Originals are untouched until every new file has been written.

## Using it

- [MANUAL.md](MANUAL.md): how to use the program and how it decides things.
- [FAQ.md](FAQ.md): quick answers.
- [ROADMAP.md](ROADMAP.md): what works, what's next.
- [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md): FFmpeg, LAME, Opus and the other libraries inside.

RangeForge is version 0.6.5 and pre-1.0. Windows is the main target. Installers are built by the project's GitHub workflow and will appear on the [releases page](https://github.com/sleeperninja/RangeForge/releases) when the first release is published.

## Building it yourself

```powershell
$env:CARGO_TARGET_DIR = 'C:\Local\rangeforge-target'   # build locally, not on a network share
cargo test --workspace
cargo run -p rangeforge-cli -- cue path\to\album.cue
```

The desktop window needs FFmpeg next to it; `scripts/fetch-ffmpeg.sh` gets the right (LGPL) build. Details are in [docs/04-development.md](docs/04-development.md).

## For people (and AIs) working on the code

Start at [CLAUDE.md](CLAUDE.md). The technical documents are in [docs/](docs/): the screen, the architecture, the decisions and why, and development.
