#!/usr/bin/env bash
# Sets one version on every Skies package: the NuGet libraries, the Rust binary and its npm wrappers, the React
# packages and their private workspace, the Flutter spine, the agent plugin, and the package references `skies new`
# writes into an app (the csproj references and the CI's pinned binary).
# The publish workflow refuses a tag whose version any of these disagree with. Usage: tools/set-version.sh 5.0.1
set -euo pipefail
version="${1:?usage: tools/set-version.sh <version>}"
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

sed -i -E "s|<Version>[^<]+</Version>|<Version>$version</Version>|" build/Skies.Framework.Library.props
sed -i -E "0,/^version = \"[^\"]+\"/s//version = \"$version\"/" Cargo.toml
sed -i -E "s/^version: .+/version: $version/" flutter-sdk/packages/skies_flutter/pubspec.yaml
sed -i -E "s|@skiesjs/cli@[^ ]+|@skiesjs/cli@$version|" cli/templates/app/.github/workflows/ci.yml
for csproj in cli/templates/app/src/*/*.csproj cli/templates/app/tests/*/*.csproj; do
  sed -i -E "s|(Include=\"Skies[^\"]*\" Version=\")[^\"]+\"|\1$version\"|" "$csproj"
done
node - "$version" <<'JS'
const fs = require("node:fs");
const version = process.argv[2];
const files = ["cli/npm/package.json", "skies-plugin/skies-plugin.json",
  ...fs.readdirSync("frontend-sdk/packages").map((dir) => `frontend-sdk/packages/${dir}/package.json`)];
for (const file of [...files, "frontend-sdk/package.json"]) {
  const json = JSON.parse(fs.readFileSync(file, "utf8"));
  json.version = version;
  for (const name of Object.keys(json.devDependencies ?? {})) {
    if (name.startsWith("@skiesjs/")) json.devDependencies[name] = version;
  }
  for (const name of Object.keys(json.optionalDependencies ?? {})) {
    if (name.startsWith("@skiesjs/cli-")) json.optionalDependencies[name] = version;
  }
  fs.writeFileSync(file, JSON.stringify(json, null, 2) + "\n");
}
JS
cargo update -p skies-cli --offline >/dev/null 2>&1 || cargo update -p skies-cli >/dev/null
(cd frontend-sdk && npm install --package-lock-only --silent)
echo "every package is now $version; re-bless generator snapshots: SKIES_BLESS=1 cargo test --test generators"
