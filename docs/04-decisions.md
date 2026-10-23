# 04 - Decisions

Each decision has a status, the reason, and when to revisit. Many of the *Rejected* items came from early agent pushback in 2025, so they are here to be questioned, not obeyed.

Statuses: **Accepted**, **Rejected**, **Reopened** (under discussion), **Parked** (later), **Superseded**.

## Technical

| ID | Decision | Status | Why / when to revisit |
| --- | --- | --- | --- |
| D-01 | Rebuild in Rust, replacing Electron + React + TypeScript | Accepted | Speed and a safe core. The old stack had unmaintained parts (Create React App, mmap-io, fluent-ffmpeg) and constant native-rebuild trouble. |
| D-02 | Memory-map **local** files only | Accepted | Mapping network files can crash on disconnect and is slow. Stream or copy those. |
| D-03 | Decode once, work from memory, cache measurements | Accepted | This is where the speed comes from. The old "mmap for preview" never fed FFmpeg, so it did not help. |
| D-04 | The "Range" column | Reopened | The old code put LRA in a column labelled DR. They are different measures. Plan: label it LRA now, add true DR later if wanted. |
| D-05 | Gain method: linear gain capped by peak protection | Accepted | Keeps dynamics (the "lossless sweetener" idea). The old plan used `loudnorm`, which can change dynamics. Revisit if clip-limiting is wanted. |
| D-06 | Keep FFmpeg for encoding and awkward inputs | Accepted | Mature ALAC/AAC encoders. Fed PCM on stdin, so no double decode. |
| D-07 | UI: terminal view first, Tauri desktop app later | Accepted | Data-heavy flow suits a table. Both sit on the same core. |
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
| S-10 | Online lookups | Accepted, limited | Last.fm (tags), Discogs (CUE/track data), Billboard Hot 100 file (Radio tag). More only with a clear need. |
| S-11 | Quality-assurance reports, batch export monitoring | Parked | Table plus log already cover it. A simple HTML report is planned instead. |
| S-12 | Dolby Digital Plus output | Parked | Waiting for encoder support. |
| S-13 | The name "RangeForge" | Parked | Previously worried it is too close to another audio product's name. Decide before any public release. |

## Naming and wording

- The button says **Run**, not "GO" or "Make It So". Movie references explain ideas and are not UI text.
- The UI uses technical terms (LUFS, dBTP) because it is an expert layout. The old "never say LUFS" rule applied to a market-facing design that is on hold.

## Changing a decision

Change the status here, add a one-line reason, and update the affected doc in the same change.
