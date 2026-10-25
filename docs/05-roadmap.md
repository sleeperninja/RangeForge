# 05 - Roadmap

One phase scheme. Status is updated here and nowhere else.

**Now:** Phases 0 and 1 are done except benchmarks. Phases 2, 3 and 5 are mostly done through the terminal view (`rangeforge ui`) and `rangeforge run`: measure, plan gain, split CUE albums, tag and write ALAC, FLAC, WAV or AAC, all four File Strategy choices, presets saved between sessions. On Houses of the Holy the whole album is written in about 9 s, every file measures -22.00 LUFS, and a 0 dB export is sample-for-sample identical to the source.

Phase 7 has started: `rangeforge-app` is a Tauri window (plain HTML, no Node needed to build) with the mockup's layout, drag and drop, file pickers, the settings panels and a working Run. A Release workflow is set up to build the MSI and DMG installers with FFmpeg inside (D-23); it has not been run on CI yet.

Still to do: benchmarks, `--json` and the HTML report, Sample Rate and Channels controls, multichannel and DSD input, the Settings window, the Last.fm and Radio tags, and signing the installers. Settled: D-09 (keep Symphonia), D-12 (gain only, the true-peak ceiling is the limit, no compression).

| Phase | Goal | Done when |
| --- | --- | --- |
| **0. Reset** | Docs, mockups and decisions reorganised; old JS code retired | CLAUDE.md and `docs/` in place |
| **1. Core slice** | Decode once, parse CUE, measure, plan gain, cache | Numbers match FFmpeg `ebur128` on real albums; CUE albums split into correct tracks |
| **2. CLI view** | `analyze` table, interactive `ui`, `--json`, HTML report | The Led Zeppelin + Killers example from the mockup renders and the target can be changed live |
| **3. Run** | Gain, split, encode (ALAC, FLAC, WAV, AAC), tags, file strategies | PowerShell script no longer needed |
| **4. Metadata** | File name patterns (done), Billboard "Radio" tag (done), who-a-song-is from tags and file names (done; editing by hand next), Last.fm genres (key storage done, lookups next), tag editing | Genre and Radio tags fill in for an album |
| **5. Presets and Settings** | Save/delete presets, encrypted keys, CPU limit, DSD fallback | Matches the Settings mockup |
| **6. Multichannel** | 4-8 channel loudness, FLAC for Plex, DSD/MLP input | The 206 GB collection processes |
| **7. Desktop app** | Tauri 2 UI matching the mockups | Mockup layout works end to end |
| **8. Releases** | MSI and DMG installers with FFmpeg inside, built by CI (set up, not yet run); signing later | Tagged release produces all artifacts |

## Phase 1 detail

1. Workspace, lints, CI. *(done locally; CI itself not yet run on GitHub)*
2. CUE parser and gain planner with tests. *(done)*
3. Decoder and meter integration. *(done: `rangeforge-audio`; matches FFmpeg on loudness and peak, see decisions D-03, D-04, D-09)*
4. Analysis cache. *(done: content fingerprint, per-CUE-layout results, see decision D-11)*
5. Benchmarks. *(first manual comparison done; proper `criterion` benchmarks still to do)*

## Parked ideas

See [04-decisions.md](04-decisions.md) for anything marked Parked.
