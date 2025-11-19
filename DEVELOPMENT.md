# Development Setup Guide

This guide will help you set up the RangeForge development environment on your system.

## Prerequisites Installation

### 1. Node.js and npm

- Download and install Node.js 18+ from [nodejs.org](https://nodejs.org/)
- Verify installation:
  ```bash
  node --version
  npm --version
  ```

### 2. FFmpeg Installation

#### Windows

1. Download FFmpeg from [ffmpeg.org](https://ffmpeg.org/download.html)
2. Extract to `C:\ffmpeg`
3. Add `C:\ffmpeg\bin` to your PATH environment variable
4. Verify installation:
   ```cmd
   ffmpeg -version
   ```

#### macOS (using Homebrew)

```bash
brew install ffmpeg
```

#### Linux (Ubuntu/Debian)

```bash
sudo apt update
sudo apt install ffmpeg
```

### 3. Git (optional)

- Download from [git-scm.com](https://git-scm.com/) if not already installed

## Project Setup

### 1. Install Dependencies

```bash
cd RangeForge
npm install
```

This will install all necessary dependencies including:

- React and TypeScript for the frontend
- Electron for the desktop framework
- FFmpeg bindings for audio processing
- Worker thread and memory mapping libraries

### 2. Configure Development Environment

#### VSCode (Recommended)

If using Visual Studio Code, install these recommended extensions:

- TypeScript and JavaScript Language Features
- React Snippets
- Electron Debug
- ESLint
- Prettier

#### Environment Variables (Optional)

Create a `.env.local` file in the root directory if you need custom configuration:

```bash
# FFmpeg custom path (if not in PATH)
FFMPEG_PATH=/custom/path/to/ffmpeg

# Custom temp directory
TEMP_DIR=/custom/temp/path

# Debug mode
DEBUG_MODE=true
```

## Development Workflow

### 1. Start Development Servers

```bash
# Start both React and Electron in development mode
npm run dev
```

This command will:

- Start the React development server on http://localhost:3000
- Compile TypeScript for Electron and workers
- Launch Electron with hot reload enabled
- Open developer tools automatically

### 2. Individual Development Commands

#### Frontend Development (React)

```bash
# Start only React development server
npm run dev:react

# Run React tests
npm test

# Build React for production
npm run build:react
```

#### Backend Development (Electron)

```bash
# Compile Electron TypeScript
npm run build:electron

# Compile Worker TypeScript
npm run build:workers

# Start Electron with compiled code
npm run electron:dev
```

### 3. Code Structure Guidelines

#### TypeScript Configuration

- `tsconfig.json` - React/frontend configuration
- `tsconfig.electron.json` - Electron main process configuration
- `tsconfig.workers.json` - Worker threads configuration

#### File Organization

```
src/
├── components/     # Reusable React components
├── context/       # React Context providers
├── hooks/         # Custom React hooks
├── types/         # TypeScript type definitions
└── utils/         # Utility functions

electron/
├── services/      # Business logic services
├── main.ts       # Main process entry point
└── preload.ts    # Renderer process bridge

workers/
├── audioWorker.ts    # Audio processing worker
└── fileWorker.ts     # File processing worker
```

## Testing and Debugging

### 1. React Testing

```bash
# Run tests in watch mode
npm test

# Run tests with coverage
npm test -- --coverage

# Run specific test file
npm test -- FileManager.test.tsx
```

### 2. Electron Debugging

#### Main Process Debugging

1. Add breakpoints in Electron code
2. Start with debug flag:
   ```bash
   npm run dev:electron -- --inspect
   ```
3. Open Chrome DevTools at `chrome://inspect`

#### Renderer Process Debugging

- Development tools are opened automatically in dev mode
- Or press `Ctrl+Shift+I` / `Cmd+Option+I` in the Electron window

### 3. Worker Thread Debugging

```bash
# Enable worker debugging
NODE_OPTIONS="--inspect" npm run dev
```

## Building and Packaging

### 1. Development Build

```bash
# Build all components
npm run build

# Test the production build
npm run pack
```

### 2. Production Distribution

```bash
# Package for current platform
npm run dist

# Package for specific platforms
npm run dist:win    # Windows
npm run dist:mac    # macOS
npm run dist:linux  # Linux
```

### 3. Build Troubleshooting

#### Common Issues

1. **Missing FFmpeg**: Ensure FFmpeg is installed and in PATH
2. **Node Version**: Use Node.js 18+ for compatibility
3. **Native Dependencies**: May require rebuilding for Electron:
   ```bash
   npm run postinstall
   ```

#### Clean Build

```bash
# Clear all build artifacts
rm -rf node_modules dist build release
npm install
npm run build
```

## Performance Tips

### 1. Development Performance

- Use `npm run dev:react` for frontend-only development
- Enable React Fast Refresh for instant updates
- Use Chrome DevTools Performance tab for profiling

### 2. Production Performance

- Enable production optimizations in build
- Use worker threads for CPU-intensive tasks
- Implement memory-mapped files for large audio files

### 3. Memory Management

```bash
# Monitor memory usage during development
node --inspect --max-old-space-size=4096 electron/main.js
```

## Additional Tools

### 1. Audio Testing Files

Create a `test-files` directory with sample audio files:

- Various formats (MP3, WAV, FLAC)
- Different sample rates and bit depths
- Files with known dynamic range values

### 2. Development Database

For testing purposes, you can use audio analysis results caching:

```bash
# Enable result caching in development
DEBUG_CACHE=true npm run dev
```

### 3. Logging

Enable detailed logging for debugging:

```bash
# Enable verbose logging
DEBUG=* npm run dev

# Enable specific module logging
DEBUG=audio:* npm run dev
```

## Contribution Guidelines

### 1. Code Style

- Follow TypeScript strict mode guidelines
- Use ESLint and Prettier for code formatting
- Write meaningful commit messages

### 2. Testing Requirements

- Add tests for new React components
- Test audio processing functions with sample files
- Ensure cross-platform compatibility

### 3. Documentation

- Update README for new features
- Add JSDoc comments for complex functions
- Include usage examples for new APIs

## Troubleshooting Common Issues

### 1. Installation Issues

```bash
# Clear npm cache
npm cache clean --force

# Reinstall dependencies
rm -rf node_modules package-lock.json
npm install
```

### 2. Build Issues

```bash
# Rebuild native modules for Electron
npx electron-rebuild

# Clear TypeScript cache
npx tsc --build --clean
```

### 3. Runtime Issues

- Check console output for error messages
- Verify FFmpeg installation and PATH
- Ensure audio files are not corrupted or locked

For additional help, check the main README.md or create an issue in the project repository.
