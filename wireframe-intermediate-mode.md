# RangeForge - Intermediate Mode Wireframe

## Layout: Smart Control

```text
┌─────────────────────────────────────────────────────────────────────────────────┐
│ 🎵 RangeForge                                          [Begin.] [Intermediate] [Exp.] │
├─────────────────────────────────────────────────────────────────────────────────┤
│                                                                                 │
│ [75% - FILE LIST AREA - Drop files anywhere in this window]                    │
│                                                                                 │
│ ┌─────────────────────────────────────────────────────────────────────────────┐ │
│ │ Filename                │Current │Target │Adjust │ DR │ Genre              │ │
│ ├─────────────────────────┼────────┼───────┼───────┼────┼────────────────────┤ │
│ │ 01 - Track One.flac    │-12 LUFS│-18🎯  │ -6dB  │ 8  │ Alternative Rock   │ │
│ │ 02 - Track Two.flac    │-18 LUFS│-18🎯  │  0dB  │ 12 │ Alternative Rock   │ │
│ │ 03 - Track Three.flac  │-24 LUFS│-18🎯  │ +6dB  │ 9  │ Alternative Rock   │ │
│ │ Single Song.mp3        │⏳ Anal.│-18🎯  │⏳ Calc│ -- │ ⏳ Detecting...     │ │
│ │                        │        │       │       │    │                    │ │
│ │ [Empty rows for more files]                                              │ │
│ │                                                                             │ │
│ └─────────────────────────────────────────────────────────────────────────────┘ │
│                                                                                 │
│ [25% - PROCESSING CONTROLS]                                                     │
│                                                                                 │
│ ┌─────────────────────────┐ ┌───────────────────────────────────────────────────┐ │
│ │ 🎯 Target Loudness      │ │ 📤 Output Settings                               │ │
│ │                         │ │                                                   │ │
│ │ ● -18 LUFS (Standard)   │ │ Format: [ALAC ▼] Quality: [Lossless ▼]          │ │
│ │ ○ -16 LUFS (iTunes)     │ │ Naming: [{trackNum:02d} - {title}.{ext} ▼]       │ │
│ │ ○ -14 LUFS (Spotify)    │ │ Folder: [Browse...]                              │ │
│ │ ○ -23 LUFS (Broadcast)  │ │                                                   │ │
│ │ ○ Custom: [-18] LUFS    │ │    📊 Progress: Ready (4 files)                  │ │
│ │                         │ │         🚀 [MAKE IT SO]                         │ │
│ │ ☑ Peak Limit (-1.0 dBTP)│ │                                                   │ │
│ │ ☑ Smart Genre Detection │ │                                                   │ │
│ └─────────────────────────┘ └───────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────────┘
```

## Key Features

### Data Population Flow

1. **Initial Drop**: Shows filename only
2. **CUE Detection**: Expands single file into track listing with metadata
3. **FFprobe Analysis**: Current LUFS values populate
4. **Target Calculation**: Adjustment values calculated automatically
5. **Genre Detection**: Smart detection + manual editing capability

### User Interactions

- **Whole window**: Drop zone for audio files
- **Target presets**: Industry-standard LUFS targets with context
- **Custom target**: Manual LUFS input field
- **Advanced options**: Peak limiting, smart genre detection toggles
- **Output control**: Format, quality, naming template, folder selection
- **Progress monitoring**: File count and processing status

### Technical Display

- **LUFS values**: Shows current integrated loudness
- **dB adjustments**: Exact volume change needed
- **Dynamic Range**: DR values for reference
- **Target indicators**: 🎯 shows active target across all tracks

### Naming Templates

- Basic variables: `{trackNum:02d} - {title}.{ext}`
- Supports padding and common patterns
- Dropdown with popular presets

### Visual Feedback

- **⏳ Anal.**: Analysis in progress
- **⏳ Calc**: Calculation in progress
- **⏳ Detecting**: Genre detection in progress
- **Color coding**: Visual status indicators
