#!/usr/bin/env bash
# Proves the release plumbing without publishing anything:
#   1. the npm wrapper's tarball carries its executable launcher (`bin/skies.js`, the package's `bin`);
#   2. given a built binary, the launcher finds it in the platform package pack-platform.mjs builds (the layout npm
#      installs from optionalDependencies) and runs it, and a missing platform package is a clear exit 2;
#   3. tools/set-version.sh, run on a copy of the repository, moves every pin tools/check-versions.sh knows and leaves
#      the old version in no other file (the changelog and the generator snapshots excepted: re-blessing moves those).
# Usage: tools/release-check.sh [path/to/built/skies]
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
binary="${1:-}"
work="$(mktemp -d "${TMPDIR:-/tmp}/skies-release-check.XXXXXX")"
trap 'rm -rf "$work"' EXIT

echo "== the npm wrapper packs its launcher"
(cd "$root/cli/npm" && npm pack --dry-run --json --silent) >"$work/pack.json"
node - "$work/pack.json" "$root/cli/npm/package.json" <<'JS'
const fs = require("node:fs");
const [pack, manifest] = process.argv.slice(2).map((file) => JSON.parse(fs.readFileSync(file, "utf8")));
const launcher = manifest.bin?.skies;
const file = pack[0].files.find((f) => f.path === launcher);
if (!launcher || !file) {
  console.error(`the tarball has no bin.skies (${launcher}); it ships: ${pack[0].files.map((f) => f.path).join(", ")}`);
  process.exit(1);
}
if ((file.mode & 0o111) === 0) {
  console.error(`${launcher} is packed without its executable bit (mode ${file.mode.toString(8)})`);
  process.exit(1);
}
console.log(`${launcher} is in the tarball, mode ${file.mode.toString(8)}`);
JS

if [ -n "$binary" ]; then
  echo "== the launcher runs the platform package's binary"
  platform=$(node -p 'process.platform'); arch=$(node -p 'process.arch')
  modules="$work/install/node_modules/@skiesjs"
  mkdir -p "$modules/cli/bin"
  cp "$root/cli/npm/package.json" "$modules/cli/"
  cp "$root/cli/npm/bin/skies.js" "$modules/cli/bin/"
  node "$root/cli/npm/pack-platform.mjs" "$platform" "$arch" "$binary" "$modules/cli-$platform-$arch"
  want="skies $(sed -n 's/^version = "\(.*\)"/\1/p' "$root/Cargo.toml" | head -1)"
  have=$(node "$modules/cli/bin/skies.js" --version)
  [ "$have" = "$want" ] || { echo "the launcher printed '$have', want '$want'"; exit 1; }
  echo "npx skies --version: $have"
  rm -rf "$modules/cli-$platform-$arch"
  status=0; node "$modules/cli/bin/skies.js" --version 2>"$work/missing.err" || status=$?
  [ "$status" = 2 ] && grep -q "is not installed" "$work/missing.err" || {
    echo "a missing platform package exited $status: $(cat "$work/missing.err")"; exit 1; }
  echo "a missing platform package: $(head -1 "$work/missing.err")"
fi

echo "== tools/set-version.sh moves every pin"
copy="$work/repo"
mkdir -p "$copy"
(cd "$root" && git ls-files -z --cached --others --exclude-standard | tar --null -T - -cf -) | tar -xf - -C "$copy"
old=$(sed -n 's/^version = "\(.*\)"/\1/p' "$copy/Cargo.toml" | head -1)
dummy="0.0.1-release-check.1"
(cd "$copy" && tools/set-version.sh "$dummy" >/dev/null)
(cd "$copy" && tools/check-versions.sh "$dummy")
left=$(cd "$copy" && grep -rlF --exclude=CHANGELOG.md --exclude-dir=node_modules --exclude-dir=target \
  --exclude-dir=snapshots "$old" . || true)
if [ -n "$left" ]; then
  echo "tools/set-version.sh left $old in:"; echo "$left"; exit 1
fi
echo "no file still says $old"
