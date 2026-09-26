#!/usr/bin/env bash
# proof-smoke — `skies proof run` and `skies proof record` through the real binary on whatever OS runs it, with no app
# framework: a throwaway git repository under a path with a space, a runner that is a plain shell script writing
# JUnit, and a feature that is one line of a file. It exists for Windows, where the runner goes through Git's `sh`
# (the binary never falls back to cmd), so the shell path, the quoting of placeholders, and red's checkout are proven
# there too. Needs cargo and git (and, on Windows, Git for Windows). Set SKIES to reuse a prebuilt binary.
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fail() { echo "FAIL: $*" >&2; exit 1; }

if [ -z "${SKIES:-}" ]; then
  echo "==> building the skies binary"
  cargo build --quiet --manifest-path "$REPO/Cargo.toml" -p skies-cli
  SKIES="$REPO/target/debug/skies"
  [ -f "$SKIES.exe" ] && SKIES="$SKIES.exe"
fi

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
APP="$WORK/demo app"
mkdir -p "$APP" && cd "$APP"
git init --quiet --initial-branch=main
git config user.email "smoke@example.com"
git config user.name "proof smoke"

cat > Skies.toml <<'EOF'
[workspace]
name = "demo"

[runners.fake]
command = "sh run.sh {dir} {report}"
EOF
# One case per line of <e2e>/cases.txt, passing only when src/feature.txt says `on`.
cat > run.sh <<'EOF'
#!/bin/sh
state=$(head -n 1 src/feature.txt | tr -d '\r')
{
  echo '<testsuites><testsuite name="fake">'
  while IFS= read -r name; do
    name=$(printf '%s' "$name" | tr -d '\r')
    [ -z "$name" ] && continue
    if [ "$state" = on ]; then echo "<testcase name=\"$name\"/>"; else echo "<testcase name=\"$name\"><failure/></testcase>"; fi
  done < "$1/cases.txt"
  echo '</testsuite></testsuites>'
} > "$2"
EOF
mkdir -p src && echo off > src/feature.txt
git add . && git commit --quiet -m base
git checkout --quiet -b feature

echo "==> spec new, then the feature on a branch"
"$SKIES" spec new toggle >/dev/null || fail "spec new"
SPEC="$(ls -d .specs/0001-*)"
cat > "$SPEC/spec.md" <<'EOF'
---
id: "0001"
runner: fake
---
# Toggle

## Failure modes

- FM-1 The feature stays off. [avp: none]
- FM-2 The feature does not survive a second read. [avp: none]

## AVP exemptions
- FM-1 Synthetic file-toggle fixture: assertions directly inspect both reads; no domain protocol is involved. | reviewed-by: smoke-fixture
- FM-2 Synthetic file-toggle fixture: assertions directly inspect both reads; no domain protocol is involved. | reviewed-by: smoke-fixture
EOF
mkdir -p "$SPEC/e2e" && printf 'FM-1: turns on\nFM-2: stays on\n' > "$SPEC/e2e/cases.txt"
echo on > src/feature.txt
git add . && git commit --quiet -m "the toggle"

echo "==> proof run"
out="$("$SKIES" proof run 1 2>&1)" || { echo "$out"; fail "proof run"; }
echo "$out" | grep -q '2/2 FMs pass' || { echo "$out"; fail "proof run did not pass both modes"; }

echo "==> proof record (red on the merge-base, green on HEAD)"
out="$("$SKIES" proof record 1 2>&1)" || { echo "$out"; fail "proof record"; }
grep -q '"red": "fail"' "$SPEC/receipt.json" && grep -q '"green": "pass"' "$SPEC/receipt.json" \
  || { cat "$SPEC/receipt.json"; fail "the receipt does not prove red→green"; }
[ ! -e .skies-red ] || [ -z "$(ls -A .skies-red)" ] || fail "red's checkout was left behind"

echo "==> proof-smoke OK"
