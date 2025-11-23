# Onboarding — AI_CONTEXT.md (condensed)

## Summary
RangeForge is a single-user desktop app that consolidates 7+ audio tools into a preview-first volume-balancing workflow. Phase 1 (preview and memory-mapped analysis) is complete; Phase 2 (the "GO" commit workflow) is current priority. The project targets high-end local systems (16GB+ RAM), uses Electron + React, and integrates FFmpeg for broad codec support. Scope is explicitly limited: no plugins, no cloud, no user profiles, and a single expert UI.

## Key Constraints / "Do Not Suggest"
- **No plugin architecture**: single-user focus, no ecosystem.
- **No user profiles / account system**: avoid settings persistence complexity.
- **No cloud or 3rd-party app integrations** (beyond file-format compatibility and Last.fm metadata lookups).
- **No workflow scripting / advanced batch scripting**: batch handled via file list UI.

## Who / Typical Environment
- **Primary user**: project owner (power user, video-editing background)
- **Hardware**: 64GB recommended; 16GB minimum
- **Usage**: Album- and discography-scale processing; ALAC primary output

## Architecture & UX Highlights
- **Preview-first** using memory-mapped file analysis for instant parameter tweaks.
- **File list + table UI**, progressive analysis population, whole-window drop zone.
- **FFmpeg** used for analysis (LUFS/dBTP) and encoding.

## Actionable Setup / First Steps for a New Contributor
1. **Read core docs**: `AI_CONTEXT.md`, `PROJECT_PLAN.md`, `DOCUMENT_CONSISTENCY.md`, `FFMPEG_SETUP.md`.
2. **Install deps**: from repo root run `npm i` (PowerShell: `npm i`).
3. **Verify FFmpeg**: follow `FFMPEG_SETUP.md` and `scripts/download-ffmpeg.js` if necessary.
4. **Run app**: typical dev start (check `package.json` scripts) — e.g., `npm run dev`.
5. **Explore critical files**: `electron/main.ts`, `preload.ts`, `src/`, `workers/audioWorker.ts`, `services/*`.
6. **Confirm current Phase 2 blockers**: iSCSI migration for Electron binaries, dev env network-drive issues.

## Suggested First Technical Tasks (low-friction)
- Add a small UI stub for the **"GO"** button in the processing controls and wire it to the preview commit flow (start with a no-op that logs commit intent).
- Add a developer checklist to `PROJECT_PLAN.md` documenting how to reproduce Phase 1 → Phase 2 transition locally.
- Verify FFmpeg analysis outputs in `workers/audioWorker.ts` and ensure LUFS values surface to the table UI.

## Priority Roadmap (from AI_CONTEXT)
- **Phase 2 (Now)**: iSCSI migration, dev env fixes, implement "GO" commit workflow, multi-format outputs (ALAC primary).
- **Phase 3**: Multichannel workflow and FLAC output for Plex.
- **Phase 4**: Metadata/Last.fm enhancements and richer tagging UI.

## Communication / Review Guidelines
- When proposing features, ask: does this solve a real workflow pain point for a single power user? Validate against the 93% stereo / 7% multichannel split.
- Keep terminology consistent: prefer "volume balancing" over "loudness normalization".
- Record any scope changes in `AI_CONTEXT.md`, `PROJECT_PLAN.md`, and `DOCUMENT_CONSISTENCY.md` before merging.

## Next Steps I Can Do For You
- Create a minimal PR adding a non-functional "GO" button and a console log to show the commit trigger.
- Run the app locally and report runtime errors or missing dependencies.
- Draft a short developer checklist in `PROJECT_PLAN.md` for Phase 2 reproduction.

---
Generated from `AI_CONTEXT.md` (Nov 23, 2025).