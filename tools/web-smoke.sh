#!/usr/bin/env bash
# web-smoke — the React path end to end, on what adopters install. It renders an app with `skies new`, gives it a
# backend with `g module` + `g entity` + `g crud`, builds it (the build writes the OpenAPI contract), adds a web
# package with `g web-app`, generates the typed client with `g client`, scaffolds a list and a form screen with
# `g feature`, routes them, and then proves the package builds (`vite build`), typechecks, and lints clean under
# `@skiesjs/eslint-plugin`'s recommended config, that a web spec made by `skies spec new` runs through the
# `[runners.web]` `g web-app` declares (the engine's output and report say which spec and runner ran), and that
# `skies doctor` is clean on the whole app.
#
# Packages come from this working tree: Skies.Framework.* is `dotnet pack`ed into a local feed (as in auth-smoke), and
# @skiesjs/react and @skiesjs/eslint-plugin are `npm pack`ed and installed from their tarballs. Everything else comes
# from nuget.org and the npm registry. Needs cargo, the .NET 10 SDK, and Node on PATH (e.g.
# `mise exec rust@latest dotnet@10 -- tools/web-smoke.sh`). Set SKIES to reuse a prebuilt binary, SKIES_SMOKE_PACKAGES
# to move the NuGet packages folder, and SKIES_WEB_SMOKE_DIR to keep the generated app (node_modules is large, so the
# default work folder lives under ~/.cache/skies, not /tmp).
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CACHE="${XDG_CACHE_HOME:-$HOME/.cache}/skies"
mkdir -p "$CACHE"
if [ -n "${SKIES_WEB_SMOKE_DIR:-}" ]; then
  WORK="$SKIES_WEB_SMOKE_DIR"; rm -rf "$WORK"; mkdir -p "$WORK"
else
  WORK="$(mktemp -d "$CACHE/web-smoke.XXXXXX")"; trap 'rm -rf "$WORK"' EXIT
fi
PACKAGES="${SKIES_SMOKE_PACKAGES:-$CACHE/smoke-packages}"
VERSION="$(sed -nE 's#.*<Version>([^<]+)</Version>.*#\1#p' "$REPO/build/Skies.Framework.Library.props" | head -1)"
fail() { echo "FAIL: $*" >&2; exit 1; }

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

echo "==> packing @skiesjs/react and @skiesjs/eslint-plugin $VERSION"
mkdir -p "$WORK/npm"
[ -d "$REPO/frontend-sdk/node_modules" ] || npm --prefix "$REPO/frontend-sdk" ci --no-audit --no-fund >/dev/null
npm --prefix "$REPO/frontend-sdk" run build --workspace @skiesjs/react >/dev/null 2>&1
(cd "$REPO/frontend-sdk" && npm pack --silent --workspace @skiesjs/react --workspace @skiesjs/eslint-plugin \
  --pack-destination "$WORK/npm" >/dev/null 2>&1)
REACT_TGZ="$WORK/npm/skiesjs-react-$VERSION.tgz"
PLUGIN_TGZ="$WORK/npm/skiesjs-eslint-plugin-$VERSION.tgz"
[ -f "$REACT_TGZ" ] && [ -f "$PLUGIN_TGZ" ] || fail "npm pack did not produce the $VERSION tarballs"

echo "==> rendering Shop: g module + g entity + g crud"
(cd "$WORK" && "$SKIES" new Shop >/dev/null)
APP="$WORK/Shop"
API="$APP/src/Shop.Api"
g() { (cd "$API" && "$SKIES" g "$@" >/dev/null); }
g auth --skip-tenancy
g module Catalog
g entity Catalog Product
cat > "$WORK/product-fields.cs" <<'EOF'

    /// <summary>The product name shown in the catalog.</summary>
    public string Name { get; private set; } = "";

    /// <summary>The unit price.</summary>
    public decimal Price { get; private set; }
EOF
sed -i "/^    public Guid Id { get; private set; }\$/r $WORK/product-fields.cs" "$API/Modules/Catalog/Product.cs"
grep -q 'public decimal Price' "$API/Modules/Catalog/Product.cs" || fail "the g entity scaffold changed shape"
g crud Catalog Product
cat > "$API/Modules/Catalog/Catalog.ctx.md" <<'EOF'
# catalog

The products the shop sells.

## Boundaries

- **Inside**: products, their names and prices, which only this module writes.
- **Outside**: stock and orders, which belong to their own modules.

## Design notes

A product's price is its current list price; an order keeps the price it was placed at.
EOF

echo "==> building the backend (writes the OpenAPI contract)"
dotnet build "$API" --nologo -v quiet >/dev/null || fail "the backend did not build"
CONTRACT="$API/contract/Shop.Api.json"
[ -f "$CONTRACT" ] || fail "dotnet build wrote no contract at $CONTRACT"
LIST="$(grep -oE '"operationId": *"ListProducts?"' "$CONTRACT" | head -1 | sed -E 's/.*"(List[A-Za-z]*)"/\1/')"
echo "    the list slice is $LIST"

echo "==> g web-app Web --path clients/web"
(cd "$APP" && "$SKIES" g web-app Web --path clients/web >/dev/null)
WEB="$APP/clients/web"
grep -q 'frontend = "clients/web"' "$APP/Skies.toml" && grep -q '"clients/"' "$APP/Skies.toml" \
  || fail "g web-app did not declare clients/web in Skies.toml"

echo "==> npm install (the @skiesjs packages from their tarballs)"
(cd "$WEB" && npm pkg set "dependencies.@skiesjs/react=file:$REACT_TGZ" \
  "devDependencies.@skiesjs/eslint-plugin=file:$PLUGIN_TGZ" \
  && npm install --no-audit --no-fund --loglevel=error >/dev/null) || fail "npm install"

echo "==> g client, g feature Products (list), CreateProduct, UpdateProduct, DeleteProduct (form), i18n"
(cd "$WEB" && "$SKIES" g client >/dev/null) || fail "g client"
(cd "$WEB" && "$SKIES" g feature Products --kind list >/dev/null) || fail "g feature Products"
(cd "$WEB" && "$SKIES" g feature CreateProduct --kind form >/dev/null) || fail "g feature CreateProduct"
(cd "$WEB" && "$SKIES" g feature UpdateProduct --kind form >/dev/null) || fail "g feature UpdateProduct"
(cd "$WEB" && "$SKIES" g feature DeleteProduct --kind form >/dev/null) || fail "g feature DeleteProduct"
(cd "$WEB" && "$SKIES" i18n >/dev/null) || fail "skies i18n"
grep -q "use$LIST" "$WEB/src/products/Products.viewModel.ts" || fail "the list screen does not read use$LIST"
grep -q 'name: z.string()' "$WEB/src/create-product/CreateProduct.viewModel.ts" \
  || fail "the form did not read CreateProduct's name field from the contract"
grep -q "version: record?.version ?? target.version" "$WEB/src/update-product/UpdateProduct.viewModel.ts" \
  && grep -q "params: { version: target.version }" "$WEB/src/delete-product/DeleteProduct.viewModel.ts" \
  || fail "the update form does not send the loaded record's version, or delete the version it was given"
grep -q "npm ci --prefix clients/web" "$APP/.github/workflows/ci.yml" || fail "g web-app did not add clients/web to the CI"

ROUTER="$WEB/src/routes/router.ts"
sed -i 's#^import { ShellView } from "@/shell/Shell.view";#&\nimport { ProductsView } from "@/products/Products.view";\nimport { CreateProductView } from "@/create-product/CreateProduct.view";#' "$ROUTER"
sed -i 's#^const routeTree = rootRoute.addChildren(\[homeRoute\]);#const productsRoute = createRoute({ getParentRoute: () => rootRoute, path: "/products", component: ProductsView });\nconst createProductRoute = createRoute({\n  getParentRoute: () => rootRoute,\n  path: "/products/new",\n  component: CreateProductView,\n});\n\nconst routeTree = rootRoute.addChildren([homeRoute, productsRoute, createProductRoute]);#' "$ROUTER"
grep -q 'productsRoute, createProductRoute' "$ROUTER" || fail "the router scaffold changed shape"

echo "==> a web spec through the runner g web-app declared"
grep -q '^\[runners.web\]$' "$APP/Skies.toml" || { cat "$APP/Skies.toml"; fail "g web-app did not declare [runners.web]"; }
# `spec new` picks the next free id (g auth already wrote 0001-auth); a hand-made 0001-home would share its id.
HOME_SPEC="$(cd "$APP" && "$SKIES" spec new home --runner web | sed -nE 's#^created \.specs/([^/]+)/.*#\1#p')"
[ -n "$HOME_SPEC" ] || fail "skies spec new home did not say which folder it created"
HOME_ID="${HOME_SPEC%%-*}"
[ -d "$APP/.specs/$HOME_SPEC/e2e" ] || fail "skies spec new did not create .specs/$HOME_SPEC/e2e"
cat > "$APP/.specs/$HOME_SPEC/spec.md" <<EOF
---
id: "$HOME_ID"
runner: web
---
# Home

## Failure modes

- FM-1 The start screen does not greet the visitor. [avp: none]

## AVP exemptions
- FM-1 The smoke fixture only checks static greeting text, directly asserted in the rendered DOM. | reviewed-by: smoke-fixture
EOF
cat > "$APP/.specs/$HOME_SPEC/e2e/Home.test.tsx" <<'EOF'
import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import "@/i18n";
import { HomeView } from "@/home/Home.view";

describe("Home", () => {
  it("FM-1: the start screen greets the visitor", () => {
    render(<HomeView />);
    expect(screen.getByText("Welcome")).toBeTruthy();
  });
});
EOF

echo "==> the web package: build, typecheck, lint, test"
for script in build typecheck lint test; do
  (cd "$WEB" && npm run --silent "$script" >"$WORK/$script.log" 2>&1) || { cat "$WORK/$script.log"; fail "npm run $script"; }
  echo "ok: npm run $script"
done
grep -qE '1 passed' "$WORK/test.log" || { cat "$WORK/test.log"; fail "the web spec did not run from the package"; }
(cd "$APP" && "$SKIES" proof run "$HOME_ID" >"$WORK/proof.log" 2>&1) \
  || { cat "$WORK/proof.log"; fail "skies proof run $HOME_ID"; }
# The engine names the spec and runner it ran, so a run that went to another spec or runner cannot pass as this one.
grep -qF "run $HOME_SPEC (runner web," "$WORK/proof.log" && grep -qxF "1/1 FMs pass" "$WORK/proof.log" \
  || { cat "$WORK/proof.log"; fail "skies proof run $HOME_ID did not run $HOME_SPEC through [runners.web]"; }
# vitest's JUnit report (the api runner would leave a TRX), holding the home spec's case.
RAW="$APP/.specs/$HOME_SPEC/evidence/raw"
[ ! -e "$RAW/run.trx" ] && grep -qF 'FM-1: the start screen greets the visitor' "$RAW/run.xml" \
  || { ls "$RAW"; fail "the web run's report is not vitest's JUnit report of the home spec"; }
echo "ok: skies proof run $HOME_ID ran $HOME_SPEC through [runners.web]"

echo "==> skies doctor on the whole app"
out="$(cd "$APP" && "$SKIES" doctor 2>&1)" || { echo "$out"; fail "skies doctor"; }
unclean="$(echo "$out" | awk '/^leg /{t=1;next} /^total /{t=0} t && $0 !~ / clean +0 /')"
[ -z "$unclean" ] || { echo "$out"; fail "skies doctor is not clean"; }
echo "$out" | grep -qE '^eslint clients/web +clean' && echo "$out" | grep -qE '^tsc clients/web +clean' \
  || { echo "$out"; fail "the doctor did not check clients/web"; }
echo "ok: skies doctor is clean (workspace, build, eslint and tsc on clients/web)"

echo "==> web-smoke OK"
