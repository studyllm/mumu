#!/usr/bin/env bash
# scripts/build-release.sh
#
# Wrapper for `npx tauri build` that renames the NSIS installer to an
# ASCII-safe filename after Tauri finishes.
#
# Why: GitHub Releases (via `gh release upload`) silently drops non-ASCII
# characters from asset filenames. Tauri derives the installer filename
# from `productName` ("沐目"), so the resulting file is named
# "沐目_0.1.0_x64-setup.exe" and uploads as "_0.1.0_x64-setup.exe"
# (the "沐目" prefix disappears). Renaming locally before upload gives
# users a clear filename on the Releases page.
#
# Usage: scripts/build-release.sh [extra args to pass to `tauri build`]
#
# Prerequisite: this must run on Windows (Git Bash / WSL) where `npx tauri
# build` produces the NSIS installer. macOS/Linux won't produce it.

set -euo pipefail

# Find the project root (script is in scripts/, so .. is the root)
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

echo "==> Building NSIS installer via tauri build"
npx tauri build "$@"

# Locate the NSIS output directory
NSIS_DIR="src-tauri/target/release/bundle/nsis"

if [ ! -d "$NSIS_DIR" ]; then
  echo "ERROR: NSIS output dir not found: $NSIS_DIR" >&2
  exit 1
fi

# Find the installer file (Tauri names it <productName>_<version>_<arch>-setup.exe)
shopt -s nullglob
INSTALLERS=("$NSIS_DIR"/*-setup.exe)

if [ "${#INSTALLERS[@]}" -eq 0 ]; then
  echo "ERROR: no *-setup.exe found in $NSIS_DIR" >&2
  exit 1
fi

if [ "${#INSTALLERS[@]}" -gt 1 ]; then
  echo "ERROR: multiple installers found, abort:" >&2
  printf '  %s\n' "${INSTALLERS[@]}" >&2
  exit 1
fi

SRC="${INSTALLERS[0]}"
FILENAME="$(basename "$SRC")"

# Detect if filename contains non-ASCII (the case we're fixing).
# Git Bash on Windows doesn't support [:ascii:] char class, so use a
# portable python check via the runtime — fallback to `od`.
is_ascii() {
  local f="$1"
  # od -c shows the byte representation; non-ASCII bytes have the high bit set
  if od -c -An "$f" 2>/dev/null | grep -q '\\3[0-3][0-7][0-7]'; then
    return 1  # has high-bit byte = non-ASCII present
  fi
  return 0  # all ASCII
}

if is_ascii "$FILENAME"; then
  echo "==> Installer filename already ASCII-safe: $FILENAME"
  echo "    (nothing to rename)"
  exit 0
fi

# Derive ASCII-safe name by stripping the non-ASCII productName prefix.
# e.g. "沐目_0.1.0_x64-setup.exe" -> "mumu_0.1.0_x64-setup.exe"
# Strategy: keep everything from the first underscore onward, prepend "mumu".
SUFFIX="${FILENAME#*_}"                # e.g. "0.1.0_x64-setup.exe"
ASCII_NAME="mumu_${SUFFIX}"
DST="${NSIS_DIR}/${ASCII_NAME}"

mv "$SRC" "$DST"

echo "==> Renamed installer:"
echo "    from: $FILENAME"
echo "    to:   $ASCII_NAME"
echo "    path: $DST"