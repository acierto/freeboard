#!/usr/bin/env bash
# Headless Linux verification. Run from the repository root:
#   scripts/linux/verify.sh            (uses Docker, Ubuntu 22.04)
# Copies the working tree into the container (no host node_modules/target),
# then: core tests, desktop-feature tests, frontend build, .deb + AppImage.
# Writes logs and installers to linux-evidence/.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
OUT="$ROOT/linux-evidence"
mkdir -p "$OUT"
docker build -t freeboard-linux-verify "$ROOT/scripts/linux"
docker volume create freeboard-linux-cargo >/dev/null
docker volume create freeboard-linux-target >/dev/null
docker run --rm \
  -v "$ROOT":/src:ro \
  -v "$OUT":/out \
  -v freeboard-linux-cargo:/root/.cargo/registry \
  -v freeboard-linux-target:/work/src-tauri/target \
  freeboard-linux-verify bash -c '
set -uo pipefail
cd /src && tar --exclude=./node_modules --exclude=./src-tauri/target --exclude=./dist --exclude=./dist-visual \
  --exclude=./linux-evidence --exclude=./visual/actual --exclude=./visual/diff --exclude=./visual/report -cf - . | (cd /work && tar -xf -)
cd /work
uname -a > /out/system.txt; cat /etc/os-release >> /out/system.txt; rustc --version >> /out/system.txt; node --version >> /out/system.txt
status=0
step() { name=$1; shift; echo "== $name"; if "$@" > "/out/$name.log" 2>&1; then echo "PASS $name" | tee -a /out/summary.txt; else echo "FAIL $name" | tee -a /out/summary.txt; status=1; fi; }
rm -f /out/summary.txt
step npm-ci npm ci
step frontend-build npm run build
step core-tests bash -c "cd src-tauri && cargo test --no-default-features --locked"
step desktop-tests bash -c "cd src-tauri && cargo test --locked"
step bundles npx tauri build --bundles deb,appimage
find src-tauri/target/release/bundle \( -name "*.deb" -o -name "*.AppImage" \) -exec cp {} /out/ \; 2>/dev/null
node scripts/perf/installer-size.mjs /out >> /out/summary.txt || true
exit $status
'
