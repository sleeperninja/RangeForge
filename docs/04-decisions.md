# 04 - Decisions

Each decision has a status, the reason, and when to revisit. Many of the *Rejected* items came from early agent pushback in 2025, so they are here to be questioned, not obeyed.

Statuses: **Accepted**, **Rejected**, **Reopened** (under discussion), **Parked** (later), **Superseded**.

## Technical

| ID | Decision | Status | Why / when to revisit |
| --- | --- | --- | --- |
| D-01 | Rebuild in Rust, replacing Electron + React + TypeScript | Accepted | Speed and a safe core. The old stack had unmaintained parts (Create React App, mmap-io, fluent-ffmpeg) and constant native-rebuild trouble. |
| D-02 | Memory-map **local** files only | Accepted | Mapping network files can crash on disconnect and is slow. Compressed files are decoded straight from the network in one pass. Uncompressed files on a network drive are copied to local temp first, then mapped. See the table in 03-architecture. |
| D-03 | Decode once, work from memory, cache measurements | Accepted, with a caveat | The old "mmap for preview" never fed FFmpeg, so it did not help. First measurement (Houses of the Holy, 41 min FLAC on the NAS, 8 tracks): one pass with 8 meters took 5.7 s, against 9.2 s for 8 separate FFmpeg runs and 7.8 s for one FFmpeg pass. That is about 1.6x faster, not the dramatic win first expected, because FFmpeg also seeks and does not re-decode the whole file per track. The bigger saving should come in the full run, where analysis and export share one decode. Reading the file alone takes 2.4 s of the 5.7 s. |
| D-04 | The "Range" column shows LRA (loudness range) | Accepted | Decided 2026-10-23. The mockup header says "(DR)" and its whole-number values resemble TT DR Meter scores, which was only an example of the layout. The old code already fed LRA into that column. The column is labelled **Range (LRA)** and shows LU. True DR (the TT DR Meter style score) is out of scope and can be added later if wanted. Open, low priority: our LRA differs from FFmpeg's by up to 0.4 LU on some tracks (No Quarter: 20.10 against 19.7) while loudness and true peak match. Gain planning does not use LRA, so this only affects the display. If it ever matters, settle it with the EBU Tech 3341/3342 test signals, which come with specified expected values, instead of comparing tools. |
| D-05 | Gain method: linear gain capped by peak protection | Accepted | Keeps dynamics (the "lossless sweetener" idea). The old plan used `loudnorm`, which can change dynamics. Revisit if clip-limiting is wanted. |
| D-06 | Keep FFmpeg for encoding and awkward inputs | Accepted | Mature ALAC/AAC encoders. Fed PCM on stdin, so no double decode. |
| D-07 | UI: terminal view first, Tauri desktop app later | Accepted | Data-heavy flow suits a table. Both sit on the same core. |
| D-09 | Decoder: Symphonia, with FFmpeg only for formats it cannot read | Accepted | Measured with files on a local disk and nothing else running, for the 41-minute album: Symphonia decodes FLAC in 1.4 s and ALAC in 4.3 s, against FFmpeg's 0.5 s and 1.0 s. FFmpeg is faster, but decoding is never the slow part: just reading the file off the NAS takes about 3 s at 80 MB/s, and a whole export takes about 9 s. Switching would save 1 to 3 s per album. Symphonia matched FFmpeg's loudness and peak on every track. Revisit only if profiling shows decoding on the critical path (for example big multichannel or DSD files, Phase 6). The earlier 21.8 s ALAC figure was a cold first read of that file from the NAS (18.6 s cold, 4.2 s on the next read), not the decoder. |
| D-10 | Overlap reading and decoding | Parked | Reading and decoding currently take turns. A read-ahead thread could hide most of the network read on a NAS, and cold first reads from the NAS vary a lot (the album ALAC took 18.6 s cold and 4.2 s warm). Larger read blocks made no difference in a test. Do it once the whole pipeline is measured. |
| D-11 | The cache recognises a file by content, not name or date | Accepted | Key is the file size plus a hash of three 64 KiB slices. It survives renames, copies and moves, which the app's own File Strategy and the NAS both cause. Cost: three small reads per file. Known trade-off: editing the tags changes the first slice, so a re-tagged file is measured again. Results are stored per CUE layout (in CD frames, so independent of sample rate), and a version number discards old entries when the measuring code changes. Measured on Houses of the Holy: 5.3 s cold, 0.00 s cached. |
| D-12 | Gain only. The true-peak ceiling is the hard limit on how far the volume knob can turn. | Accepted | Decided 2026-10-24. The tool never compresses or limits the waveform, so no peak data is ever lost. The ceiling decides how much gain a song can take, and a song that runs out of headroom stays below the target and is marked `*`. Seen on Houses of the Holy at -16 LUFS with a -0.6 dBTP ceiling: The Rain Song wants +4.66 dB but can only take +1.31 dB. Someone who asks for -10 LUFS with a 0 dBTP ceiling will get a wide spread of volumes, and that is the setting working as intended, not a flaw. Any waveform compression is an optional extra for much later, after everything else works. For that day: a light limiter measured on this album reached the target within 0.1 LU and moved the loudness range by under 0.2 LU, but FFmpeg's `alimiter` works on sample peaks, so it needed to be set about 1.4 dB below the true-peak ceiling to stay under it. A listening comparison is in `testdata/ab-limiter/`. |
| D-13 | Nothing original is touched until everything has been written | Accepted | Every song is encoded to a temporary file, and files are only moved into place once the whole source has decoded and every encode has succeeded. A failure deletes the temporary files and leaves originals as they were. Tested, including a failed placement. |
| D-14 | File Strategy meanings | Accepted | Serialize always adds the suffix. Rename-old renames an existing file at the same path and never overwrites an earlier backup. Replace overwrites only a file at exactly the same path and never deletes a differently named source. Save-to writes elsewhere and serializes clashes. Replace needs an explicit `--yes` on the command line. For a CUE album, which has no same-named output to replace, Replace writes the songs next to the image and keeps the image (decided 2026-10-24). Replace is expected to become the default strategy once it has proven itself, and until then the safer choices stay the default. Planned: after a run that has been verified, offer to move the CUE image (or a replaced source) to the Recycle Bin or Trash, always with explicit confirmation. Rule for testing it: the real action sits behind a small interface with a fake for tests, and tests only ever use generated files or copies in temporary folders, never `testdata/` originals or the real Recycle Bin. |
| D-15 | Dither only when needed | Accepted | A gain on 16-bit audio, or a drop from 24 to 16 bits, uses triangular dither. Audio at 0 dB is never dithered, so it stays bit-exact, and 24-bit output is not dithered. |
| D-16 | FFmpeg is found on the path or via `RANGEFORGE_FFMPEG` | Accepted for now | Bundling one per platform is Phase 8. |
| D-08 | Platforms: Win x64, macOS ARM first; macOS Intel, Linux GNU, Win ARM follow | Accepted | One CI matrix builds all. Only the first two are tested on real hardware. |

## Scope

| ID | Decision | Status | Why / when to revisit |
| --- | --- | --- | --- |
| S-01 | Personal tool first | Accepted | Simplest honest scope. The old market research is in git history (`01c53ea`). |
| S-02 | Plugin architecture | Rejected | Single user, big maintenance cost. |
| S-03 | Workflow scripting | Rejected | Would recreate the PowerShell complexity being escaped. |
| S-04 | Cloud processing | Rejected | Local only. Metadata lookups are the only network use. |
| S-05 | Multi-user, accounts, teams | Rejected | One user. |
| S-06 | Beginner and intermediate UI modes | Parked | One expert layout. Old wireframes are in git history (`01c53ea`). |
| S-07 | User-defined presets | **Accepted (was Rejected)** | The new mockups include Save/Delete Preset. Replaces the old "no profiles" rule. |
| S-08 | Persistent settings and API keys | **Accepted (was Rejected)** | The mockup Settings dialog needs it. Keys are stored encrypted. |
| S-09 | App integrations (iTunes, Plex) | Rejected | Compatible files are enough. |
| S-10 | Online lookups | Accepted, limited | Last.fm (tags), Discogs (CUE/track data), Billboard Hot 100 history file (Radio tag; local copy, refreshed only when a newer one exists). More only with a clear need. |
| S-11 | Quality-assurance reports, batch export monitoring | Parked | Table plus log already cover it. A simple HTML report is planned instead. |
| S-12 | Dolby Digital Plus output | Parked | Waiting for encoder support. |
| S-13 | The name "RangeForge" | Parked | Previously worried it is too close to another audio product's name. Decide before any public release. |

## Naming and wording

- The button says **Run**, not "GO" or "Make It So". Movie references explain ideas and are not UI text.
- The UI uses technical terms (LUFS, dBTP) because it is an expert layout. The old "never say LUFS" rule applied to a market-facing design that is on hold.

## Changing a decision

Change the status here, add a one-line reason, and update the affected doc in the same change.
