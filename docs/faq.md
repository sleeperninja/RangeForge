# FAQ

## What is "volume balancing"?

Making tracks play at a consistent perceived loudness without distorting them. It is based on loudness standards (LUFS) and not on peak level.

## Why not just "normalize"?

Peak normalization makes the loudest instant hit a level, so two tracks can still sound very different in volume. Loudness-based balancing targets how loud the track *sounds*.

## What are LUFS, LRA, DR and true peak?

- **LUFS:** perceived loudness, as used by broadcasters and streaming services.
- **LRA (loudness range):** how much the loudness varies across a track.
- **DR (dynamic range):** a related but different measure, usually the gap between peaks and average level. See decision D-04 for how the app labels this.
- **True peak (dBTP):** the real highest level, including between samples. It matters for avoiding distortion on playback.

## Does the app change the dynamics of my music?

By design, no. It applies a single gain per track, and caps it so the true peak stays under your ceiling.

## Will there be beginner or intermediate modes?

Not now. There is one expert layout. The old wireframes are in git history (commit `01c53ea`).

## What does "Radio" mean?

A blue tag marking songs that charted on the Billboard Hot 100.

## Which online services does it use?

- **Discogs:** CUE and track data.
- **Last.fm:** album and track-level genre tags.
- **Billboard Hot 100 chart data:** the `all.json` file from `github.com/mhollingshead/billboard-hot-100`, fetched at app start (about 42 MB as of 2026-10-08), to decide the Radio tag.

Keys for Discogs and Last.fm are entered in Settings.

## Where did the old project go?

All of it, including the old plans, research and wireframes, is in git history at commit `01c53ea`.

## Where do I read about the design?

Start at [../CLAUDE.md](../CLAUDE.md).
