#!/usr/bin/env bash
# Give the Linux package the apt name "freeboard-disk".
#   scripts/linux/rename-deb.sh <in.deb> <out.deb>
#
# Tauri names the .deb after the app ("freeboard"), but Ubuntu's archive has
# an unrelated "freeboard" package (an IoT dashboard, 1.1.0). Its higher
# version made `apt upgrade` replace Freeboard with it (found in the Linux test
# VM, 2026-11-07). The app, its binary and its menu entry keep the name
# Freeboard; only the package name changes. An install under the old package
# name (our 0.x "freeboard") is removed in favour of this one. The conflict
# has no version bound on purpose: with "<< 1.0", apt satisfied it by
# upgrading the old package to the IoT dashboard instead of removing it.
set -euo pipefail
in="$1"; out="$2"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
dpkg-deb -R "$in" "$work/pkg"
ctl="$work/pkg/DEBIAN/control"
sed -i 's/^Package: freeboard$/Package: freeboard-disk/' "$ctl"
grep -q '^Package: freeboard-disk$' "$ctl" || { echo "rename-deb: unexpected control file" >&2; cat "$ctl" >&2; exit 1; }
{
  echo 'Conflicts: freeboard'
  echo 'Replaces: freeboard'
} >> "$ctl"
dpkg-deb --root-owner-group -b "$work/pkg" "$out" >/dev/null
dpkg-deb -f "$out" Package Version Conflicts Replaces
