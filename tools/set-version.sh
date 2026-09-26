#!/usr/bin/env bash
# Sets one version on every Skies package: the NuGet libraries, the Rust binary and its npm wrappers, the React
# packages and their private workspace, the Flutter spine, the agent plugin, and the package references `skies new`
# writes into an app (the csproj references and the CI's pinned binary).
# The publish workflow refuses a tag whose version any of these disagree with (tools/check-versions.sh, which this
# script also runs last). Usage: tools/set-version.sh 5.0.1
set -euo pipefail
version="${1:?usage: tools/set-version.sh <version>}"
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

# perl -pi rather than sed -i: BSD sed (macOS) reads `-i -E` as a backup suffix and has no `0,/re/` address. The
# version reaches perl through the environment, so no character in it is read as regex or code.
export SKIES_VERSION="$version"
perl -pi -e 's|<Version>[^<]+</Version>|<Version>$ENV{SKIES_VERSION}</Version>|' build/Skies.Framework.Library.props
perl -pi -e '$done ||= s/^version = "[^"]+"/version = "$ENV{SKIES_VERSION}"/' Cargo.toml
perl -pi -e 's/^version: .+/version: $ENV{SKIES_VERSION}/' flutter-sdk/packages/skies_flutter/pubspec.yaml
perl -pi -e 's|\@skiesjs/cli\@\S+|\@skiesjs/cli\@$ENV{SKIES_VERSION}|' cli/templates/app/.github/workflows/ci.yml
for csproj in cli/templates/app/src/*/*.csproj cli/templates/app/tests/*/*.csproj; do
  perl -pi -e 's|(Include="Skies[^"]*" Version=")[^"]+"|$1$ENV{SKIES_VERSION}"|' "$csproj"
done
node - "$version" <<'JS'
const fs = require("node:fs");
const version = process.argv[2];
const files = ["cli/npm/package.json", "skies-plugin/skies-plugin.json",
  ...fs.readdirSync("frontend-sdk/packages").map((dir) => `frontend-sdk/packages/${dir}/package.json`)];
for (const file of [...files, "frontend-sdk/package.json"]) {
  const json = JSON.parse(fs.readFileSync(file, "utf8"));
  json.version = version;
  for (const field of ["dependencies", "devDependencies", "peerDependencies", "optionalDependencies"]) {
    for (const name of Object.keys(json[field] ?? {})) {
      if (name.startsWith("@skiesjs/")) json[field][name] = version;
    }
  }
  fs.writeFileSync(file, JSON.stringify(json, null, 2) + "\n");
}
JS
cargo update -p skies-cli --offline >/dev/null 2>&1 || cargo update -p skies-cli >/dev/null
(cd frontend-sdk && npm install --package-lock-only --silent)
tools/check-versions.sh "$version"
echo "every package is now $version; re-bless generator snapshots: SKIES_BLESS=1 cargo test --test generators"
