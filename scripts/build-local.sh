#!/usr/bin/env bash
# Builds this machine's installer, the same steps the Release workflow runs.
#
#   scripts/build-local.sh             list what is needed, then build
#   scripts/build-local.sh --check     only list what is needed and what is missing
#   scripts/build-local.sh --skip-tests  leave out the (slow) export tests
#
# A program can only be built on its own kind of machine, so each machine builds one thing:
#   Windows x64 -> MSI;  Mac with Apple Silicon -> app and DMG;  Linux x64 (or WSL) -> .deb
# The result is copied to dist/<version>/. On Windows, run it from Git Bash, or double-click
# scripts\build-local.bat.
set -uo pipefail

cd "$(dirname "$0")/.."
root="$PWD"

skip_tests=0
check_only=0
choice=""
explicit=0
for arg in "$@"; do
  case "$arg" in
    --skip-tests) skip_tests=1; explicit=1 ;;
    --check) check_only=1; explicit=1 ;;
    win64 | macarm | linux) choice="$arg"; explicit=1 ;;
    -h | --help) sed -n 2,11p "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "Unknown option: $arg (try --help)" >&2; exit 2 ;;
  esac
done

host="other"
case "$(uname -s)-$(uname -m)" in
  MINGW* | MSYS* | CYGWIN*) host="windows" ;;
  Darwin-arm64) host="mac-arm" ;;
  Darwin-x86_64) host="mac-intel" ;;
  Linux-x86_64) host="linux" ;;
esac

# Build on this computer's own disk, never inside the repository: the repository may be on a
# network drive, where Windows refuses to replace the programs a build makes ("Access is
# denied") and everything is slow. Set CARGO_TARGET_DIR yourself to choose another place.
if [ -z "${CARGO_TARGET_DIR:-}" ]; then
  if [ "$host" = windows ]; then
    base="${LOCALAPPDATA:-$HOME/AppData/Local}/RangeForge"
  else
    base="${XDG_CACHE_HOME:-$HOME/.cache}/rangeforge"
  fi
  export CARGO_TARGET_DIR="$base/target"
fi

# Per target: what it is, the Rust target, the bundle kind, and which machine builds it.
target_info() {
  case "$1" in
    win64) name="Windows x64 (MSI)"; triple="x86_64-pc-windows-msvc"; bundles="msi"; builds_on="windows" ;;
    macarm) name="macOS Apple Silicon (app and DMG)"; triple="aarch64-apple-darwin"; bundles="app,dmg"; builds_on="mac-arm" ;;
    linux) name="Linux GNU x64 (deb)"; triple="x86_64-unknown-linux-gnu"; bundles="deb"; builds_on="linux" ;;
  esac
}

# A program can only be built on its own kind of machine, so this machine offers one target.
case "$host" in
  windows) own="win64" ;;
  mac-arm) own="macarm" ;;
  linux) own="linux" ;;
  *)
    echo "This script builds on Windows (x64), a Mac with Apple Silicon, or Linux (x64)." >&2
    echo "This machine is $(uname -s) $(uname -m)." >&2
    exit 1
    ;;
esac
if [ -n "$choice" ] && [ "$choice" != "$own" ]; then
  target_info "$choice"
  case "$builds_on" in
    windows) where="a Windows PC" ;;
    mac-arm) where="a Mac with Apple Silicon" ;;
    linux) where="a Linux machine (or WSL)" ;;
  esac
  echo "$name has to be built on $where; this machine builds $own." >&2
  exit 2
fi
choice="$own"
target_info "$choice"

have() { command -v "$1" > /dev/null 2>&1; }
has_vs_cpp() {
  "/c/Program Files (x86)/Microsoft Visual Studio/Installer/vswhere.exe" -latest -products "*" \
    -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 | grep -q installationPath
}

# Every requirement is listed with its state, so one run shows everything that is needed.
bad=0
req() { # req <what> <how to get it> <command...>
  local what="$1" fix="$2"; shift 2
  if "$@" > /dev/null 2>&1; then
    echo "  [ok]       $what"
  else
    echo "  [MISSING]  $what"
    echo "               -> $fix"
    bad=$((bad + 1))
  fi
}

show_requirements() {
  req "Rust (cargo)" "install rustup from https://rustup.rs" have cargo
  req "Rust target $triple" "run  rustup target add $triple" \
    bash -c "rustup target list --installed | grep -qx $triple"
  req "Tauri build tool" "run  cargo install tauri-cli --version \"^2\" --locked" \
    bash -c 'cargo tauri --version'
  req "git" "https://git-scm.com/downloads" have git
  req "curl (fetches FFmpeg)" "install curl" have curl
  case "$host" in
    windows)
      req "Visual Studio Build Tools, 'Desktop development with C++' workload" \
        "https://visualstudio.microsoft.com/visual-cpp-build-tools/" has_vs_cpp
      req "7-Zip or unzip (unpacks FFmpeg)" \
        "install 7-Zip from https://7-zip.org and add it to PATH (Git Bash has neither)" \
        bash -c 'command -v 7z || command -v unzip'
      ;;
    mac-arm)
      req "Xcode command line tools" "run  xcode-select --install" xcode-select -p
      req "make (FFmpeg is built from source)" "comes with the Xcode tools" have make
      ;;
    linux)
      req "Build tools" "sudo apt-get install -y build-essential pkg-config unzip" have make
      req "Window libraries" "sudo apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev libssl-dev" \
        pkg-config --exists webkit2gtk-4.1
      ;;
  esac
}

version="$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)"
echo "RangeForge $version: $name, on this machine ($host)."
echo "Building in $CARGO_TARGET_DIR"
echo
echo "What this needs:"
show_requirements
echo
if [ $bad -gt 0 ]; then
  echo "$bad missing. Install everything marked MISSING above, then run this again."
  exit 1
fi
echo "Everything needed is in place."
[ $check_only -eq 1 ] && exit 0

# Started with no options (double-clicked): let the person choose to skip the slow tests.
if [ "$explicit" -eq 0 ] && [ -t 0 ]; then
  echo
  read -r -p "Press Enter to build, s then Enter to build without the tests (faster), or q to quit: " answer
  case "$answer" in
    q | Q) exit 0 ;;
    s | S) skip_tests=1 ;;
  esac
fi

step() { echo; echo "== $*"; }
die() { echo; echo "Stopped: $*" >&2; exit 1; }

step "Getting FFmpeg"
bash scripts/fetch-ffmpeg.sh "$triple" crates/rangeforge-app/binaries || {
  echo
  echo "If the message above mentions CRYPT_E_NO_REVOCATION_CHECK, this network blocks Windows'"
  echo "certificate check. If you accept that for this download, run again with:"
  echo "    CURL_OPTIONS=--ssl-no-revoke scripts/build-local.sh $choice"
  echo "(or set CURL_OPTIONS=--ssl-no-revoke before double-clicking build-local.bat), or put an"
  echo "FFmpeg at crates/rangeforge-app/binaries/ffmpeg-$triple by hand."
  die "FFmpeg could not be fetched"
}

if [ $skip_tests -eq 0 ]; then
  step "Testing the export with the bundled FFmpeg"
  ext=""; [ "$host" = windows ] && ext=".exe"
  ffmpeg="$root/crates/rangeforge-app/binaries/ffmpeg-$triple$ext"
  have cygpath && ffmpeg="$(cygpath -w "$ffmpeg")"
  RANGEFORGE_FFMPEG="$ffmpeg" cargo test -p rangeforge-export -p rangeforge-session || die "the tests failed, so nothing was packaged"
fi

step "Building the installer"
started="$(mktemp)"
trap 'rm -f "$started"' EXIT
(cd crates/rangeforge-app && cargo tauri build --config tauri.bundle.conf.json --bundles "$bundles" --ci) \
  || die "the build failed"

step "Collecting the results"
# Cargo builds in CARGO_TARGET_DIR when it is set (the repository may be on a network share, where
# building is slow and unreliable), and otherwise in target/.
target_dir="${CARGO_TARGET_DIR:-target}"
have cygpath && target_dir="$(cygpath -u "$target_dir")"
out="dist/$version"
mkdir -p "$out"
# Only what this build just made: an older installer left in the folder must not be passed off
# as the new one.
for pattern in msi/*.msi dmg/*.dmg deb/*.deb; do
  for built in "$target_dir"/release/bundle/$pattern; do
    [ -f "$built" ] && [ "$built" -nt "$started" ] && cp "$built" "$out/"
  done
done
fresh="$(find "$out" -type f -newer "$started")"
[ -n "$fresh" ] || die "no new installer was produced (looked in $target_dir/release/bundle)"
ls -l "$out"
echo
echo "Done. The installer is in $root/$out"
