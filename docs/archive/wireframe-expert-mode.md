# RangeForge - Expert Mode Wireframe

## Layout: Full Control

```text
┌─────────────────────────────────────────────────────────────────────────────────┐
│ 🎵 RangeForge                                    [Begin.] [Inter.] [Expert] │ ⚙️ │
├─────────────────────────────────────────────────────────────────────────────────┤
│                                                                                 │
│ [75% - FILE LIST AREA - Drop files anywhere in this window]                    │
│                                                                                 │
│ ┌─────────────────────────────────────────────────────────────────────────────┐ │
│ │Filename             │I.LUFS│M.LUFS│S.LUFS│dBTP│→LUFS│→dBTP│ΔdB│DR│Genre   │□│ │
│ ├─────────────────────┼──────┼──────┼──────┼────┼─────┼─────┼───┼──┼────────┼─┤ │
│ │01 - Track One.flac │ -12.3│ -8.1 │ -9.2 │-0.1│-18.0│-1.0 │-6 │8 │Alt Rock│✓│ │
│ │02 - Track Two.flac │ -18.1│-14.2 │-15.8 │-2.3│-18.0│-1.0 │ 0 │12│Alt Rock│✓│ │
│ │03 - Track Three... │ -24.2│-20.1 │-21.4 │-8.2│-18.0│-1.0 │+6 │9 │Alt Rock│✓│ │
│ │Single Song.mp3     │⏳ Ana.│⏳ Ana│⏳ Ana│⏳ -│-18.0│-1.0 │⏳ -│--│⏳ Det.. │□│ │
│ │                    │      │      │      │    │     │     │   │  │        │ │ │
│ │ [Empty rows]                                                              │ │
│ └─────────────────────────────────────────────────────────────────────────────┘ │
│                                                                                 │
│ [25% - PROCESSING CONTROLS]                                                     │
│                                                                                 │
│ ┌───────────────────────┐ ┌─────────────────────────────────────────────────────┐ │
│ │ 🎯 Loudness Targeting │ │ 📤 Export Configuration                            │ │
│ │ I.LUFS: [-18.0] ±0.1  │ │ Codec: [ALAC ▼] Sample: [Original ▼] Bit: [24▼]   │ │
│ │ M.LUFS: [Auto calc]   │ │ Template: {artist}/{album}/{trackNum:02d} - {title}│ │
│ │ S.LUFS: [Auto calc]   │ │ Output: [C:\Music\Processed\] [📁]                │ │
│ │ dBTP:   [-1.0] Max    │ │                                                     │ │
│ │                       │ │ ⚡ CPU Cores: [Auto (16)] 🧠 RAM: [8GB Max]        │ │
│ │ 🔧 Advanced Options   │ │                                                     │ │
│ │ ☑ True Peak Scanning  │ │    📊 Queue: 4 files │ ETA: ~2min                 │ │
│ │ ☑ Gating (-70 LUFS)   │ │         🚀 [MAKE IT SO]  ⏸️ [PAUSE] 🛑 [STOP]    │ │
│ │ ☐ Multi-thread Analysis│ │                                                     │ │
│ │ ☑ Memory Mapping      │ │ 📋 [Export Log] 📊 [Analysis Report]              │ │
│ └───────────────────────┘ └─────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────────┘
```

## Key Features

### Comprehensive Audio Analysis

- **I.LUFS**: Integrated loudness (primary target)
- **M.LUFS**: Momentary loudness (peak detection)
- **S.LUFS**: Short-term loudness (dynamic sections)
- **dBTP**: True peak levels (clipping prevention)
- **→LUFS/→dBTP**: Target values for processing
- **ΔdB**: Exact adjustment amount
- **DR**: Dynamic range measurement

### Professional Controls

#### Loudness Targeting

- **Precision targeting**: ±0.1 LUFS tolerance
- **Automatic calculations**: Momentary and short-term derived from integrated
- **Peak limiting**: True peak maximum setting
- **Professional standards**: Full compliance with broadcast/streaming specs

#### Advanced Processing Options

- **True Peak Scanning**: Sample-accurate peak detection
- **Gating**: Professional loudness gating at -70 LUFS
- **Multi-threading**: CPU optimization control
- **Memory Mapping**: Large file handling optimization

### Export Configuration

#### Technical Controls

- **Codec selection**: Full format control (ALAC, FLAC, etc.)
- **Sample rate**: Original or specific rate conversion
- **Bit depth**: Precision control (16/24/32-bit)
- **Advanced templates**: Full variable substitution system

#### System Resource Management

- **CPU utilization**: Core allocation control
- **RAM management**: Memory usage limits
- **Processing queue**: Batch management with ETA
- **Progress monitoring**: Detailed processing feedback

### Professional Workflow

#### Quality Assurance

- **Per-track validation**: Checkboxes for manual approval
- **Export logging**: Detailed processing reports
- **Analysis reports**: Professional documentation
- **Pause/resume**: Workflow interruption control

#### Data Population Flow

1. **File analysis**: Complete loudness measurement suite
2. **Target calculation**: Professional-grade precision
3. **Quality validation**: Manual review checkboxes
4. **Batch processing**: Queue management with resource control

### Expert-Level Features

- **Settings gear**: Advanced configuration access
- **Professional terminology**: Full technical accuracy
- **Precision controls**: Exact values for all parameters
- **Resource monitoring**: System utilization feedback
- **Professional reporting**: Industry-standard documentation
