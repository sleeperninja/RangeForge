#!/usr/bin/env bash
# Builds the installers on this machine, the same steps the Release workflow runs.
#
#   scripts/build-local.sh             choose from a menu
#   scripts/build-local.sh win64       Windows x64 MSI           (build on Windows)
#   scripts/build-local.sh macarm      macOS Apple Silicon DMG   (build on a Mac with an M1 or later)
#   scripts/build-local.sh linux       Linux x64 .deb            (build on Linux or in WSL)
#   scripts/build-local.sh --check     only report what is missing for each target
#
# Add --skip-tests to leave out the export tests. The results are copied to dist/<version>/.
#
# A program can only be built on its own kind of machine: there is no cross-building here.
# On Windows, run it from Git Bash, or double-click scripts\build-local.bat.
set -uo pipefail

cd "$(dirname "$0")/.."
root="$PWD"

skip_tests=0
check_only=0
choice=""
for arg in "$@"; do
  case "$arg" in
    --skip-tests) skip_tests=1 ;;
    --check) check_only=1 ;;
    win64 | macarm | linux) choice="$arg" ;;
    -h | --help) sed -n 2,13p "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
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

# Per target: what it is, the Rust target, the bundle kind, and where it can be built.
target_info() {
  case "$1" in
    win64) name="Windows x64 (MSI)"; triple="x86_64-pc-windows-msvc"; bundles="msi"; builds_on="windows" ;;
    macarm) name="macOS Apple Silicon (app and DMG)"; triple="aarch64-apple-darwin"; bundles="app,dmg"; builds_on="mac-arm" ;;
    linux) name="Linux GNU x64 (deb)"; triple="x86_64-unknown-linux-gnu"; bundles="deb"; builds_on="linux" ;;
  esac
}

have() { command -v "$1" > /dev/null 2>&1; }
has_vs_cpp() {
  "/c/Program Files (x86)/Microsoft Visual Studio/Installer/vswhere.exe" -latest -products "*" \n    -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 | grep -q installationPath
}
missing=()
need() { # need <description> <command...>: records a missing item and its fix
  local what="$1"; shift
  if ! "$@" > /dev/null 2>&1; then missing+=("$what"); return 1; fi
}

# Fills `missing` with plain-words fixes for one target.
check_target() {
  missing=()
  target_info "$1"
  if [ "$host" != "$builds_on" ]; then
    case "$builds_on" in
      windows) missing+=("This one has to be built on Windows.") ;;
      mac-arm) missing+=("This one has to be built on a Mac with Apple Silicon (the M1 mini).") ;;
      linux) missing+=("This one has to be built on Linux. On Windows, open WSL and run this script there.") ;;
    esac
    return
  fi
  need "Rust: install rustup from https://rustup.rs (it brings cargo)" have cargo
  have cargo && ! need "Rust target: run  rustup target add $triple" \
    bash -c "rustup target list --installed | grep -qx $triple"
  need "git: https://git-scm.com/downloads" have git
  need "curl" have curl
  case "$builds_on" in
    windows)
      need "Visual Studio Build Tools with the 'Desktop development with C++' workload: https://visualstudio.microsoft.com/visual-cpp-build-tools/" has_vs_cpp
      need "unzip or 7-Zip (Git Bash usually has neither: install 7-Zip from https://7-zip.org and add it to PATH)" bash -c 'command -v 7z || command -v unzip'
      ;;
    mac-arm)
      need "Xcode command line tools: run  xcode-select --install" xcode-select -p
      need "FFmpeg is built from source: needs make (comes with the Xcode tools)" have make
      ;;
    linux)
      need "Build tools: sudo apt-get install -y build-essential pkg-config unzip" have make
      need "Window libraries: sudo apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev libssl-dev" \
        pkg-config --exists webkit2gtk-4.1
      ;;
  esac
  have cargo && ! need "Tauri build tool: run  cargo install tauri-cli --version \"^2\" --locked" \
    bash -c 'cargo tauri --version'
}

report() { # report <key>: prints status, returns 0 if ready
  check_target "$1"
  local real=("${missing[@]}")
  target_info "$1"
  if [ ${#real[@]} -eq 0 ]; then
    echo "  [ready]  $1  $name"
    return 0
  fi
  echo "  [needs]  $1  $name"
  for m in "${real[@]}"; do echo "             - $m"; done
  return 1
}

if [ $check_only -eq 1 ]; then
  echo "This machine: $host"
  for k in win64 macarm linux; do report "$k"; done
  exit 0
fi

if [ -z "$choice" ]; then
  echo "Build RangeForge installers on this machine ($host)."
  echo
  for k in win64 macarm linux; do report "$k"; done
  echo
  read -r -p "Which one? (win64 / macarm / linux, or Enter to quit) " choice
  [ -z "$choice" ] && exit 0
fi

case "$choice" in win64 | macarm | linux) ;; *) echo "Not a choice: $choice" >&2; exit 2 ;; esac
if ! report "$choice" > /dev/null; then
  echo
  report "$choice"
  echo
  echo "Install the above, then run this again."
  exit 1
fi
target_info "$choice"

version="$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)"
echo
echo "== Building $name, version $version"

step() { echo; echo "== $*"; }
die() { echo; echo "Stopped: $*" >&2; exit 1; }

step "Getting FFmpeg"
bash scripts/fetch-ffmpeg.sh "$triple" crates/rangeforge-app/binaries || die "FFmpeg could not be fetched"

if [ $skip_tests -eq 0 ]; then
  step "Testing the export with the bundled FFmpeg"
  ext=""; [ "$host" = windows ] && ext=".exe"
  ffmpeg="$root/crates/rangeforge-app/binaries/ffmpeg-$triple$ext"
  have cygpath && ffmpeg="$(cygpath -w "$ffmpeg")"
  RANGEFORGE_FFMPEG="$ffmpeg" cargo test -p rangeforge-export -p rangeforge-session || die "the tests failed, so nothing was packaged"
fi

step "Building the installer"
(cd crates/rangeforge-app && cargo tauri build --config tauri.bundle.conf.json --bundles "$bundles" --ci) \
  || die "the build failed"

step "Collecting the results"
out="dist/$version"
mkdir -p "$out"
for pattern in msi/*.msi dmg/*.dmg deb/*.deb; do
  cp target/release/bundle/$pattern "$out/" 2> /dev/null || true
done
[ -n "$(ls -A "$out")" ] || die "no installer was produced"
ls -l "$out"
echo
echo "Done. The installer is in $root/$out"
