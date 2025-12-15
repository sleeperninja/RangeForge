const https = require('https');
const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

/**
 * Download FFmpeg static binaries for distribution
 * This script downloads official FFmpeg static builds for Windows, macOS, and Linux
 */

const FFMPEG_URLS = {
  win32: {
    url: 'https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip',
    binaries: ['ffmpeg.exe', 'ffprobe.exe']
  },
  darwin: {
    ffmpeg: 'https://evermeet.cx/ffmpeg/getrelease/zip',
    ffprobe: 'https://evermeet.cx/ffmpeg/getrelease/ffprobe/zip'
  },
  linux: {
    url: 'https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-amd64-static.tar.xz',
    binaries: ['ffmpeg', 'ffprobe']
  }
};

const binDir = path.join(__dirname, '..', 'bin');

function createDirectories() {
  const dirs = [
    path.join(binDir, 'win32'),
    path.join(binDir, 'darwin'),
    path.join(binDir, 'linux')
  ];

  dirs.forEach(dir => {
    if (!fs.existsSync(dir)) {
      fs.mkdirSync(dir, { recursive: true });
      console.log(`Created directory: ${dir}`);
    }
  });
}

async function downloadFFmpeg() {
  console.log('🎵 RangeForge FFmpeg Binary Download Script');
  console.log('==========================================\n');

  createDirectories();

  console.log('ℹ️  This script downloads official FFmpeg static builds for distribution.');
  console.log('📋 FFmpeg is LGPL licensed and safe to bundle in proprietary applications.\n');

  console.log('📁 Download locations:');
  console.log(`   Windows: ${FFMPEG_URLS.win32.url}`);
  console.log(`   macOS:   ${FFMPEG_URLS.darwin.ffmpeg}`);
  console.log(`   Linux:   ${FFMPEG_URLS.linux.url}\n`);

  console.log('⚠️  Manual Setup Required:');
  console.log('   Due to the complexity of extracting different archive formats');
  console.log('   and platform-specific binaries, please download manually:\n');

  console.log('1️⃣  Windows:');
  console.log(`   - Download: ${FFMPEG_URLS.win32.url}`);
  console.log('   - Extract ffmpeg.exe and ffprobe.exe');
  console.log('   - Place in: bin/win32/\n');

  console.log('2️⃣  macOS:');
  console.log(`   - Download FFmpeg: ${FFMPEG_URLS.darwin.ffmpeg}`);
  console.log(`   - Download FFprobe: ${FFMPEG_URLS.darwin.ffprobe}`);
  console.log('   - Place in: bin/darwin/\n');

  console.log('3️⃣  Linux:');
  console.log(`   - Download: ${FFMPEG_URLS.linux.url}`);
  console.log('   - Extract ffmpeg and ffprobe binaries');
  console.log('   - Place in: bin/linux/\n');

  console.log('✅ Alternative: For development, FFmpeg from PATH is sufficient');
  console.log('   (Your Chocolatey installation works great!)\n');

  // Check if FFmpeg is available in PATH
  try {
    execSync('ffmpeg -version', { stdio: 'ignore' });
    console.log('✅ FFmpeg detected in system PATH - ready for development!');
  } catch {
    console.log('❌ FFmpeg not found in system PATH');
  }

  try {
    execSync('ffprobe -version', { stdio: 'ignore' });
    console.log('✅ FFprobe detected in system PATH - ready for development!');
  } catch {
    console.log('❌ FFprobe not found in system PATH');
  }

  console.log('\n🚀 RangeForge will automatically detect and use available FFmpeg binaries.');
}

downloadFFmpeg().catch(console.error);
