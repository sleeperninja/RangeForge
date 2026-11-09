# 03 - How it works

**In short:** the app reads each audio file once, takes notes on it, and does everything else from those notes. Because it never goes back to re-read the file, changing a setting feels instant. The notes are saved, so dropping the same album tomorrow costs nothing.

## What happens when you drop an album

1. **Look at the files.** The app spots CUE sheets and pairs them with their audio.
2. **Read once.** Each audio file is opened and turned into raw sound in memory, one time.
3. **Take notes.** For every song it records how loud it is, its highest peak, and how much the loudness moves. A CUE album is one long recording, so each song is just "from here to there" inside it. Nothing is re-read per song.
4. **Save the notes.** They go in a small file on disk. The app recognises an audio file by its **content**, not its name or date: its size plus a fingerprint of three small slices (start, middle, end). So a renamed file, a copy in another folder, or the same album moved to the NAS all reuse the notes. If the file really changes, or the measuring code is improved, the old notes are ignored and replaced. A cache problem never stops a measurement.
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

## Writing files safely

When you run, the app never changes an original until every new file has been written successfully.

1. Each song is encoded to a hidden temporary file next to where it will end up.
2. Only after the whole source has been read and every song has encoded without error are the temporary files moved into place, following your File Strategy.
3. If anything fails along the way, the temporary files are deleted and your originals are exactly as they were.

The File Strategy is three separate choices. First, **where** the files go: next to the original, in a subfolder of the original's folder, or all in one folder. Second, **what to do if a file is already at the new file's path**, which is the only time a name is ever changed:

| Choice | What happens | What it never does |
| --- | --- | --- |
| Add a number | The new file gets a serial suffix such as `_00`, counting up until the name is free. | Touch the file that is already there. |
| Rename the existing file | The file already there is renamed with the suffix (such as `_old`), and the new file takes its name. | Overwrite an earlier `_old` file; it stops with an error instead. |
| Overwrite | The file at exactly the new file's path is replaced. | Delete a source with a different name, such as the FLAC when making ALAC. |

Third, an optional **delete the original**, off by default. It happens last, after every new file has been placed, and only when every song from that source was written (none left out) and the source is not itself one of the files just written. A failed run deletes nothing. Overwriting and deleting both need an explicit confirmation (`--yes` on the command line).

**Volume and quality.** Volume change is a single gain per song, applied to the sound in memory before it is encoded, so the dynamics are untouched. A song exported at 0 dB is sample for sample identical to its source. When a gain forces 16-bit audio between its steps, a tiny amount of triangular dither is used instead of plain rounding, which trades a faint distortion for a steady noise about 96 dB down. Audio kept at 0 dB is never dithered, so it stays exact. Audio written at 24 bits needs no dither.

**Conversions and limiting.** Resampling, a drop in bit depth and mixing channels are format conversions, not volume work, so they are allowed alongside the gain-only rule (D-35). The order is convert, measure the converted sound, apply the one gain, encode: a channel mix changes the loudness and raises the peaks, so the gain and the true-peak ceiling are worked out on the mixed signal. The mix is one function (`rangeforge-core::mix`, giving a coefficient matrix for a source layout) applied to each decoded block in `rangeforge-audio` (`for_each_block_mixed`), which both the meter and the export read through; the measurement is cached under a key that includes the mix, and a `Job` carries the mix so the table and the written files cannot disagree. Gain-only stays the default. A limiter would only ever be an opt-in that touches the songs peak protection would otherwise hold below the target, and it would report each song it touched (D-36, not built).

**Tags.** CUE albums get title, artist, album, album artist, track number and total, genre and date from the CUE sheet. A single file keeps the tags it already has, with anything we set on top.

## The parts

| Part | What it does | Built with |
| --- | --- | --- |
| Reader | Opens files, decides local-map versus copy | `memmap2`, standard library |
| CUE reader | Turns a `.cue` sheet into songs and exact start and end points | our own code |
| Decoder | Turns audio files into raw sound | Symphonia; FFmpeg for what it cannot read (DSD, MLP) |
| Meter | Measures loudness, peak and range to the broadcast standard (EBU R128) | `ebur128` |
| Planner | Works out the volume change per song, with peak protection | our own code |
| Cache | Remembers measurements between sessions, per way of splitting a file | `serde_json`, `blake3`; one small JSON file per audio file in the app's cache folder |
| Encoder | Writes ALAC, FLAC, WAV and AAC, one FFmpeg process per song, fed the already-decoded sound through a pipe | FFmpeg |
| Tagger | Writes tags as the file is encoded. Reading and editing tags in the app comes later. | FFmpeg now; `lofty` later |
| Lookups | Last.fm: album tags from its API. Song tags are added by the user from the song's public tags page, which the program only opens in the user's browser and never fetches (D-31; the API returns none for songs, and the page has a bot check). Plus the Billboard Hot 100 history: a ~40 MB JSON file kept as a local copy and re-downloaded only when a newer version exists. Radio tags are then a local lookup. | `reqwest` |
| Settings | Presets and keys. Keys are stored encrypted. | JSON file plus the system keychain |
| Readiness check | Answers "do the settings all agree, and has all the asked-for work finished?" The screen uses it to allow Auto Mode, and the app uses it to start an automatic run. | our own code, heavily tested |

FFmpeg stays for **writing** files because its ALAC and AAC encoders are mature. It is handed sound we already decoded, so there is no double work.

## How the code is organised

```
crates/
  rangeforge-core/   the pure logic: CUE, planning, (later) readiness, cache
  rangeforge-audio/  reads audio files once and measures loudness per track
  rangeforge-export/ writes the finished files: gain, encode through FFmpeg, place by File Strategy
  rangeforge-lookup/ outside data: the Billboard Hot 100 index and how songs are matched to it
  rangeforge-session/ the rules the screens share: files, measuring, settings, presets, running
  rangeforge-cli/    the command line and terminal front end (a developer tool, not shipped)
  rangeforge-app/    the desktop window (Tauri), using the same logic
```

- The core does no drawing and no printing. Front ends ask it questions and show the answers.
- Maths and parsing are written as plain functions with tests. Reading and writing files stays at the edges.

## The front ends

1. **Command line:** `measure` prints loudness per song. `ui` is an interactive terminal version of the mockup. `run` writes the finished files, with `--dry-run` to preview. Planned: `report --html` for a shareable page, and `--json` on anything for machine-readable output.
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
- Whether the settings can be fully checked up front, or some problems only show during a run.
