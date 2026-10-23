# 03 - How it works

**In short:** the app reads each audio file once, takes notes on it, and does everything else from those notes. Because it never goes back to re-read the file, changing a setting feels instant. The notes are saved, so dropping the same album tomorrow costs nothing.

## What happens when you drop an album

1. **Look at the files.** The app spots CUE sheets and pairs them with their audio.
2. **Read once.** Each audio file is opened and turned into raw sound in memory, one time.
3. **Take notes.** For every song it records how loud it is, its highest peak, and how much the loudness moves. A CUE album is one long recording, so each song is just "from here to there" inside it. Nothing is re-read per song.
4. **Save the notes.** They go in a small file on disk, keyed to the audio file's size and date. Same file next time means the notes are reused.
5. **Show the plan.** The list fills in. The **Adjust** column is simple subtraction: your target minus the measured loudness. Change the target and it updates instantly.
6. **Optional lookups.** If enabled, tags come from online sources (see the FAQ).
7. **Run.** The app applies the volume change, cuts the album into songs, writes the tags, and saves the new files using your File Strategy. It uses the sound already in memory, so nothing is decoded a second time.

## Why this is faster than before

The old design started a separate FFmpeg program for each measurement and read the file again for every export. Reading the sound out of a compressed file (FLAC, ALAC) is the slow part, and it was being repeated. Now it happens once.

Several files are handled at the same time, up to the thread limit in Settings.

## About memory mapping

Memory mapping lets the computer treat a file on disk as if it were already in memory. It is a speed-up for **uncompressed** files (WAV, AIFF, DSD, big multichannel files), where the app can jump to any song instantly.

- **Used only for files on your own drive.** If a network drive disconnects or the file changes while it is mapped, the app can crash.
- **Not a big help for FLAC and ALAC.** They are compressed, so unpacking them, not reading them, is the cost.

How each kind of file is handled:

| File | Where it lives | What the app does |
| --- | --- | --- |
| Compressed (FLAC, ALAC) | Anywhere | Decodes straight from where it is, in one sequential read. Safe on a network drive, because only memory mapping is risky, and a copy would add a second pass. |
| Uncompressed (WAV, AIFF, DSD) | Your own drive | Memory-mapped directly. |
| Uncompressed (WAV, AIFF, DSD) | Network drive | Copied to a local temporary file first, then that copy is mapped. These files need jumping around between CUE tracks. |

Temporary copies are deleted when the file finishes or fails.
- All of this code sits in one small, separate piece so it is easy to check.

## The parts

| Part | What it does | Built with |
| --- | --- | --- |
| Reader | Opens files, decides local-map versus copy | `memmap2`, standard library |
| CUE reader | Turns a `.cue` sheet into songs and exact start and end points | our own code |
| Decoder | Turns audio files into raw sound | Symphonia; FFmpeg for what it cannot read (DSD, MLP) |
| Meter | Measures loudness, peak and range to the broadcast standard (EBU R128) | `ebur128` |
| Planner | Works out the volume change per song, with peak protection | our own code |
| Cache | Remembers measurements between sessions | `serde` plus a file in the app's data folder |
| Encoder | Writes ALAC, FLAC, WAV and AAC | FFmpeg, fed the sound we already decoded |
| Tagger | Reads and writes tags | `lofty` |
| Lookups | Last.fm and Discogs queries, plus the Billboard Hot 100 history: a ~40 MB JSON file kept as a local copy and re-downloaded only when a newer version exists. Radio tags are then a local lookup. | `reqwest` |
| Settings | Presets and keys. Keys are stored encrypted. | JSON file plus the system keychain |
| Readiness check | Answers "do the settings all agree, and has all the asked-for work finished?" The screen uses it to allow Auto Mode, and the app uses it to start an automatic run. | our own code, heavily tested |

FFmpeg stays for **writing** files because its ALAC and AAC encoders are mature. It is handed sound we already decoded, so there is no double work.

## How the code is organised

```
crates/
  rangeforge-core/   the pure logic: CUE, planning, (later) readiness, cache
  rangeforge-audio/  reads audio files once and measures loudness per track
  rangeforge-cli/    the command line front end
  (later) rangeforge-app/   the desktop window, using the same logic
```

- The core does no drawing and no printing. Front ends ask it questions and show the answers.
- Maths and parsing are written as plain functions with tests. Reading and writing files stays at the edges.

## The front ends

1. **Command line:** `analyze` prints the list as a table. `ui` is an interactive terminal version of the mockup. `report --html` makes a shareable page. `--json` on anything gives machine-readable output.
2. **Desktop app (Tauri 2):** the mockup layout in a web view, using the same core.

## How we check it is right

- Loudness numbers are compared with FFmpeg's own measurement on real albums from your collection, within a stated tolerance.
- The CUE reader gets example tests and random-input tests (it must never crash, and songs must always be in order).
- Any speed claim is backed by a benchmark.
- On every change, automated checks run format, lint (warnings count as failures) and tests on Windows, macOS, Linux.

## Platforms

First class: Windows 64-bit and macOS Apple Silicon. Also built and released: macOS Intel, Linux (GNU), Windows ARM. If work has to be staged, they are done in that order.

## Still undecided

- Which decoder handles DSD and MLP without FFmpeg, if any.
- Whether "Range" should be loudness range or true dynamic range (decision D-04).
- Whether the settings can be fully checked up front, or some problems only show during a run.
