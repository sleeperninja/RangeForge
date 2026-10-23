# 05 - Roadmap

One phase scheme. Status is updated here and nowhere else.

**Now:** Phase 0 done. Phase 1 mostly done. The CUE reader, gain planner, FLAC and ALAC decoding and per-track loudness measuring all work and are checked against a real EAC rip of Houses of the Holy and against FFmpeg. `rangeforge cue <file>` and `rangeforge measure <audio> [--cue <file>]` work. The analysis cache is done too: the same album takes 5.3 s the first time and 0.00 s after. Still to do in Phase 1: proper benchmarks. Phase 2 has started: `rangeforge ui` opens an interactive terminal view of the mockup, with live volume targets, background measuring, a log panel and peak-limit markers. Still to do in Phase 2: `--json` output and the HTML report.

| Phase | Goal | Done when |
| --- | --- | --- |
| **0. Reset** | Docs, mockups and decisions reorganised; old JS code retired | CLAUDE.md and `docs/` in place |
| **1. Core slice** | Decode once, parse CUE, measure, plan gain, cache | Numbers match FFmpeg `ebur128` on real albums; CUE albums split into correct tracks |
| **2. CLI view** | `analyze` table, interactive `ui`, `--json`, HTML report | The Led Zeppelin + Killers example from the mockup renders and the target can be changed live |
| **3. Run** | Gain, split, encode (ALAC, FLAC, WAV, AAC), tags, file strategies | PowerShell script no longer needed |
| **4. Metadata** | Last.fm, Discogs, Billboard "Radio" tag, tag editing | Genre and Radio tags fill in for an album |
| **5. Presets and Settings** | Save/delete presets, encrypted keys, CPU limit, DSD fallback | Matches the Settings mockup |
| **6. Multichannel** | 4-8 channel loudness, FLAC for Plex, DSD/MLP input | The 206 GB collection processes |
| **7. Desktop app** | Tauri 2 UI matching the mockups | Mockup layout works end to end |
| **8. Releases** | Five-platform CI builds and bundled FFmpeg | Tagged release produces all artifacts |

## Phase 1 detail

1. Workspace, lints, CI. *(done locally; CI itself not yet run on GitHub)*
2. CUE parser and gain planner with tests. *(done)*
3. Decoder and meter integration. *(done: `rangeforge-audio`; matches FFmpeg on loudness and peak, see decisions D-03, D-04, D-09)*
4. Analysis cache. *(done: content fingerprint, per-CUE-layout results, see decision D-11)*
5. Benchmarks. *(first manual comparison done; proper `criterion` benchmarks still to do)*

## Parked ideas

See [04-decisions.md](04-decisions.md) for anything marked Parked.
