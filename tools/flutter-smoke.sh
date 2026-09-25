#!/usr/bin/env bash
# flutter-smoke — the Flutter path end to end, on what adopters install. It renders an app with `skies new`, adds a
# Flutter package with `g flutter-app`, and proves that the package is pinned to this version of `skies_flutter`, is
# declared in Skies.toml (a product's `frontend`, the root allowlist) and in the CI, analyzes clean, runs a spec case
# through the `[runners.flutter]` the scaffold prints, and that `skies doctor` is clean on the whole app (SKYWS001
# included).
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
grep -q '^\[runners.flutter\]$' "$WORK/g.log" || { cat "$WORK/g.log"; fail "g flutter-app printed no [runners.flutter]"; }
if command -v actionlint >/dev/null; then
  actionlint "$APP/.github/workflows/ci.yml" || fail "the generated CI does not lint"
fi
echo "ok: pinned to $VERSION, declared in Skies.toml, a CI job"

echo "==> flutter analyze"
(cd "$MOBILE" && "$FLUTTER" analyze >"$WORK/analyze.log" 2>&1) || { cat "$WORK/analyze.log"; fail "flutter analyze"; }
echo "ok: flutter analyze"

echo "==> a Flutter spec through the printed runner"
sed -n '/^\[runners.flutter\]$/,/^command = /p' "$WORK/g.log" > "$WORK/runner.toml"
printf '\n' >> "$APP/Skies.toml" && cat "$WORK/runner.toml" >> "$APP/Skies.toml"
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

echo "==> skies doctor on the whole app"
out="$(cd "$APP" && "$SKIES" doctor 2>&1)" || { echo "$out"; fail "skies doctor"; }
unclean="$(echo "$out" | awk '/^leg /{t=1;next} /^total /{t=0} t && $0 !~ / clean +0 /')"
[ -z "$unclean" ] || { echo "$out"; fail "skies doctor is not clean"; }
echo "$out" | grep -qE '^flutter clients/mobile +clean' && echo "$out" | grep -qE '^workspace +clean' \
  || { echo "$out"; fail "the doctor did not check clients/mobile and the root"; }
echo "ok: skies doctor is clean (workspace, build, flutter clients/mobile)"

echo "==> flutter-smoke OK"
