# Third-party notices

RangeForge's installers include these programs and libraries.

## FFmpeg

RangeForge uses FFmpeg to write ALAC, FLAC, WAV and AAC files. The copy inside the app is an
**LGPL** build: it contains no GPL or non-free parts, and it is a separate program that
RangeForge starts, not code linked into RangeForge.

- Project: <https://ffmpeg.org>
- Licence: GNU Lesser General Public License, version 2.1 or later,
  <https://www.gnu.org/licenses/old-licenses/lgpl-2.1.html>
- Version: 9.0.2 (source tag `n9.0.2`, <https://github.com/FFmpeg/FFmpeg>).
  - macOS and Linux: built from that source by `scripts/fetch-ffmpeg.sh` in this repository,
    with `--disable-gpl --disable-nonfree --disable-autodetect`, which also means it uses no
    outside libraries. The exact build steps are in that script.
  - Windows: the LGPL build from <https://github.com/BtbN/FFmpeg-Builds>.
- The FFmpeg source for the version shipped is at the tag above. You can replace the bundled
  `ffmpeg` file with a build of your own: RangeForge also uses the `RANGEFORGE_FFMPEG`
  environment variable, or an `ffmpeg` on the system path, if the bundled one is missing.

FFmpeg is a trademark of Fabrice Bellard, originator of the FFmpeg project.

## Rust libraries and the web view

RangeForge is built from Rust libraries (Tauri, Symphonia, ebur128 and others) under MIT,
Apache-2.0 and similar permissive licences. The window uses the web view that comes with the
operating system (WebView2 on Windows, WebKit on macOS and Linux).
