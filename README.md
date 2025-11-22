# RangeForge

A professional audio dynamic range analysis and volume processing application built with Electron, React, and Node.js.

## Features

- **Audio Analysis**: Comprehensive volume level and dynamic range analysis using FFmpeg
- **Batch Processing**: Multi-threaded file processing using Worker Threads
- **Memory-Mapped Files**: Efficient large file handling with mmap
- **Volume Adjustment**: Professional volume balancing and limiting
- **Cross-Platform**: Builds for Windows, macOS, and Linux

## Technology Stack

- **Frontend**: React 18 with TypeScript
- **Backend**: Node.js with TypeScript
- **Desktop**: Electron 27
- **Audio Processing**: FFmpeg with fluent-ffmpeg
- **File Operations**: Memory-mapped I/O with mmap-io
- **Multithreading**: Worker Threads for parallel processing
- **Build System**: electron-builder for cross-platform packaging

## Project Structure

```
RangeForge/
├── src/                    # React frontend source
│   ├── components/         # React components
│   ├── context/           # React Context providers
│   └── types/             # TypeScript declarations
├── electron/              # Electron main process
│   ├── services/          # Audio and file processing services
│   ├── main.ts           # Main Electron process
│   └── preload.ts        # Preload script for security
├── workers/               # Worker thread scripts
│   ├── audioWorker.ts    # Audio processing worker
│   └── fileWorker.ts     # File processing worker
├── build/                 # React build output
├── dist/                  # TypeScript compilation output
└── release/               # Packaged app binaries
```

## Setup and Installation

### Prerequisites

- Node.js 18+
- npm or yarn
- FFmpeg (for audio processing)

### Development Setup

1. **Clone and install dependencies:**

   ```bash
   cd RangeForge
   npm install
   ```

2. **Start development servers:**
   ```bash
   npm run dev
   ```
   This will start both the React development server and Electron in development mode.

### Building for Production

1. **Build the complete application:**

   ```bash
   npm run build
   ```

2. **Package for current platform:**

   ```bash
   npm run pack
   ```

3. **Create distribution packages:**
   ```bash
   npm run dist
   ```

### Platform-Specific Builds

- **Windows**: `npm run dist:win`
- **macOS**: `npm run dist:mac`
- **Linux**: `npm run dist:linux`

## Audio Processing Features

### Supported Formats

- MP3, WAV, FLAC, M4A, AAC, OGG
- High-resolution audio support
- Batch processing capabilities

### Analysis Features

- **Volume Statistics**: Peak, RMS, mean volume levels
- **Dynamic Range**: DR measurement with quality assessment
- **Metadata Extraction**: Format, bitrate, sample rate, channels
- **EBU R128 Loudness**: Professional loudness measurement

### Processing Features

- **Volume Balancing**: Target level adjustment
- **Peak Limiting**: Configurable threshold limiting
- **Batch Processing**: Multi-file parallel processing
- **Progress Monitoring**: Real-time processing updates

## Development Scripts

```bash
# Development
npm run dev              # Start React + Electron in development
npm run dev:react        # Start React development server only
npm run dev:electron     # Start Electron in development mode

# Building
npm run build            # Build React + Electron + Workers
npm run build:react      # Build React application
npm run build:electron   # Compile Electron TypeScript
npm run build:workers    # Compile Worker TypeScript

# Packaging
npm run pack             # Package for development (faster)
npm run dist             # Create distribution packages
npm run dist:win         # Package for Windows
npm run dist:mac         # Package for macOS
npm run dist:linux       # Package for Linux

# Testing
npm test                 # Run React tests
```

## Configuration

### Electron Builder Configuration

The app packaging is configured in `package.json` under the `build` section:

- **Windows**: Creates NSIS installer and portable executable
- **macOS**: Creates DMG and ZIP packages with universal binaries
- **Linux**: Creates AppImage and DEB packages

### Audio Processing Configuration

Audio processing uses FFmpeg with the following default settings:

- **Volume Balancing**: EBU R128 standard
- **Default Target**: -23 LUFS (broadcast standard)
- **Limiter Threshold**: -1.0 dB
- **Quality**: Preserves original bit depth and sample rate

## Architecture

### Main Process (Electron)

- **main.ts**: Application lifecycle and window management
- **preload.ts**: Secure IPC bridge to renderer
- **AudioProcessor**: FFmpeg-based audio analysis and processing
- **FileProcessor**: Memory-mapped file operations and batch processing

### Renderer Process (React)

- **AudioContext**: Centralized state management for audio files
- **FileManager**: File selection and management interface
- **AudioProcessor**: Processing options and controls
- **ProgressMonitor**: Real-time processing feedback

### Worker Threads

- **audioWorker.ts**: Isolated audio processing tasks
- **fileWorker.ts**: File I/O and batch operations

## Security

The application follows Electron security best practices:

- Context isolation enabled
- Node integration disabled in renderer
- Remote module disabled
- Secure IPC communication through preload script

## Memory Management

- Memory-mapped files for efficient large file access
- Worker thread isolation prevents UI blocking
- Automatic cleanup of resources and temporary files
- Progress monitoring prevents memory leaks during long operations

## Troubleshooting

### FFmpeg Issues

- Ensure FFmpeg is installed and available in PATH
- Check FFmpeg version compatibility (4.0+)
- Verify audio codec support for target formats

### Build Issues

- Clear `node_modules` and reinstall dependencies
- Check Node.js version (18+ required)
- Verify TypeScript compilation with `npm run build:electron`

### Performance Issues

- Adjust worker thread count in processing options
- Use memory-mapped files for large audio files
- Monitor system resources during batch processing

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes with tests
4. Submit a pull request

## License

MIT License - see LICENSE file for details

## Additional Resources

- [Electron Documentation](https://www.electronjs.org/docs)
- [React Documentation](https://react.dev/)
- [FFmpeg Documentation](https://ffmpeg.org/documentation.html)
- [EBU R128 Standard](https://tech.ebu.ch/docs/r/r128.pdf)
