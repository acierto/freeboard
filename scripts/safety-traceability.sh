#!/bin/sh
# Verifies docs/safety-traceability.md: every invariant 1-13 has at least one
# test, every listed test exists, and all listed tests pass.
set -eu
cd "$(dirname "$0")/.."
doc=docs/safety-traceability.md
manifest=src-tauri/Cargo.toml

list=$(cargo test --manifest-path "$manifest" --no-default-features --lib -- --list 2>/dev/null | sed -n 's/: test$//p')
status=0
for n in 1 2 3 4 5 6 7 8 9 10 11 13 12; do
  row=$(grep -E "^\| $n \|" "$doc" || true)
  tests=$(printf '%s\n' "$row" | grep -oE '`[a-z0-9_:]+`' | tr -d '`' || true)
  if [ -z "$tests" ]; then
    echo "invariant $n: no tests listed"; status=1; continue
  fi
  for t in $tests; do
    if ! printf '%s\n' "$list" | grep -qx "$t"; then
      echo "invariant $n: missing test $t"; status=1
    fi
  done
done
[ $status -eq 0 ] || exit $status

all=$(grep -oE '`[a-z0-9_]+::[a-z0-9_:]+`' "$doc" | tr -d '`' | sort -u)
failed=0
for t in $all; do
  if ! cargo test --manifest-path "$manifest" --no-default-features --lib -q -- --exact "$t" >/dev/null 2>&1; then
    echo "FAILED: $t"; failed=1
  fi
done
[ $failed -eq 0 ] && echo "All 13 invariants are covered by passing tests."
exit $failed
