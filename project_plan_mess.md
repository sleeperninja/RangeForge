Application name RangeForge
API key 869104bf8f37f005a8c38cd85d1cdd58
Shared secret df53c7ec1eef58d3f30ca0bb03add1e8
Registered to superjudge42

## Technologies

- Stack
  - ECMAScript
  - Node
  - React
  - Electron

### Stack features

- Multithreading (mandatory)
- Memory-Mapped Files (nice to have)
- Targets - Win64 (mandatory), MacARM, GNU AMD64 (nice to have)
- Internally packed Frameworks/SDKs only (no pre-install installs)

## App Operation

1. Options on the main view of the app when opened
   1. Title bar "RangeForge"
   2. Drop zone / list of input files
      1. Input files in table format
         Table Headers: file name,
   3. Target file dropdowns (bottom row, if possible)
      1. Auto Mode {switch or check--bool}
      2. Target volume level
      3. Target codecs (ORIGINAL, aac, ac3, alac, eac3, flac, libmp3lame, libopus, pcm\_(s16le, s24le, s32le))
      4. Target samplerate (ORIGINAL, 32kHz, 44.1kHz, 48kHz, 88kHz, 96kHz, 192kHz, CUSTOM)
      5. Target bitrate (ORIGINAL, 4bit, 8bit, 16bit, 32bit)
2. On drag & drop, would:
   1. identify if a file is an individual audio track or an album file with an associated cue in the same folder
      1. cue file may not always have the same name as the album file, but it should be required that the name of the album file is in the FILE field of the cue file
   2. split an album file/cue into separate tracks (only if necessary), and assigning metadata from the CUE file
   3. used ffmpeg to read the dynamic range of the split files, as well as any additional audio files of any type that don't need splitting
3. Then would allow you to either manually or automatically export audio files that:
   1. have adhered to the desired loudness/volume level (without normalization), while preserving the waveform without hard clipping
   2. have been transitioned to selected audio codec (FLAC, AAC, ALAC, MP3, Opus), maintaining the existing metadata
4. Optional features:
   Last.fm (would need an API key and secret to enable, and the page to get an API key should be linked in the app. API is free, but limited to 1 call per second)
   Desktop App guide: https://www.last.fm/api/desktopauth
   1. Set/add genre tags based on artist tags {check or switch--bool}
   2. Set/add genre tags based on album tags {check or switch--bool}
   3. Set/add genre tags based on song tags {check or switch--bool}
   4. Set/add a custom genre tag (such as "popular" or "radio") when a song shows up in the top {slider, dial, or input--integer} songs of artist's "all time" Top Tracks, AND/OR
   5. Set/add a custom genre tag (such as "popular" or "radio") when a song has {slider, dial, or input--integer} or more listens on the artist's "all time" Top Tracks.

Recommended Tech Stack
Component Recommendation Rationale
Backend Runtime Node.js with Worker Threads Supports multithreading and fits well with your knowledge of Node.js.
Frontend Framework React Utilize the latest features like hooks for managing state and effects.
Desktop Framework Electron Works across platforms (Windows, macOS, Linux) and matches your existing knowledge.
Audio Processing FFmpeg Powerful tool for audio manipulation, suitable for splitting and analyzing audio.
File I/O Operations Node.js 'fs' with memory-mapped files To implement memory-mapped files, you could use native modules or libraries like mmap.
Audio Libraries Web Audio API or Wavesurfer.js Useful for visualizations and audio playback; integrate with React.
Features Breakdown
Multithreading

    Implementation: Utilize Node.js Worker Threads for handling heavy computations (like audio processing) to keep the UI responsive in your Electron app.

Memory-Mapped Files

    Approach: You can implement memory-mapped file functionality using native libraries such as mmap.js or a Node.js addon that provides mmap support. This is particularly useful for handling large audio files without loading them entirely into memory.

Cross-Platform Compatibility

    Targets: Electron provides a unified framework that allows you to target Win64, MacARM, and GNU AMD64 without drastic changes to the codebase.

Application Behavior

    Drag & Drop Functionality:
        Utilize React DnD for drag-and-drop file handling.
        On drop, trigger a worker thread to handle the splitting of FLAC/CUE files. Use FFmpeg to manage audio processing.
    Audio Export Options:
        After audio analysis (to check for loudness and dynamic range), allow users to select their preferred codec (e.g., FLAC, MP3).
        Use FFmpeg in a worker thread to handle conversions while ensuring existing metadata is preserved.

Additional Considerations

    Performance: Test and optimize the application for performance, especially during the audio manipulation processes. Use tools to profile memory usage and CPU performance.
    User Experience: Prioritize a clean user interface and experience in the Electron app. Consider using libraries like Material-UI or Ant Design to enhance the design.

Conclusion

This stack is well-aligned with your expertise while meeting the specific needs of your project. Node.js provides a robust backend for audio processing, React offers a modern, component-based UI, and Electron allows seamless cross-platform desktop applications. Implementing multithreading with worker threads will help maintain responsiveness, crucial for handling large audio files dynamically. Consider exploring opt-in libraries and tools that complement your choice for a polished application.

## What would you think this app should be called?

RangeSmith
Concept 1: An anvil + waveform — stylized anvil silhouette with a single, clean waveform carved across the top; colors: gunmetal gray + teal accent for the waveform. Use for pro/industrial feel.
Concept 2: Hammer + dynamic bars — small hammer striking a stack of vertical bars (different heights) that form a smooth curve; colors: deep blue + bright orange accent to show action.
Concept 3: Precision ruler + waveform — a minimal ruler overlaying a gentle waveform, implies measurement and craft; colors: slate + lime green accent for precision.

LevelSmith
Concept 1: Smith's hammer + level meter — hammer crossing a horizontal level meter (needle centered), rounded corners; colors: charcoal + mint to signal control.
Concept 2: Shield + volume fader — small shield shape containing a centered fader knob set to target position — communicates preservation and protection; colors: navy + soft gold.
Concept 3: Gear + waveform slice — gear with a single waveform slice inside, suggests crafted leveling; colors: steel blue + cyan.

TrackSmith
Concept 1: Vinyl record + small hammer — record with subtle grooves and a tiny hammer overlay in corner; colors: black/ivory + copper accent for craft feel.
Concept 2: Stacked tracks icon + wrench — three horizontal stacked rectangles (tracks) with a small wrench on the corner; colors: indigo + lime.
Concept 3: Notched timeline + anvil — compact timeline with cut markers and anvil icon, indicating splitting + craft; colors: slate + orange.

RangeForge
Concept 1: Forge flame + waveform — stylized flame with negative-space waveform inside; colors: ember orange + dark slate background for warmth/energy.
Concept 2: Hammer striking waveform arc — hammer above a glowing curved waveform (like forging metal into a curve); colors: obsidian + molten amber.
Concept 3: Molten metal droplet + dynamic bars — droplet shape composed of vertical DR bars transitioning from wide to narrow; colors: red-gradient + charcoal.

LevelForge
Concept 1: Anvil + level slider — anvil silhouette with a horizontal slider (knob centered) embedded; colors: graphite + teal knob.
Concept 2: Flame-forge motif + meter needle — subtle forge element with an adjacent analogue meter showing a target region; colors: dark red + silver.
Concept 3: Metal plate + balanced bars — rectangular metal plate icon with five evenly spaced bars (balanced heights) suggesting stable levels; colors: pewter + mint.

TrackForge
Concept 1: Multi-track lines + hammer head — three parallel horizontal lines (tracks) with hammer head striking left end; colors: navy + orange.
Concept 2: Forge stamp on a track — single long track with a circular stamp/brand (anvil mark) showing crafted per-track processing; colors: slate + brass.
Concept 3: Timeline with smoothing arc — timeline with a smoothing arc drawn above the cuts, implying shaping per track; colors: teal + charcoal.

RangeKeeper
Concept 1: Lock + waveform — padlock with a clean waveform in the negative space, signaling preservation of range; colors: deep green + white waveform.
Concept 2: Safe dial + dynamic bars — safe-door dial centered over vertical DR bars; colors: forest green + soft gray.
Concept 3: Shield + balanced waveform — rounded shield containing a symmetrical waveform with capped peaks (non-destructive); colors: navy + mint.

LevelKeeper
Concept 1: Keyhole + level meter — elegant keyhole silhouette with small level needle, conveys safeguarding target loudness; colors: teal + dark slate.
Concept 2: Lifebuoy + volume knob — lifebuoy ring around a volume knob set to target, suggests rescue/preservation; colors: coral + slate.
Concept 3: Locked fader icon — classic vertical fader with a small padlock overlay on the knob; colors: charcoal + lime accent on lock.

Design notes (quick)
Style: keep icons legible at small sizes (app tray / toolbar). Favor simplified silhouettes and 1–2 strong accents.
Palette: pick one primary (dark/neutral) + one accent (teal/orange/amber/green) for contrast and accessibility.
Usage: provide flat SVG and a monochrome variant for toolbar/system integration.

Would you like:
SVG sketch drafts for your top 3 name/icon concepts (I can generate basic SVG code)?
Color palette swatches and a single, consistent icon system to apply across the name family?
