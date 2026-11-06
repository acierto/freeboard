#!/usr/bin/env bash
# Disposable test data for the real-app click-through (scripts/native/e2e.mjs).
# Creates ~/fbtest inside the TEST MACHINE ONLY (the Linux or Windows VM):
# Git repositories with inactive Node projects, a Python .venv and Rust build
# output, all dated 60 days ago so Freeboard suggests them. Never run this on
# a machine whose files matter: the click-through may clean these folders.
set -euo pipefail
root="${1:-$HOME/fbtest}"
rm -rf "$root"
mkdir -p "$root"
blob() { head -c "$2" /dev/zero > "$1"; }
for p in web-app admin-portal docs-site storybook; do
  d="$root/$p"
  mkdir -p "$d/node_modules/pkg" "$d/src"
  echo '{"name":"'"$p"'"}' > "$d/package.json"
  echo 'lockfileVersion: 9' > "$d/pnpm-lock.yaml"
  echo 'export {}' > "$d/src/index.ts"
  # 150-400 MB each, so one project changes the size shown to one decimal of a GB.
  blob "$d/node_modules/pkg/big.bin" $(( (RANDOM % 250 + 150) * 1048576 ))
  git -C "$d" init -q
done
mkdir -p "$root/data-tools/.venv/lib" && echo requests > "$root/data-tools/requirements.txt"
blob "$root/data-tools/.venv/lib/site.bin" $((30 * 1048576))
git -C "$root/data-tools" init -q
mkdir -p "$root/rust-cli/target/debug" "$root/rust-cli/src" && echo '[package]' > "$root/rust-cli/Cargo.toml"
echo 'fn main() {}' > "$root/rust-cli/src/main.rs"
blob "$root/rust-cli/target/debug/app" $((60 * 1048576))
git -C "$root/rust-cli" init -q
# Inactive for 60 days (the default threshold is 30).
find "$root" -exec touch -h -d "60 days ago" {} +
echo "fixtures in $root"
