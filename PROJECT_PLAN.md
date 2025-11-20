# RangeForge Project Plan & Configuration

## Project Overview

RangeForge is a professional audio dynamic range analysis and volume processing application built with Electron, React, and Node.js. This document outlines the project roadmap, features, and configuration settings.

---

## Core Features & Roadmap

### Phase 1: Foundation (Current)

- [x] Electron + React + TypeScript boilerplate
- [x] Audio analysis service with FFmpeg integration
- [x] Worker threads for multithreaded processing
- [x] Memory-mapped file I/O support
- [x] Cross-platform build configuration
- [ ] Basic UI implementation
- [ ] Core audio analysis functionality

### Phase 2: Audio Processing Engine

- [ ] Volume level detection and analysis
- [ ] Dynamic range (DR) measurement algorithms
- [ ] EBU R128 loudness measurement implementation
- [ ] Batch processing capabilities
- [ ] Real-time progress monitoring
- [ ] Audio normalization and limiting features

### Phase 3: Enhanced Features

- [ ] Last.fm API integration for metadata enrichment
- [ ] Audio fingerprinting and identification
- [ ] Playlist and library management
- [ ] Export capabilities (reports, processed files)
- [ ] Advanced filtering and search
- [ ] Custom presets and profiles

### Phase 4: Professional Tools

- [ ] Advanced DR analysis algorithms
- [ ] Professional mastering tools
- [ ] Batch comparison reports
- [ ] Quality assurance workflows
- [ ] Integration with external tools
- [ ] Plugin architecture

---

## API Keys & Secrets Configuration

### Last.fm API Integration

**Purpose**: Metadata enrichment, track identification, and music database features
**Registered to**: superjudge42

**Required Credentials**:

- API Key: `869104bf8f37f005a8c38cd85d1cdd58`
- Shared Secret: `df53c7ec1eef58d3f30ca0bb03add1e8`

**Setup Instructions**:

1. Register at: https://www.last.fm/api/account/create
2. Create a new application
3. Copy the API Key and Shared Secret
4. Replace the placeholders above with your actual credentials
5. The app will automatically use these for metadata lookups

**Features Enabled**:

- Automatic track identification
- Album art retrieval
- Artist and album metadata
- Similar tracks recommendations
- Music library organization

### Future API Integrations (Planned)

#### Spotify API (Optional)

**Purpose**: Enhanced metadata and playlist features

- Client ID: `[USER_ENTER_SPOTIFY_CLIENT_ID_HERE]`
- Client Secret: `[USER_ENTER_SPOTIFY_CLIENT_SECRET_HERE]`

#### MusicBrainz API (Optional)

**Purpose**: Open music database integration

- No API key required (rate-limited)
- User Agent: `[USER_ENTER_APP_CONTACT_INFO_HERE]`

#### Discogs API (Optional)

**Purpose**: Vinyl and physical media database

- Consumer Key: `[USER_ENTER_DISCOGS_CONSUMER_KEY_HERE]`
- Consumer Secret: `[USER_ENTER_DISCOGS_CONSUMER_SECRET_HERE]`

---

## Technical Specifications

### Audio Processing Requirements

- **Supported Output Formats**: MP3, WAV, FLAC, M4A, AAC, OGG, ALAC
- **Sample Rates**: 8 kHz to 192 kHz
- **Bit Depths**: 8-bit to 32-bit (including floating point)
- **Channels**: Mono, Stereo, Surround (up to 7.1)

- **Input Format**: Anything Ffmpeg will dedode
- **Fallback Parameters**: Fallbacks are supplied for output format, sample rate, bit depth, and channel downsampling for formats such as DSD

### Performance Targets

- **File Processing**: 50+ files simultaneously
- **Memory Usage**: <2GB for typical workflows
- **Processing Speed**: Real-time or faster for most formats
- **Response Time**: <100ms UI responsiveness

### Platform Support

- **Primary**: Windows 10/11 (x64)
- **Secondary**: macOS 10.15+ (Intel/Apple Silicon)
- **Tertiary**: Linux (Ubuntu/Debian x64)

---

## Feature Specifications

### Dynamic Range Analysis

**Algorithm**: ITU-R BS.1770-4 compliant measurement
**Metrics**:

- Peak levels (dBFS)
- RMS levels (dBFS)
- LUFS (Loudness Units relative to Full Scale)
- LRA (Loudness Range)
- True Peak (dBTP)
- Dynamic Range (DR) values

### Audio Fingerprinting

**Technology**: Chromaprint/AcoustID integration
**Use Cases**:

- Duplicate detection
- Track identification
- Version comparison
- Quality assessment

### Batch Processing

**Capabilities**:

- Multi-threaded analysis (configurable thread count)
- Progress tracking with ETA
- Error handling and recovery
- Resumable operations
- Export to multiple formats (CSV, JSON, XML, PDF reports)

---

## User Interface Design

### Main Application Layout

```
┌─────────────────────────────────────────────────┐
│ Menu Bar                                        │
├─────────────────┬───────────────────────────────┤
│ File Manager    │ Analysis Results              │
│ - Drag & Drop   │ - Waveform Display           │
│ - File List     │ - Metrics Table              │
│ - Batch Queue   │ - Visual Charts              │
├─────────────────┼───────────────────────────────┤
│ Processing      │ Audio Player                  │
│ Options         │ - Playback Controls           │
│ - Presets       │ - A/B Comparison             │
│ - Settings      │ - Real-time Monitoring       │
└─────────────────┴───────────────────────────────┘
```

### Key User Workflows

1. **File Import**: Drag files → Auto-analysis → Results display
2. **Batch Processing**: Select files → Choose preset → Monitor progress
3. **Quality Assessment**: Load file → Visual analysis → Generate report
4. **Audio Comparison**: Load multiple versions → Side-by-side analysis

---

## Configuration Settings

### Application Preferences

```javascript
{
  // Processing Settings
  "maxWorkerThreads": 4,
  "memoryMapThreshold": "100MB",
  "tempDirectory": "./temp",
  "cacheAnalysisResults": true,

  // Audio Settings
  "defaultAnalysisPreset": "broadcast_standard",
  "targetLoudness": -23.0, // LUFS
  "peakLimiter": -1.0,     // dBFS

  // UI Settings
  "theme": "dark",
  "waveformQuality": "high",
  "autoLoadLastSession": true,

  // API Settings
  "lastfmApiKey": "[USER_CONFIGURED]",
  "lastfmSecret": "[USER_CONFIGURED]",
  "enableMetadataLookup": true,
  "cacheMetadata": true,

  // Export Settings
  "defaultExportFormat": "csv",
  "includeWaveformImages": false,
  "reportTemplate": "standard"
}
```

---

## Development Notes

### Current Status

- Basic project structure implemented
- Audio processing foundation in place
- Cross-platform build system configured
- Modern npm/Node.js compatibility ensured

### Next Steps

1. Implement core audio analysis algorithms
2. Build React UI components for file management
3. Add FFmpeg integration for format support
4. Implement worker thread processing
5. Add Last.fm API integration
6. Create export and reporting system

### Known Issues & Considerations

- `mmap-io` package compatibility with Node.js 22
- FFmpeg binary distribution strategy
- Cross-platform audio codec support
- Memory usage optimization for large file batches
- Error handling for corrupted audio files

---

## Testing Strategy

### Unit Tests

- Audio analysis algorithms
- File processing functions
- API integration modules
- UI component behavior

### Integration Tests

- End-to-end file processing workflows
- Cross-platform compatibility
- Performance benchmarking
- Memory leak detection

### User Acceptance Tests

- Professional audio engineer workflows
- Batch processing scenarios
- Export functionality
- Error recovery procedures

---

## Deployment & Distribution

### Build Artifacts

- **Windows**: NSIS installer + portable executable
- **macOS**: DMG disk image + ZIP archive
- **Linux**: AppImage + DEB package

### Update Mechanism

- Electron auto-updater integration
- Staged rollout capability
- Rollback functionality
- Update notification system

### Documentation

- User manual with screenshots
- Developer API documentation
- Video tutorials for key workflows
- Troubleshooting guide

---

## Branding & Application Naming

### Current Name: RangeForge

**Selected concept emphasizing the crafting/forging of audio dynamic range**

### Alternative Naming Concepts Considered

#### RangeSmith

- **Concept 1**: Anvil + waveform — stylized anvil with waveform carved across top
- **Concept 2**: Hammer + dynamic bars — hammer striking vertical bars forming curve
- **Concept 3**: Precision ruler + waveform — minimal ruler overlaying gentle waveform

#### LevelSmith

- **Concept 1**: Smith's hammer + level meter — hammer crossing horizontal level meter
- **Concept 2**: Shield + volume fader — shield containing centered fader knob
- **Concept 3**: Gear + waveform slice — gear with waveform slice inside

#### TrackSmith

- **Concept 1**: Vinyl record + small hammer — record with grooves and tiny hammer overlay
- **Concept 2**: Stacked tracks + wrench — three horizontal rectangles with small wrench
- **Concept 3**: Notched timeline + anvil — timeline with cut markers and anvil icon

#### LevelForge

- **Concept 1**: Anvil + level slider — anvil with horizontal slider embedded
- **Concept 2**: Flame-forge + meter needle — forge element with analogue meter
- **Concept 3**: Metal plate + balanced bars — rectangular plate with evenly spaced bars

#### TrackForge

- **Concept 1**: Multi-track lines + hammer — three parallel lines with hammer striking
- **Concept 2**: Forge stamp on track — single track with circular stamp/brand
- **Concept 3**: Timeline with smoothing arc — timeline with smoothing arc above cuts

#### RangeKeeper

- **Concept 1**: Lock + waveform — padlock with waveform in negative space
- **Concept 2**: Safe dial + dynamic bars — safe-door dial over vertical DR bars
- **Concept 3**: Shield + balanced waveform — shield containing symmetrical waveform

#### LevelKeeper

- **Concept 1**: Keyhole + level meter — keyhole silhouette with small level needle
- **Concept 2**: Lifebuoy + volume knob — lifebuoy ring around volume knob
- **Concept 3**: Locked fader — vertical fader with small padlock overlay

### Visual Identity Guidelines

**Design Principles**:

- Keep icons legible at small sizes (app tray/toolbar)
- Favor simplified silhouettes with 1-2 strong accents
- Maintain accessibility with high contrast

**Color Palette Options**:

- **Primary**: Dark/neutral (charcoal, slate, graphite, obsidian)
- **Accent**: Single bright color (teal, orange, amber, mint, lime)

**Icon Requirements**:

- Flat SVG format for scalability
- Monochrome variant for system integration
- Optimized for 16x16, 32x32, 64x64, 256x256 sizes

### Selected Branding: RangeForge

**Chosen Icon Concept**: Forge flame + waveform

- Stylized flame with negative-space waveform inside
- **Colors**: Ember orange flame + dark slate background
- **Symbolism**: Crafting/forging audio with energy and precision
- **Variants**: Full color, monochrome, simplified versions

---

## Detailed UI Specifications

### Main Window Layout

```
┌─────────────────────────────────────────────────────────────────────┐
│ Title Bar: "RangeForge"                                             │
├─────────────────────────────────────────────────────────────────────┤
│ Drop Zone / File List (Table Format)                               │
│ Headers: File Name | Duration | Format | Sample Rate | Bit Depth | DR │
├─────────────────────────────────────────────────────────────────────┤
│ Target Output Settings (Bottom Section)                            │
│ ┌─────────────────┬──────────────────┬───────────────┬────────────┐ │
│ │ Auto Mode ☑     │ Target Volume    │ Target Codec  │ Sample Rate│ │
│ │ [Toggle/Switch] │ [Slider/Input]   │ [Dropdown]    │ [Dropdown] │ │
│ └─────────────────┴──────────────────┴───────────────┴────────────┘ │
│ ┌─────────────────┬──────────────────┬───────────────┬────────────┐ │
│ │ Target Bitrate  │ Last.fm Genre    │ Top Tracks Tag│ Play Count │ │
│ │ [Dropdown]      │ ☑ [Checkboxes]  │ [Slider/Input]│ [Slider]   │ │
│ │ ORIGINAL, 4bit, │ Artist/Album/    │ "popular" when│ Tag thresh │ │
│ │ 8bit, 16bit...  │ Song tags        │ in top N tracks│ for listens│ │
│ └─────────────────┴──────────────────┴───────────────┴────────────┘ │
└─────────────────────────────────────────────────────────────────────┘
```

### Control Specifications

**Auto Mode Toggle**: Boolean switch for automatic processing
**Target Volume**: Slider or numeric input for loudness target
**Target Codecs**: Dropdown with options:

- ORIGINAL, aac, ac3, alac, eac3, flac, libmp3lame, libopus
- PCM variants: pcm_s16le, pcm_s24le, pcm_s32le

**Sample Rates**: Dropdown with options:

- ORIGINAL, 32kHz, 44.1kHz, 48kHz, 88kHz, 96kHz, 192kHz, CUSTOM

**Bit Rates**: Dropdown with options:

- ORIGINAL, 4bit, 8bit, 16bit, 32bit

**Last.fm Integration Controls**:

- Genre tag checkboxes for Artist/Album/Song sources
- Top tracks slider for popularity threshold
- Play count slider for listen-based tagging

### Processing Workflow

1. **File Detection**: Identify individual tracks vs. album+CUE combinations
2. **CUE Processing**: Split albums using CUE metadata when FILE field matches
3. **Analysis**: Use FFmpeg to analyze dynamic range of all audio files
4. **Export Options**: Manual or automatic export with selected parameters
5. **Metadata Enhancement**: Apply Last.fm tags based on user preferences

---

_Last Updated: November 19, 2025_
_Document Version: 1.1_
