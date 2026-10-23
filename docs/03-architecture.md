# 03 - Architecture

## The short version

Decode each file **once**, keep the audio in memory, and do everything else (measure, preview, split, gain, encode) from that memory. Cache the measurements so the work is never repeated.

```
 files ──► [ Read ] ──► [ Decode once ] ──► [ Measure per track ] ──► table / preview
                              │                     │
                              │                     └──► [ Cache on disk ]
                              ▼
                      (on Run) [ Gain + split ] ──► [ Encode ] ──► [ Tag + write ]
```

## Why it is fast

- A compressed file (FLAC, ALAC) costs time to **decode**, not to read. We decode once, where the old design decoded for every analysis and again for every export.
- A CUE album is one long buffer. Each track is just a start and end offset into it, so there is no re-decode per track.
- The preview is arithmetic. True peak after gain equals measured peak plus gain. Changing the target never re-measures.
- Several files are processed at once on a worker pool sized to the CPU (or the Settings limit).

## Where memory mapping is used (and where it is not)

- **Used for:** local, uncompressed sources (WAV, AIFF, DSD, large multichannel PCM). The OS gives zero-copy random access to any segment.
- **Not used for:** files on network shares. A dropped connection or a changed file can crash a process that has the file mapped, and it is slow. These are streamed or copied to a local temp file first.
- **Not a big win for:** FLAC and ALAC, where decoding dominates.
- All memory-map code lives in one small module, so the one place `unsafe` is needed is easy to audit.

## The pieces

| Piece | Job | Planned library |
| --- | --- | --- |
| Reader | Opens files, picks local-map vs stream | `memmap2`, `std::fs` |
| CUE | Parses cue sheets into tracks and offsets | own code (small, heavily tested) |
| Decoder | Turns audio files into PCM | Symphonia; FFmpeg for what it cannot read (DSD, MLP) |
| Meter | EBU R128 / BS.1770 loudness, true peak, range | `ebur128` crate |
| Planner | Pure maths: target, peak cap, gain per track | own code |
| Cache | Stores measurements keyed by file path, size, modified time | `serde` + a file in the app data folder |
| Encoder | Writes ALAC, FLAC, WAV, AAC | FFmpeg, fed raw PCM on stdin |
| Tagger | Reads/writes tags | `lofty` |
| Lookups | Last.fm, Discogs, Billboard chart file | `reqwest` |
| Settings | Presets and keys; keys stored encrypted | JSON file + OS keychain |
| Readiness check | Pure function: given the settings and the job state, reports whether everything is consistent (valid output format, keys present for enabled lookups, DSD fallback usable) and whether all asked-for work (maths, API calls, post-add processing) has finished. The UI uses it to allow the Auto Mode toggle, and the pipeline uses it to start an automatic run. | own code, heavily tested |

FFmpeg stays in the design for **encoding** (and awkward inputs) because ALAC and AAC encoders are mature there. Because it receives PCM we already decoded, nothing is decoded twice.

## Code layout

```
crates/
  rangeforge-core/   pure library: cue, planner, models, (later) meter, cache, pipeline
  rangeforge-cli/    command line front end
  (later) rangeforge-app/   desktop UI on the same core
```

- The core has no UI and does no printing. Front ends call it and render results.
- Anything that is just maths or parsing is a pure function with tests. I/O sits at the edges.

## Front ends

1. **CLI:** `analyze` (table output), `ui` (interactive terminal view of the mockup), `report --html`, `--json` on everything.
2. **Desktop app (Tauri 2):** renders the mockup layout in a web view, calling the same core.

## Correctness and quality

- Loudness numbers are compared against FFmpeg's `ebur128` filter on real files. We accept a small, stated tolerance.
- The CUE parser gets example tests and property tests (never panics, offsets always ordered).
- `criterion` benchmarks back any speed claim.
- CI runs format, lints (warnings are errors), tests and a build on every target platform.

## Known open questions

- Exact behaviour of Auto Mode.
- DR versus LRA (decision D-04).
- Which decoder handles DSD/MLP without FFmpeg, if any.
