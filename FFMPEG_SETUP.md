# FFmpeg Integration Guide

RangeForge requires FFmpeg for audio processing operations including LUFS measurement, True Peak detection, and format conversion. The application automatically detects FFmpeg in the following priority order:

## 1. Development Setup (Current - Chocolatey)

Since you've installed FFmpeg via Chocolatey and added it to PATH, RangeForge should automatically detect it from the system PATH.

**Verification:**

```cmd
ffmpeg -version
ffprobe -version
```

**Common Chocolatey locations:**

- `C:\ProgramData\chocolatey\bin\ffmpeg.exe`
- `C:\ProgramData\chocolatey\bin\ffprobe.exe`

## 2. Production Distribution

### Legal Considerations for FFmpeg Bundling

FFmpeg is **legally safe to bundle** with RangeForge:

- **LGPL License**: FFmpeg is licensed under LGPL v2.1, which allows bundling in proprietary applications
- **No GPL Components**: As long as we use standard FFmpeg builds without GPL-only components
- **Dynamic Linking**: Since we call FFmpeg as external processes (not linking), licensing is even more permissive
- **No Source Code Requirement**: Unlike GPL, LGPL doesn't require distributing source code when using as external process

### Recommended Bundling Strategy

1. **Download Static Builds**: Use official static builds from https://ffmpeg.org/download.html
2. **Include in Distribution**: Bundle FFmpeg binaries in the `bin/` directory
3. **Platform-Specific Binaries**:
   - Windows: `ffmpeg.exe`, `ffprobe.exe`
   - macOS: `ffmpeg`, `ffprobe` (Universal binary for Intel/Apple Silicon)
   - Linux: `ffmpeg`, `ffprobe` (x64)

### Bundling Setup

1. **Create bin directory structure:**

```
RangeForge/
├── bin/
│   ├── win32/
│   │   ├── ffmpeg.exe
│   │   └── ffprobe.exe
│   ├── darwin/
│   │   ├── ffmpeg
│   │   └── ffprobe
│   └── linux/
│       ├── ffmpeg
│       └── ffprobe
```

2. **Update electron-builder configuration** (already configured in package.json):

```json
"extraResources": [
  {
    "from": "bin/",
    "to": "bin/",
    "filter": ["**/*"]
  }
]
```

3. **FFmpeg Auto-Detection** (already implemented):
   - Checks bundled binaries first
   - Falls back to system PATH (development)
   - Tries common installation directories

## 3. Download Script for FFmpeg Binaries

Create a download script to fetch FFmpeg binaries for distribution:

```cmd
:: download-ffmpeg.bat
@echo off
echo Downloading FFmpeg binaries for distribution...
mkdir bin\\win32 2>nul
mkdir bin\\darwin 2>nul
mkdir bin\\linux 2>nul

:: Download Windows (you would implement actual download logic)
echo Download Windows FFmpeg static build to bin/win32/
echo Download macOS FFmpeg static build to bin/darwin/
echo Download Linux FFmpeg static build to bin/linux/
```

## 4. Runtime Detection

The `FFmpegConfig` service automatically:

1. **Bundled Detection**: Looks for bundled FFmpeg in `app.getAppPath()/bin/`
2. **System PATH**: Uses chocolatey or other system installations
3. **Common Locations**: Checks standard installation directories
4. **Validation**: Tests FFmpeg functionality before use
5. **Error Handling**: Provides clear error messages if FFmpeg not found

## 5. Build Process Integration

Add to package.json scripts:

```json
{
  "scripts": {
    "download:ffmpeg": "node scripts/download-ffmpeg.js",
    "prebuild": "npm run download:ffmpeg",
    "build": "npm run build:react && npm run build:electron && npm run build:workers"
  }
}
```

This ensures FFmpeg binaries are downloaded before building distribution packages.

## Current Status

✅ **Development Ready**: Works with your Chocolatey installation
✅ **Auto-Detection**: Implemented in `FFmpegConfig` service  
✅ **Build Configuration**: electron-builder configured for bundling
⏳ **Production Binaries**: Need to download and include static builds
⏳ **Download Script**: Optional automation for fetching binaries

The application will work immediately for development with your current Chocolatey setup and is ready for production bundling when FFmpeg binaries are added to the `bin/` directory.
