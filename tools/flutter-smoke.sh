#!/usr/bin/env bash
# flutter-smoke — the Flutter path end to end, on what adopters install. It renders an app with `skies new`, adds a
# Flutter package with `g flutter-app`, and proves that the package is pinned to this version of `skies_flutter`, is
# declared in Skies.toml (a product's `frontend`, the root allowlist, `[runners.flutter]`) and in the CI, analyzes
# clean, runs a spec case through that runner, still analyzes clean once `g client` has generated the typed client from
# a crud backend's contract and `g feature` has scaffolded a list screen on it, and that `skies doctor` is clean on the
# whole app (SKYWS001 included). The client generator runs through npx and needs Java.
#
# Packages come from this working tree: Skies.Framework.* is `dotnet pack`ed into a local feed (as in auth-smoke), and
# `skies_flutter` resolves to flutter-sdk/packages/skies_flutter through a pubspec_overrides.yaml the smoke drops in
# the package before `g flutter-app` first runs pub (the pinned version may not be on pub.dev yet). Everything else
# comes from nuget.org and pub.dev. Needs cargo, the .NET 10 SDK, and Flutter (FLUTTER, else `flutter` on PATH). Set
# SKIES to reuse a prebuilt binary, SKIES_SMOKE_PACKAGES to move the NuGet packages folder, and
# SKIES_FLUTTER_SMOKE_DIR to keep the generated app.
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CACHE="${XDG_CACHE_HOME:-$HOME/.cache}/skies"
mkdir -p "$CACHE"
if [ -n "${SKIES_FLUTTER_SMOKE_DIR:-}" ]; then
  WORK="$SKIES_FLUTTER_SMOKE_DIR"; rm -rf "$WORK"; mkdir -p "$WORK"
else
  WORK="$(mktemp -d "$CACHE/flutter-smoke.XXXXXX")"; trap 'rm -rf "$WORK"' EXIT
fi
PACKAGES="${SKIES_SMOKE_PACKAGES:-$CACHE/smoke-packages}"
VERSION="$(sed -nE 's#.*<Version>([^<]+)</Version>.*#\1#p' "$REPO/build/Skies.Framework.Library.props" | head -1)"
FLUTTER="${FLUTTER:-$(command -v flutter || true)}"
fail() { echo "FAIL: $*" >&2; exit 1; }
[ -n "$FLUTTER" ] || fail "flutter is not on PATH (set FLUTTER)"

if [ -z "${SKIES:-}" ]; then
  echo "==> building the skies binary"
  cargo build --quiet --manifest-path "$REPO/Cargo.toml" -p skies-cli
  SKIES="$REPO/target/debug/skies"
fi

echo "==> packing Skies.Framework.* $VERSION into a local feed"
mkdir -p "$WORK/feed" "$PACKAGES"
for project in "$REPO"/src/*/*.csproj "$REPO/analyzers/Skies.Framework.Doctor/Skies.Framework.Doctor.csproj"; do
  dotnet pack "$project" -c Release -o "$WORK/feed" -p:ContinuousIntegrationBuild=true --nologo -v quiet >/dev/null \
    || fail "dotnet pack $project"
done
rm -rf "$PACKAGES"/skies.framework*/"$VERSION"
cat > "$WORK/NuGet.config" <<EOF
<?xml version="1.0" encoding="utf-8"?>
<configuration>
  <packageSources>
    <clear />
    <add key="skies-working-tree" value="$WORK/feed" />
    <add key="nuget.org" value="https://api.nuget.org/v3/index.json" />
  </packageSources>
  <packageSourceMapping>
    <packageSource key="skies-working-tree">
      <package pattern="Skies.Framework" />
      <package pattern="Skies.Framework.*" />
    </packageSource>
    <packageSource key="nuget.org">
      <package pattern="*" />
    </packageSource>
  </packageSourceMapping>
  <config>
    <add key="globalPackagesFolder" value="$PACKAGES" />
  </config>
</configuration>
EOF

echo "==> rendering Shop"
(cd "$WORK" && "$SKIES" new Shop >/dev/null)
APP="$WORK/Shop"
MOBILE="$APP/clients/mobile"

# FLUTTER_BIN is how the scaffold finds Flutter. This shim points `skies_flutter` at the working tree before the
# first `pub` command resolves the package, then runs the real Flutter.
cat > "$WORK/flutter-shim" <<EOF
#!/usr/bin/env bash
if [ "\$1" = pub ] && [ -f pubspec.yaml ] && [ ! -f pubspec_overrides.yaml ]; then
  printf 'dependency_overrides:\n  skies_flutter:\n    path: %s\n' "$REPO/flutter-sdk/packages/skies_flutter" > pubspec_overrides.yaml
fi
exec "$FLUTTER" "\$@"
EOF
chmod +x "$WORK/flutter-shim"

echo "==> g flutter-app Mobile --path clients/mobile"
(cd "$APP" && FLUTTER_BIN="$WORK/flutter-shim" "$SKIES" g flutter-app Mobile --path clients/mobile >"$WORK/g.log" 2>&1) \
  || { cat "$WORK/g.log"; fail "g flutter-app"; }
grep -qE "^  skies_flutter: \"?\^?$VERSION\"?$" "$MOBILE/pubspec.yaml" && ! grep -qE "^  skies_flutter: \"?\^" "$MOBILE/pubspec.yaml" \
  || { grep -n skies_flutter "$MOBILE/pubspec.yaml"; fail "skies_flutter is not pinned to $VERSION"; }
grep -q 'frontend = "clients/mobile"' "$APP/Skies.toml" && grep -q '"clients/"' "$APP/Skies.toml" \
  || fail "g flutter-app did not declare clients/mobile in Skies.toml"
grep -q '^  flutter-clients-mobile:$' "$APP/.github/workflows/ci.yml" || fail "g flutter-app did not add a CI job"
grep -q '^\[runners.flutter\]$' "$APP/Skies.toml" || fail "g flutter-app did not declare [runners.flutter]"
grep -qE '^  intl: \^[0-9]' "$MOBILE/pubspec.yaml" || { grep -n 'intl' "$MOBILE/pubspec.yaml"; fail "intl is not pinned"; }
if command -v actionlint >/dev/null; then
  actionlint "$APP/.github/workflows/ci.yml" || fail "the generated CI does not lint"
fi
echo "ok: pinned to $VERSION, declared in Skies.toml (frontend, root, runner), a CI job"

echo "==> flutter analyze"
(cd "$MOBILE" && "$FLUTTER" analyze >"$WORK/analyze.log" 2>&1) || { cat "$WORK/analyze.log"; fail "flutter analyze"; }
echo "ok: flutter analyze"

echo "==> a Flutter spec through the declared runner"
mkdir -p "$APP/.specs/0001-app/e2e"
cat > "$APP/.specs/0001-app/spec.md" <<'EOF'
---
id: "0001"
runner: flutter
---
# App

## Failure modes

- FM-1 The app does not start.
EOF
cat > "$APP/.specs/0001-app/e2e/app_test.dart" <<'EOF'
import 'package:flutter_test/flutter_test.dart';
import 'package:mobile/main.dart';

void main() {
  testWidgets('FM-1: the app starts', (tester) async {
    await tester.pumpWidget(const MainApp());
    expect(tester.takeException(), isNull);
  });
}
EOF
(cd "$APP" && PATH="$(dirname "$FLUTTER"):$PATH" "$SKIES" proof run 0001 >"$WORK/proof.log" 2>&1) \
  || { cat "$WORK/proof.log"; fail "skies proof run 0001"; }
echo "ok: skies proof run 0001 through [runners.flutter]"

echo "==> a crud backend, its contract, then g client + g feature Products in clients/mobile"
API="$APP/src/Shop.Api"
g() { (cd "$API" && "$SKIES" g "$@" >/dev/null); }
g auth --skip-tenancy
g module Catalog
g entity Catalog Product
cat > "$WORK/product-fields.cs" <<'EOF'

    /// <summary>The product name shown in the catalog.</summary>
    public string Name { get; private set; } = "";
EOF
sed -i "/^    public Guid Id { get; private set; }\$/r $WORK/product-fields.cs" "$API/Modules/Catalog/Product.cs"
grep -q 'public string Name' "$API/Modules/Catalog/Product.cs" || fail "the g entity scaffold changed shape"
g crud Catalog Product
cat > "$API/Modules/Catalog/Catalog.ctx.md" <<'EOF'
# catalog

The products the shop sells.

## Boundaries

- **Inside**: products and their names, which only this module writes.
- **Outside**: stock and orders, which belong to their own modules.

## Design notes

A product's name is what the catalog shows; nothing else reads it.
EOF
dotnet build "$API" --nologo -v quiet >/dev/null || fail "the backend did not build"
[ -f "$API/contract/Shop.Api.json" ] || fail "dotnet build wrote no contract"
(cd "$MOBILE" && PATH="$(dirname "$FLUTTER"):$PATH" "$SKIES" g client >"$WORK/client.log" 2>&1) \
  || { cat "$WORK/client.log"; fail "g client"; }
(cd "$MOBILE" && PATH="$(dirname "$FLUTTER"):$PATH" "$SKIES" g feature Products --kind list >"$WORK/feature.log" 2>&1) \
  || { cat "$WORK/feature.log"; fail "g feature Products"; }
(cd "$MOBILE" && "$FLUTTER" pub get >/dev/null && "$FLUTTER" analyze >"$WORK/analyze.log" 2>&1) \
  || { cat "$WORK/analyze.log"; fail "flutter analyze after g client + g feature"; }
echo "ok: g client + g feature Products, flutter analyze clean"

echo "==> skies doctor on the whole app"
out="$(cd "$APP" && "$SKIES" doctor 2>&1)" || { echo "$out"; fail "skies doctor"; }
unclean="$(echo "$out" | awk '/^leg /{t=1;next} /^total /{t=0} t && $0 !~ / clean +0 /')"
[ -z "$unclean" ] || { echo "$out"; fail "skies doctor is not clean"; }
echo "$out" | grep -qE '^flutter clients/mobile +clean' && echo "$out" | grep -qE '^workspace +clean' \
  || { echo "$out"; fail "the doctor did not check clients/mobile and the root"; }
echo "ok: skies doctor is clean (workspace, build, flutter clients/mobile)"

echo "==> flutter-smoke OK"
