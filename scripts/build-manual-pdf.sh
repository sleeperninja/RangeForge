#!/usr/bin/env bash
# Builds one PDF from the three documents people read: the manual, the FAQ and the third-party
# notices. The release workflow runs this and attaches the result to the release.
#
# Usage: scripts/build-manual-pdf.sh <output.pdf>
#
# Needs: pandoc (3.x), and a Chrome or Edge to print the page to PDF. On GitHub's Ubuntu
# runners Chrome is already installed. Set CHROME to use a particular browser.
set -euo pipefail

out="${1:?usage: build-manual-pdf.sh <output.pdf>}"
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

command -v pandoc > /dev/null 2>&1 || { echo "pandoc is needed to build the manual PDF" >&2; exit 1; }

# Find a browser that can print a page to PDF.
chrome="${CHROME:-}"
if [ -z "$chrome" ]; then
  for candidate in google-chrome google-chrome-stable chromium chromium-browser chrome msedge \
    "/c/Program Files/Google/Chrome/Application/chrome.exe" \
    "/c/Program Files (x86)/Microsoft/Edge/Application/msedge.exe" \
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"; do
    if command -v "$candidate" > /dev/null 2>&1 || [ -x "$candidate" ]; then
      chrome="$candidate"
      break
    fi
  done
fi
[ -n "$chrome" ] || { echo "No Chrome, Chromium or Edge found; set CHROME" >&2; exit 1; }

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

version="$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)"

# A marker left in a document means the writing is not finished. Say so, but still build.
if grep -n ">>> ATTENTION" MANUAL.md FAQ.md THIRD_PARTY_NOTICES.md > /dev/null 2>&1; then
  count="$(grep -c ">>> ATTENTION" MANUAL.md FAQ.md THIRD_PARTY_NOTICES.md | awk -F: '{n += $2} END {print n}')"
  echo "::warning::$count '>>> ATTENTION' note(s) are still in the documents and will be in the PDF"
fi

# One document, in reading order. Links between the three become links inside the PDF.
{
  cat MANUAL.md
  printf '\n\n'
  cat FAQ.md
  printf '\n\n'
  cat THIRD_PARTY_NOTICES.md
} | sed \
  -e 's#](MANUAL.md)#](\#rangeforge-manual)#g' \
  -e 's#](FAQ.md)#](\#frequently-asked-questions)#g' \
  -e 's#](THIRD_PARTY_NOTICES.md)#](\#third-party-notices)#g' \
  -e 's#](ROADMAP.md)#](https://github.com/sleeperninja/RangeForge/blob/main/ROADMAP.md)#g' \
  > "$work/combined.md"

# Pictures and the style sheet are put inside the page, so the browser needs nothing else.
pandoc "$work/combined.md" \
  --from gfm+smart --to html5 --standalone --embed-resources \
  --resource-path="$root" --css "$root/scripts/manual.css" \
  --toc --toc-depth=2 \
  --metadata title="RangeForge" \
  --metadata subtitle="Manual, FAQ and notices, version $version" \
  --metadata lang=en \
  -o "$work/manual.html"

# Windows browsers need a Windows path; the others take the path as it is.
page="$work/manual.html"
target="$out"
mkdir -p "$(dirname "$target")"
target="$(cd "$(dirname "$target")" && pwd)/$(basename "$target")"
if command -v cygpath > /dev/null 2>&1; then
  page="$(cygpath -m "$page")"
  target="$(cygpath -m "$target")"
fi

# Its own profile folder keeps it from handing the job to a browser that is already open.
profile="$work/profile"
if command -v cygpath > /dev/null 2>&1; then profile="$(cygpath -m "$profile")"; fi
extra=()
[ -n "${CI:-}" ] && extra+=(--no-sandbox)
"$chrome" --headless=new --disable-gpu "${extra[@]}" --user-data-dir="$profile" --no-pdf-header-footer \
  --print-to-pdf="$target" "file:///${page#/}" > "$work/browser.log" 2>&1 || {
  cat "$work/browser.log" >&2
  echo "The browser could not print the PDF" >&2
  exit 1
}

# Some browsers (Edge on Windows) return before the file is finished, so wait for it.
for _ in $(seq 1 60); do
  if [ -s "$out" ]; then sleep 1; break; fi
  sleep 1
done
[ -s "$out" ] || { cat "$work/browser.log" >&2; echo "No PDF was written" >&2; exit 1; }
echo "Wrote $out ($(wc -c < "$out") bytes)"
