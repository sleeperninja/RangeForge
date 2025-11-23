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

## Network / SAN Development Safety

When working across multiple machines against a SAN (network-attached storage), be careful: build tools (Electron packaging, native rebuilds, Visual Studio/MSBuild/node-gyp) can create and lock files on the shared filesystem. To avoid lost work and locked files, follow these guidelines.



```powershell
# create local temp/cache directories
New-Item -ItemType Directory -Path C:\LocalTemp -Force
New-Item -ItemType Directory -Path C:\Local\npm-cache -Force

# point npm/node/gyp to local caches for this session
$env:TEMP = 'C:\LocalTemp'
$env:TMP = 'C:\LocalTemp'
$env:NPM_CONFIG_CACHE = 'C:\Local\npm-cache'
$env:TMPDIR = 'C:\LocalTemp'

# then run renderer-only dev (safe):
npm run dev:react
```

## VSCode / Build Checklist (What to run where)

Use this checklist when preparing to build/rebuild Electron and native modules on a Windows development machine. Items marked **(Admin)** require elevated terminal privileges. Items marked **(VS Terminal)** should be run in the Visual Studio Developer PowerShell (so MSBuild/node-gyp can find build tools). Items marked **(VSCode)** can be run from the integrated terminal in VSCode.

- [ ] **Clone locally or prepare SAN-safe environment** — ensure code is on a local drive or you have redirected temp/cache to a local path as described above. (VSCode)

- [ ] **Open VSCode** and the project folder `C:\Dev\RangeForge` (or your local path). (VSCode)

- [ ] **Install dependencies** — run in VSCode integrated terminal: `npm install`. If you need Visual Studio build tools for native modules, open a Visual Studio Developer PowerShell instead. (VSCode / VS Terminal)

- [ ] **Set local temp & npm cache (optional but recommended when using SAN)** — run in PowerShell (Admin not required):

```powershell
$env:TEMP = 'C:\LocalTemp'
$env:TMP = 'C:\LocalTemp'
$env:NPM_CONFIG_CACHE = 'C:\Local\npm-cache'
```

- [ ] **Rebuild native modules for Electron** — required for modules like `mmap-io`. Run in Visual Studio Developer PowerShell so node-gyp/MSBuild are available. This may require admin elevation depending on your Windows environment: (VS Terminal) **(Admin if prompted)**

```powershell
npx electron-rebuild
```

- [ ] **Compile Electron TypeScript** — compiles `electron/main.ts` to `dist`. Run in VSCode terminal. (VSCode)

```powershell
npm run build:electron
```

- [ ] **Run Electron locally (dev)** — run from the compiled local `dist` folder or directly with `electron .` while React dev server is running. If Electron is installed as a dev dependency, use `npx electron .` or `npm run electron`. (VSCode)

```powershell
# Start React dev server in one terminal (VSCode)
npm run dev:react

# In another terminal after compile/run
npm run electron
```

- [ ] **Avoid packaging on SAN** — do not run `npm run pack` or `npm run dist` on the SAN-mounted repo; run packaging on a local clone. (VS Terminal/VSCode)

- [ ] **If you see file locks** — use Process Explorer or Resource Monitor to identify locking process. If the lock is from an MSBuild/VS process, close Visual Studio and retry. As a last resort, reboot the machine that has the lock. (External)

Notes (Visual Studio Community 2026 — Feature Update 18.0.1):

- Visual Studio Community 2026 (v18.0.1, Nov 2025 Feature Update) is the target developer environment for native rebuilds on Windows. Install Visual Studio Community 2026 and include the **"Desktop development with C++"** workload.

- Recommended Visual Studio components (install via Visual Studio Installer):
   - Desktop development with C++ workload
   - Latest MSVC v143 (or the toolset offered with VS 2026)
   - Windows 10/11 SDK (recommended)
   - Optional: CMake and Git for Windows

- Opening the correct shell (important):
   - Start Menu → **Developer PowerShell for Visual Studio 2026** (preferred)
   - Or Start Menu → **x64 Native Tools Command Prompt for VS 2026**
   - If you need Admin rights, right-click the shell and choose **Run as administrator**.

- Verify toolchain availability in that shell:

```powershell
where msbuild
msbuild -version
python --version
```

- node-gyp / electron-rebuild compatibility:
   - If node-gyp still has trouble detecting VS, you can set the msvs version to a known token that node-gyp recognizes (many setups use `2022` as the token even when running newer VS releases):

```powershell
npm config set msvs_version 2022
```

   - Alternatively pass `--msvs_version=2022` to `electron-rebuild`. In most cases running `electron-rebuild` from the Developer PowerShell will let node-gyp detect MSBuild automatically and you won't need to set this.

## Automated / Unattended Install: Visual Studio 2022 Build Tools (PowerShell)

If you want to install the minimal Build Tools for C++ automatically (useful for CI or to avoid interactive installer steps), use the `vs_BuildTools.exe` installer with the C++ workload. The command below downloads the installer and runs a quiet unattended install for the C++ build tools (includes MSVC toolset, MSBuild and Windows SDK recommended components).

Run these commands in an elevated PowerShell prompt (Run as Administrator):

```powershell
# Download the Build Tools installer to a temp file
Invoke-WebRequest -Uri "https://aka.ms/vs/17/release/vs_BuildTools.exe" -OutFile "$env:TEMP\vs_BuildTools.exe"

# Unattended install: C++ build tools workload with recommended components
& "$env:TEMP\vs_BuildTools.exe" --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended --quiet --wait --norestart

# Optional: include explicit components if you want to pin specific MSVC/SDK
## Example (explicit MSVC + Windows 10 SDK 19041):
& "$env:TEMP\vs_BuildTools.exe" --add Microsoft.VisualStudio.Component.VC.Tools.x86.x64 --add Microsoft.VisualStudio.Component.Windows10SDK.19041 --quiet --wait --norestart
```

Notes:
- `--quiet` performs a silent install; remove it to use the interactive GUI instead.
- `--includeRecommended` ensures the workload's recommended components (like Windows SDK and MSBuild) are installed.
- `--norestart` prevents an automatic reboot; reboot manually when convenient.

After the installer finishes, restart your machine or at minimum restart your terminal sessions so environment variables and PATH updates take effect. Then open **Developer PowerShell for VS 2022 (x64)** and run the verification and rebuild steps shown above.

## Picking up after removal/reboot (recommended quick checklist)

1. Remove Visual Studio 2026 (if you plan to uninstall) using Visual Studio Installer, then reboot.
2. Run the unattended `vs_BuildTools.exe` command above (or install via GUI) and reboot when complete.
3. Open `Developer PowerShell for VS 2022 (x64)` and verify:

```powershell
where msbuild
msbuild -version
python --version
```

4. From the repo root (`Z:\RangeForge`) run the rebuild in that shell:

```powershell
Set-Location Z:\RangeForge
$env:GYP_MSVS_VERSION='2022'
$env:npm_config_msvs_version='2022'
npx electron-rebuild --force --verbose --msvs_version=2022
```

5. If successful, proceed with `npm run build:electron` and `npm run electron` as needed.


- Run electron-rebuild with extra debug output (in Developer PowerShell):

```powershell
$env:DEBUG = 'electron-rebuild'
npx electron-rebuild --force --verbose --msvs_version=2022
```

- Additional tips:
   - Ensure Python 3.x is on PATH; node-gyp requires it.
   - Use the x64 Developer PowerShell when building x64 native modules.
   - If you have multiple Visual Studio installations, `--msvs_version` can help choose the right toolset.
   - Perform native rebuilds and packaging on a single machine that has full network access to npm and the Visual Studio toolchain to avoid intermittent failures.

- **Avoid packaging commands on the SAN**: `npm run pack`, `npm run dist`, `npm run rebuild:native` and other electron-builder or node-gyp operations are the most likely to create locked files. Run these on a local clone instead.

- **Compile Electron main locally** if you need to run the full Electron app but don't want to package on the SAN. Example approach (compile to a local directory and run dev renderer from SAN):

```powershell
# compile electron main to a local folder
tsc -p tsconfig.electron.json --outDir C:\Local\rangeforge-dist

# in another terminal start the React dev server from the SAN repo
npm run dev:react

# then run Electron from the compiled local folder (it will load http://localhost:3000 in dev mode)
cd C:\Local\rangeforge-dist
electron .
```

- **When cloning locally is feasible:** do so. Example recommended layout:

```
C:\Dev\RangeForge\        # code and build artifacts (local)
Z:\RangeForgeData\        # SAN-mounted audio collections and large test files
```

- **Syncing options**: use `git push/pull` for code; use `robocopy` or dedicated sync tools for large non-repository audio files if necessary.

- **If you hit locks anyway**: capture process details and file handles (Windows Resource Monitor or Process Explorer) and avoid forcing deletions. Rebooting the machine that holds the lock is often the simplest recovery if the process cannot be identified.

Following these steps will minimize file-locking surprises while allowing shared data to remain on the SAN.
