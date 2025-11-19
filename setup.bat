@echo off
setlocal

echo ============================================
echo RangeForge Development Environment Setup
echo ============================================
echo.

echo [1/6] Checking environment...

REM Check Node.js
where node >nul 2>&1
if errorlevel 1 (
    echo ERROR: Node.js not found! Please install Node.js first.
    pause
    exit /b 1
)

REM Check npm
where npm >nul 2>&1
if errorlevel 1 (
    echo ERROR: npm not found! Please install npm first.
    pause
    exit /b 1
)

echo Node.js and npm found successfully.

REM Get versions
echo Getting versions...
node --version
npm --version
echo.

echo [2/6] Cleaning up existing files...
if exist node_modules (
    echo Removing node_modules directory...
    rmdir /s /q node_modules
    echo Done.
)

if exist package-lock.json (
    echo Removing package-lock.json...
    del package-lock.json
    echo Done.
)

if exist npm-shrinkwrap.json (
    echo Removing npm-shrinkwrap.json...
    del npm-shrinkwrap.json
    echo Done.
)

echo [3/6] Cleaning npm cache...
echo Running: npm cache clean --force
npm cache clean --force

echo [4/6] Cleaning Electron cache...
if exist "%APPDATA%\Electron" (
    echo Removing Electron cache...
    rmdir /s /q "%APPDATA%\Electron" 2>nul
    echo Done.
) else (
    echo No Electron cache found.
)

echo [5/6] Setting environment variables for native module building...
set npm_config_target_platform=win32
set npm_config_target_arch=x64
set npm_config_runtime=electron
echo Environment variables set.

echo [6/6] Installing dependencies...
echo This may take a few minutes...
echo.
echo Running: npm install
npm install

echo.
echo ============================================
echo Installation process completed.
echo Check the messages above for any errors.
echo ============================================
echo.
echo You can now run:
echo   npm run dev     - Start development servers
echo   npm run build   - Build for production
echo.
pause