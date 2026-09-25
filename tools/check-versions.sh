#!/usr/bin/env bash
# Fails unless every Skies version pin in the repository equals <version>: the packages themselves (NuGet, the Rust
# binary and its lockfile entry, the npm wrapper and its platform packages, the React packages and their workspace,
# the Flutter spine, the agent plugin) and the pins `skies new` writes into an app (the csproj references and the
# CI's `@skiesjs/cli`). What `skies g web-app` and `skies g flutter-app` pin (`@skiesjs/*`, `skies_flutter`) is the
# binary's own version, so the Cargo.toml line covers it. The publish workflow runs this against the tag, and
# tools/release-check.sh runs it after tools/set-version.sh to prove that script moves every pin.
# Usage: tools/check-versions.sh <version>
set -euo pipefail
want="${1:?usage: tools/check-versions.sh <version>}"
cd "$(cd "$(dirname "$0")/.." && pwd)"

fail=0
check() {
  if [ "$2" != "$want" ]; then
    echo "$1 is '${2}', want $want"
    fail=1
  fi
}

check build/Skies.Framework.Library.props "$(sed -n 's:.*<Version>\(.*\)</Version>.*:\1:p' build/Skies.Framework.Library.props)"
check Cargo.toml "$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
check "Cargo.lock (skies-cli)" "$(awk '/^name = "skies-cli"$/ { getline; gsub(/version = |"/, ""); print }' Cargo.lock)"
check flutter-sdk/packages/skies_flutter/pubspec.yaml \
  "$(sed -n 's/^version:[[:space:]]*//p' flutter-sdk/packages/skies_flutter/pubspec.yaml)"

ci=cli/templates/app/.github/workflows/ci.yml
pins=$(grep -o '@skiesjs/cli@[^ ]*' "$ci" || true)
[ -n "$pins" ] || { echo "$ci installs no @skiesjs/cli@<version>"; fail=1; }
for pin in $pins; do check "$ci (@skiesjs/cli)" "${pin#@skiesjs/cli@}"; done

for csproj in cli/templates/app/src/*/*.csproj cli/templates/app/tests/*/*.csproj; do
  for pin in $(sed -n 's/.*Include="\(Skies[^"]*\)" Version="\([^"]*\)".*/\1@\2/p' "$csproj"); do
    check "$csproj (${pin%@*})" "${pin#*@}"
  done
done

node - "$want" <<'JS' || fail=1
const fs = require("node:fs");
const want = process.argv[2];
let ok = true;
const check = (what, have) => {
  if (have !== want) { console.log(`${what} is '${have}', want ${want}`); ok = false; }
};
const files = ["cli/npm/package.json", "skies-plugin/skies-plugin.json", "frontend-sdk/package.json",
  ...fs.readdirSync("frontend-sdk/packages").map((dir) => `frontend-sdk/packages/${dir}/package.json`)];
for (const file of files) {
  const json = JSON.parse(fs.readFileSync(file, "utf8"));
  check(file, json.version);
  for (const field of ["dependencies", "devDependencies", "peerDependencies", "optionalDependencies"]) {
    for (const [name, range] of Object.entries(json[field] ?? {})) {
      if (name.startsWith("@skiesjs/")) check(`${file} ${field}.${name}`, range);
    }
  }
}
const wrapper = JSON.parse(fs.readFileSync("cli/npm/package.json", "utf8"));
if (Object.keys(wrapper.optionalDependencies ?? {}).length === 0) {
  console.log("cli/npm/package.json has no platform packages in optionalDependencies");
  ok = false;
}
const lock = JSON.parse(fs.readFileSync("frontend-sdk/package-lock.json", "utf8"));
for (const [where, entry] of Object.entries(lock.packages ?? {})) {
  if (where === "" || where.startsWith("packages/")) check(`frontend-sdk/package-lock.json "${where}"`, entry.version);
}
process.exit(ok ? 0 : 1);
JS

[ "$fail" = 0 ] && echo "every pin is $want"
exit "$fail"
