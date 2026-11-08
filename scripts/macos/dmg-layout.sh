#!/usr/bin/env bash
# Give a DMG its window layout: background picture and icon positions.
#   scripts/macos/dmg-layout.sh <file.dmg>
#
# Tauri lays out the DMG window by scripting Finder, which does not work on
# CI runners (tauri-apps/tauri#1731): the released 1.5.0 DMG had the
# background file but no .DS_Store, so Finder showed neither the picture nor
# the icon positions. src-tauri/installer/dmg-DS_Store is the .DS_Store of a
# DMG built on a Mac with the same volume name (Freeboard), background
# (.background/dmg-background.tiff) and items (Freeboard.app, Applications).
# This copies it into the DMG and compresses the DMG again.
set -euo pipefail
dmg="$1"
here="$(cd "$(dirname "$0")" && pwd)"
layout="$here/../../src-tauri/installer/dmg-DS_Store"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
hdiutil convert -quiet "$dmg" -format UDRW -o "$work/rw.dmg"
mnt="$(hdiutil attach -nobrowse -noautoopen "$work/rw.dmg" | grep -o '/Volumes/.*' | head -1)"
[ -d "$mnt/.background" ] && [ -d "$mnt/Freeboard.app" ] || { echo "unexpected DMG contents in $mnt" >&2; hdiutil detach "$mnt" -quiet; exit 1; }
cp "$layout" "$mnt/.DS_Store"
hdiutil detach "$mnt" -quiet
rm -f "$dmg"
hdiutil convert -quiet "$work/rw.dmg" -format UDZO -imagekey zlib-level=9 -o "$dmg"
echo "layout added to $dmg"
