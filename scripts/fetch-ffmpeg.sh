#!/usr/bin/env bash
# Puts an FFmpeg where Tauri's bundler expects a bundled program:
#
#   <out-dir>/ffmpeg-<rust target triple>[.exe]
#
# Usage: scripts/fetch-ffmpeg.sh <rust-target-triple> <out-dir>
#
# It is always an LGPL build (no GPL or non-free parts), so it can be shipped inside the
# app. RangeForge needs FFmpeg's own ALAC, FLAC, AAC and PCM encoders, and LAME (MP3) and
# libopus (Opus) for those two formats.
#
#  - Windows: the prebuilt LGPL static build from BtbN, for FFmpeg 9.0 (it includes LAME,
#    libopus and libsoxr).
#  - macOS and Linux: built from the FFmpeg source at the tag below, together with LAME and
#    libopus built from the source tarballs below (checked by hash), all linked statically, so
#    the result runs on a machine that has nothing else installed. Needs a C compiler, make
#    and pkg-config.
#
# Running it again does nothing if the file is already there.
set -euo pipefail

triple="${1:?usage: fetch-ffmpeg.sh <rust-target-triple> <out-dir>}"
out="${2:?usage: fetch-ffmpeg.sh <rust-target-triple> <out-dir>}"

# The source release to build, pinned by tag and by the commit the tag must point at.
TAG=n9.0.2
COMMIT=946fcce07b6dcd0331c8cc609192aeff5e1924f8
BTBN=https://github.com/BtbN/FFmpeg-Builds/releases/download/latest
# The MP3 and Opus encoders for the builds from source, pinned by hash.
LAME_URL=https://downloads.sourceforge.net/project/lame/lame/3.100/lame-3.100.tar.gz
LAME_SHA256=ddfe36cab873794038ae2c1210557ad34857a4b6bdc515785d1da9e175b1da1e
OPUS_URL=https://downloads.xiph.org/releases/opus/opus-1.5.2.tar.gz
OPUS_SHA256=65c1d2f78b9f2fb20082c38cbe47c951ad5839345876e46941612ee87f9a7ce1

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
    command -v pkg-config > /dev/null 2>&1 || { echo "pkg-config is needed to build FFmpeg" >&2; exit 1; }
    jobs="$(nproc 2> /dev/null || sysctl -n hw.ncpu 2> /dev/null || echo 2)"
    case "$triple" in
      *-apple-darwin)
        export MACOSX_DEPLOYMENT_TARGET=11.0
        export CFLAGS="-mmacosx-version-min=11.0" LDFLAGS="-mmacosx-version-min=11.0"
        ;;
    esac
    prefix="$work/deps"
    mkdir -p "$prefix"
    # Checks a download against its pinned hash (sha256sum on Linux, shasum on a Mac).
    verify() {
      local want="$1" file="$2" got
      if command -v sha256sum > /dev/null 2>&1; then got="$(sha256sum "$file" | cut -d' ' -f1)"
      else got="$(shasum -a 256 "$file" | cut -d' ' -f1)"; fi
      [ "$got" = "$want" ] || { echo "$file has hash $got, expected $want" >&2; exit 1; }
    }
    echo "Building LAME and Opus (static)"
    curl -fL --retry 3 ${CURL_OPTIONS:-} -o "$work/lame.tar.gz" "$LAME_URL"
    verify "$LAME_SHA256" "$work/lame.tar.gz"
    curl -fL --retry 3 ${CURL_OPTIONS:-} -o "$work/opus.tar.gz" "$OPUS_URL"
    verify "$OPUS_SHA256" "$work/opus.tar.gz"
    tar -xzf "$work/lame.tar.gz" -C "$work"
    ( cd "$work/lame-3.100"       && ./configure --prefix="$prefix" --disable-shared --enable-static --disable-frontend --disable-decoder --disable-gtktest       && make -j"$jobs" && make install ) > /dev/null
    tar -xzf "$work/opus.tar.gz" -C "$work"
    ( cd "$work/opus-1.5.2"       && ./configure --prefix="$prefix" --disable-shared --enable-static --disable-doc --disable-extra-programs       && make -j"$jobs" && make install ) > /dev/null
    export PKG_CONFIG_PATH="$prefix/lib/pkgconfig"

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
      --enable-libmp3lame --enable-libopus
      "--extra-cflags=-I$prefix/include" "--extra-ldflags=-L$prefix/lib"
    )
    case "$triple" in
      *-apple-darwin)
        flags+=(--extra-cflags=-mmacosx-version-min=11.0 --extra-ldflags=-mmacosx-version-min=11.0)
        ;;
    esac
    ./configure "${flags[@]}"
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
for name in alac flac aac pcm_s16le pcm_s24le pcm_f32le libmp3lame libopus; do
  if ! echo "$encoders" | grep -qw "$name"; then
    echo "This FFmpeg cannot encode $name" >&2
    rm -f "$target"
    exit 1
  fi
done
echo "Ready: $target"
