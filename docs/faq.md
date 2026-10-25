# FAQ

## What is "volume balancing"?

Making tracks play at a consistent perceived loudness without distorting them. It is based on loudness standards (LUFS) and not on peak level.

## Why not just "normalize"?

Peak normalization makes the loudest instant hit a level, so two tracks can still sound very different in volume. Loudness-based balancing targets how loud the track *sounds*.

## What are LUFS, LRA, DR and true peak?

- **LUFS:** perceived loudness, as used by broadcasters and streaming services.
- **LRA (loudness range):** how much the loudness varies across a track.
- **DR (dynamic range):** a related but different measure, popularised by the TT DR Meter and the online dynamic-range database. RangeForge does not show DR. Its Range column is LRA, as decision D-04 explains.
- **True peak (dBTP):** the real highest level, including between samples. It matters for avoiding distortion on playback.

## Does the app change the dynamics of my music?

By design, no. It applies a single gain per track, and caps it so the true peak stays under your ceiling.

## Will there be beginner or intermediate modes?

Not now. There is one expert layout. The old wireframes are in git history (commit `01c53ea`).

## What does "Radio" mean?

A blue tag marking songs that charted on the Billboard Hot 100.

## Which online services does it use?

- **Last.fm:** album and track-level genre tags.
- **Billboard Hot 100 chart data:** decides the **Radio** tag. A public GitHub project keeps the whole Hot 100 history in one file, `all.json`, about 42 MB as of 2026-10-08.

- The file: <https://github.com/mhollingshead/billboard-hot-100/blob/main/all.json>
- The app downloads the raw file, which is the same path on GitHub's raw host: `https://raw.githubusercontent.com/mhollingshead/billboard-hot-100/main/all.json`

## How does the Radio tag stay current?

The app keeps a **local copy** of `all.json` and does not download it on every launch. It asks GitHub whether a newer version exists, and only downloads the file when it does. After that, every Radio lookup is a local check. A song gets the Radio tag when it appears anywhere in the chart history, matched on artist and title. If the download fails, the app uses the copy it already has.

The Last.fm key is entered in Settings. Discogs is not used: finding the right release among many near-identical ones is a hard problem, and nothing planned needs it (decision S-10).

## Where did the old project go?

All of it, including the old plans, research and wireframes, is in git history at commit `01c53ea`.

## Where do I read about the design?

Start at [../CLAUDE.md](../CLAUDE.md).
