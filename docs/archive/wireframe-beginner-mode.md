# RangeForge - Beginner Mode Wireframe

## Layout: Simple & Clean

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│ 🎵 RangeForge                                          [Beginner] [Int.] [Exp.] │
├─────────────────────────────────────────────────────────────────────────────────┤
│                                                                                 │
│ [75% - FILE LIST AREA - Drop files anywhere in this window]                    │
│                                                                                 │
│ ┌─────────────────────────────────────────────────────────────────────────────┐ │
│ │ Filename                    │ Volume      │ New Volume       │ Genre         │ │
│ ├─────────────────────────────┼─────────────┼──────────────────┼───────────────┤ │
│ │ 01 - Track One.flac        │ Too Loud 🔊 │ Will Be Quieter  │ Rock          │ │
│ │ 02 - Track Two.flac        │ Perfect 🎯  │ No Change        │ Alternative   │ │
│ │ 03 - Track Three.flac      │ Too Quiet🔉 │ Will Be Louder   │ Rock          │ │
│ │ Single Song.mp3            │ ⏳ Reading...│ ⏳ Calculating... │ ⏳ Loading...  │ │
│ │                            │             │                  │               │ │
│ │ [Empty rows for more files to be dropped]                                   │ │
│ │                                                                             │ │
│ └─────────────────────────────────────────────────────────────────────────────┘ │
│                                                                                 │
│ [25% - PROCESSING CONTROLS]                                                     │
│                                                                                 │
│ ┌───────────────────────┐ ┌─────────────────────────────────────────────────────┐ │
│ │ 🎚️ Target Volume      │ │ 📤 Output                                          │ │
│ │                      │ │                                                     │ │
│ │ ● Normal Listening   │ │ Format: [ALAC Quality ▼]                          │ │
│ │ ○ Quiet Room         │ │ Naming: [01 - Song.alac ▼]                        │ │
│ │ ○ Party/Loud         │ │                                                     │ │
│ │                      │ │         🚀 [MAKE IT SO]                           │ │
│ │ ☑ Prevent Distortion │ │                                                     │ │
│ └───────────────────────┘ └─────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────────┘
```

## Key Features

### Data Population Flow

1. **Initial Drop**: Shows filename only
2. **CUE Detection**: Expands single file into track listing
3. **Metadata Reading**: Genre column populates from file/CUE data
4. **Audio Analysis**: Volume assessment completes (Too Loud/Perfect/Too Quiet)
5. **Target Calculation**: "New Volume" shows planned adjustment

### User Interactions

- **Whole window**: Drop zone for audio files
- **Volume presets**: Simple radio buttons (Normal/Quiet/Party)
- **Output format**: Basic dropdown (ALAC Quality)
- **File naming**: Simple presets dropdown
- **Processing**: Single "MAKE IT SO" button

### Visual Indicators

- 🔊 = Too loud (needs quieter)
- 🎯 = Perfect target volume
- 🔉 = Too quiet (needs louder)
- ⏳ = Processing in progress

### Simplified Language

- No LUFS/dBTP terminology
- "Volume" instead of "Loudness"
- "Will Be Quieter/Louder" instead of dB adjustments
- "Prevent Distortion" instead of "Peak Limiting"
