# 05 - Roadmap

One phase scheme. This is the precise list. [ROADMAP.md](../ROADMAP.md) in the repository root is the same plan in plain words; when something changes, change both.

**Now (0.6.1):** Phases 0 to 5 are built except benchmarks and `--json`/the HTML report. The channel mix (D-35: Original, Quad, Stereo, Mono, with a Downmix style) is built, measured on the mixed sound. The desktop window (`rangeforge-app`, Tauri) does the whole job: measure, plan gain, split CUE albums, tag and write ALAC, FLAC, WAV, AAC or DSD output, the three File Strategy choices, file name patterns, presets, hand editing of names and tags, album headings, Last.fm album genres and the Radio tag. File Strategy, Output Format, Volume and the name patterns are remembered between sessions. On Houses of the Holy the whole album is written in about 9 s, every file measures -22.00 LUFS, and a 0 dB export is sample-for-sample identical to the source. The installer workflow (D-23) is set up and has not been run on CI yet.

Next: click through the newest build (song tags page and Add a tag lists, settings restored on reopen, Album & track names, the rows' new names, Channels and Downmix), renaming from tags as a File Strategy choice, and remembering hand edits between sessions. Settled: D-09 (keep Symphonia), D-12 (gain only, the true-peak ceiling is the limit, no compression).

| Phase | Goal | Done when |
| --- | --- | --- |
| **0. Reset** | Docs, mockups and decisions reorganised; old JS code retired | CLAUDE.md and `docs/` in place |
| **1. Core slice** | Decode once, parse CUE, measure, plan gain, cache | Numbers match FFmpeg `ebur128` on real albums; CUE albums split into correct tracks |
| **2. CLI view** | `analyze` table, interactive `ui`, `--json`, HTML report | The Led Zeppelin + Killers example from the mockup renders and the target can be changed live |
| **3. Run** | Gain, split, encode (ALAC, FLAC, WAV, AAC, MP3, Opus), tags, file strategies | PowerShell script no longer needed |
| **4. Metadata** | File name patterns (done), Billboard "Radio" tag (done), who-a-song-is from tags and file names (done, including editing by hand), Last.fm album genres (done and tried against the live service: lookup, filtering, cache, written comma separated with Radio last), Last.fm song genres (the API returns none and the tags page has a bot check, so right-click opens the page in your browser and you add the tags with Add a tag, which takes comma, semicolon or line-break separated lists, D-31; MusicBrainz and Wikidata were tried and rejected, D-32; audio analysis is a parked idea), tag editing (done: × on chips, Add a tag, Album & track names, multi-select, album headings; kept for the session only) | Genre and Radio tags fill in for an album |
| **5. Presets and Settings** | Save/delete presets, encrypted keys, CPU limit, DSD fallback (built) | Matches the Settings mockup |
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

- **Remember hand edits** (names and tags) between sessions, keyed by file, so they survive closing the program.
- **Use song length in the track-list matcher** (needs the length in the measurements and the cache).
- **Rename from the tags** as a File Strategy choice (keep the original name, or use the single-file pattern), refusing or flagging names that are only a guess, and never letting two new files share a name.
- **Check the resampling and the 24 to 16 bit dither** against a null or loudness test.
- **Guess genre from the audio itself** (see D-32).
- **Right-click copy** of table text, and a highlight on an album heading when all its songs are picked.
- A note on a file's row when it cannot be mixed, a "will clip" mark for whole-number formats with Peak Protection off (D-41), and a Settings CPU limit (not built).
- **Opt-in limiter** for songs peak protection holds back (D-36); gain-only stays the default.
