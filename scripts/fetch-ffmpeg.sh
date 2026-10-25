#!/usr/bin/env bash
# Puts an FFmpeg where Tauri's bundler expects a bundled program:
#
#   <out-dir>/ffmpeg-<rust target triple>[.exe]
#
# Usage: scripts/fetch-ffmpeg.sh <rust-target-triple> <out-dir>
#
# It is always an LGPL build (no GPL or non-free parts), so it can be shipped inside the
# app. RangeForge only needs FFmpeg's own ALAC, FLAC, AAC and PCM encoders.
#
#  - Windows: the prebuilt LGPL static build from BtbN, for FFmpeg 9.0.
#  - macOS and Linux: built from the FFmpeg source at the tag below, with no outside
#    libraries, so the result runs on a machine that has nothing else installed.
#
# Running it again does nothing if the file is already there.
set -euo pipefail

triple="${1:?usage: fetch-ffmpeg.sh <rust-target-triple> <out-dir>}"
out="${2:?usage: fetch-ffmpeg.sh <rust-target-triple> <out-dir>}"

# The source release to build, pinned by tag and by the commit the tag must point at.
TAG=n9.0.2
COMMIT=946fcce07b6dcd0331c8cc609192aeff5e1924f8
BTBN=https://github.com/BtbN/FFmpeg-Builds/releases/download/latest

mkdir -p "$out"
out="$(cd "$out" && pwd)"

suffix=""
case "$triple" in *windows*) suffix=".exe" ;; esac
target="$out/ffmpeg-$triple$suffix"
if [ -f "$target" ]; then
  echo "Already there: $target"
  exit 0
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

case "$triple" in
  x86_64-pc-windows-msvc | aarch64-pc-windows-msvc)
    arch=win64
    [ "$triple" = aarch64-pc-windows-msvc ] && arch=winarm64
    asset="ffmpeg-n9.0-latest-$arch-lgpl-9.0.zip"
    echo "Downloading $asset"
    # CURL_OPTIONS lets a network that blocks revocation checks add --ssl-no-revoke.
    # shellcheck disable=SC2086
    curl -fL --retry 3 ${CURL_OPTIONS:-} -o "$work/ffmpeg.zip" "$BTBN/$asset"
    if command -v 7z > /dev/null 2>&1; then
      7z x -y "-o$work/unpacked" "$work/ffmpeg.zip" > /dev/null
    else
      unzip -q "$work/ffmpeg.zip" -d "$work/unpacked"
    fi
    exe="$(find "$work/unpacked" -type f -name ffmpeg.exe | head -n 1)"
    if [ -z "$exe" ]; then
      echo "ffmpeg.exe is not in $asset" >&2
      exit 1
    fi
    cp "$exe" "$target"
    ;;

  x86_64-apple-darwin | aarch64-apple-darwin | x86_64-unknown-linux-gnu)
    echo "Building FFmpeg $TAG from source"
    git clone --quiet --depth 1 --branch "$TAG" https://github.com/FFmpeg/FFmpeg.git "$work/src"
    actual="$(git -C "$work/src" rev-parse HEAD)"
    if [ "$actual" != "$COMMIT" ]; then
      echo "FFmpeg $TAG is at $actual, but $COMMIT was expected" >&2
      exit 1
    fi
    cd "$work/src"
    flags=(
      --disable-gpl --disable-nonfree --disable-autodetect
      --disable-doc --disable-network --disable-debug
      --disable-ffplay --disable-ffprobe --disable-avdevice
      --disable-shared --enable-static --disable-x86asm
    )
    case "$triple" in
      *-apple-darwin)
        export MACOSX_DEPLOYMENT_TARGET=11.0
        flags+=(--extra-cflags=-mmacosx-version-min=11.0 --extra-ldflags=-mmacosx-version-min=11.0)
        ;;
    esac
    ./configure "${flags[@]}"
    jobs="$(nproc 2> /dev/null || sysctl -n hw.ncpu 2> /dev/null || echo 2)"
    make -j"$jobs"
    case "$triple" in
      *-apple-darwin) strip -x ffmpeg ;;
      *) strip ffmpeg ;;
    esac
    cp ffmpeg "$target"
    ;;

  *)
    echo "No FFmpeg recipe for $triple" >&2
    exit 1
    ;;
esac

chmod +x "$target"

# It must run, must be LGPL, and must be able to write the formats RangeForge writes.
version="$("$target" -version)"
echo "$version" | head -n 1
if echo "$version" | grep -q -- '--enable-gpl'; then
  echo "This FFmpeg is a GPL build and cannot be shipped in the app" >&2
  rm -f "$target"
  exit 1
fi
encoders="$("$target" -hide_banner -encoders)"
for name in alac flac aac pcm_s16le pcm_s24le; do
  if ! echo "$encoders" | grep -qw "$name"; then
    echo "This FFmpeg cannot encode $name" >&2
    rm -f "$target"
    exit 1
  fi
done
echo "Ready: $target"
