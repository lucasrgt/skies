#!/usr/bin/env bash
# auth-smoke — render apps with the `skies` binary, point them at this working tree's Skies.Framework.* projects,
# and prove the generated code builds doctor-clean and its spec E2E pass. This is the guard that catches a template
# regression (a generated app that does not compile, trips a SKY rule, or ships a red spec) before a release does.
#
# Three apps, each rendered with `skies new`:
#   Full   — g auth + auth:otp + auth:oauth + auth:email, plus module/slice/entity/vo/hub and a slice under an
#            anonymous module group.
#   Single — g auth --skip-tenancy --skip-cookies.
#   Crud   — g auth + module + g entity (given tenancy and fields) + g crud, with no edit after crud.
#
# Legs per app:
#   DOCTOR — run `skies doctor` in the app: the workspace leg (every root entry declared in Skies.toml, SKYWS*) and
#            the tests project's build (and through it the API's) with the SKY* analyzers ON. Every leg must be clean
#            (errors and warnings alike, for every app), so the generated spec cases are held to SKY0029 (every test
#            lives in a spec) and the generators to the declared root, with the rest.
#   SPECS  — build and run the tests project (analyzers off), which compiles .specs/*/e2e; every case must pass
#            and the count of passed tests must equal the number of FM cases in the specs.
#
# Headless and Docker-free: the in-memory provider backs the tests. Needs cargo and the .NET 10 SDK on PATH (e.g.
# `mise exec rust@latest dotnet@10 -- tools/auth-smoke.sh`). Set SKIES to reuse a prebuilt binary.
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

if [ -z "${SKIES:-}" ]; then
  echo "==> building the skies binary"
  cargo build --quiet --manifest-path "$REPO/Cargo.toml" -p skies-cli
  SKIES="$REPO/target/debug/skies"
fi

# Swap every Skies.Framework.* PackageReference for a ProjectReference into this checkout, so the smoke tests the
# working tree rather than whatever is on the package feed. The doctor becomes an analyzer-only reference.
use_working_tree() {
  local csproj="$1"
  sed -E -i \
    -e "s#<PackageReference Include=\"Skies\.Framework\.Doctor\" Version=\"[^\"]*\" PrivateAssets=\"all\" />#<ProjectReference Include=\"$REPO/analyzers/Skies.Framework.Doctor/Skies.Framework.Doctor.csproj\" OutputItemType=\"Analyzer\" ReferenceOutputAssembly=\"false\" />#" \
    -e "s#<PackageReference Include=\"(Skies\.Framework\.[A-Za-z.]+)\" Version=\"[^\"]*\" />#<ProjectReference Include=\"$REPO/src/\1/\1.csproj\" />#" \
    "$csproj"
  if grep -q 'PackageReference Include="Skies\.' "$csproj"; then
    echo "FAIL: $csproj still references a Skies package from the feed" >&2
    exit 1
  fi
}

# new <App> — render an app into $WORK/<App> and print its API directory.
new_app() {
  (cd "$WORK" && "$SKIES" new "$1" >/dev/null)
  echo "$WORK/$1/src/$1.Api"
}

g() { (cd "$API" && "$SKIES" g "$@" >/dev/null); }

# doctor <App> — run `skies doctor` in the app: the workspace leg, then the tests project's build (it references the
# API) with the analyzers on. Every leg in its table must read `clean`: any finding, error or warning, fails.
doctor() {
  local app="$1" out ok=1 unclean
  echo "==> [$app] DOCTOR: skies doctor (declared root + build with the SKY* analyzers on)"
  use_working_tree "$WORK/$app/src/$app.Api/$app.Api.csproj"
  use_working_tree "$WORK/$app/tests/$app.Tests/$app.Tests.csproj"
  out="$(cd "$WORK/$app" && "$SKIES" doctor 2>&1)" || ok=0
  # The table's rows sit between its `leg` header and its `total` line.
  unclean="$(echo "$out" | awk '/^leg /{t=1;next} /^total /{t=0} t && $0 !~ / clean +0 /')"
  echo "$out" | grep -qE '^workspace +clean ' || unclean="${unclean:-the workspace leg did not run}"
  if [ "$ok" -ne 1 ] || [ -n "$unclean" ]; then
    echo "FAIL: [$app] must be doctor-clean. Reported:"; echo "$out" | head -60
    exit 1
  fi
  echo "ok: [$app] skies doctor is clean: the root is declared and the build has zero SKY diagnostics"
}

# specs <App> — run the tests project (it compiles .specs/*/e2e) and check every FM case ran and passed.
specs() {
  local app="$1" out expected passed
  echo "==> [$app] SPECS: run the spec E2E"
  expected="$(cat "$WORK/$app"/.specs/*/e2e/*.cs | grep -c 'DisplayName = "FM-')"
  if ! out="$(dotnet test "$WORK/$app/tests/$app.Tests/$app.Tests.csproj" -p:RunAnalyzers=false 2>&1)"; then
    echo "$out" | tail -60
    echo "FAIL: [$app] spec E2E failed"
    exit 1
  fi
  passed="$(echo "$out" | grep -oE 'Passed: +[0-9]+' | grep -oE '[0-9]+' | tail -1)"
  if [ "$passed" != "$expected" ]; then
    echo "$out" | tail -20
    echo "FAIL: [$app] $passed tests passed, but the specs declare $expected FM cases"
    exit 1
  fi
  echo "ok: [$app] $passed/$expected spec cases passed ($(ls -d "$WORK/$app"/.specs/*/ | xargs -n1 basename | tr '\n' ' '))"
}

echo "==> rendering Full: auth + otp + oauth + email + module/slice/entity/vo/hub"
API="$(new_app Full)"
g auth; g auth:otp; g auth:oauth; g auth:email
g module Billing; g slice Billing CreateInvoice; g slice Billing GetInvoice; g entity Billing Invoice
g vo Money; g hub Billing Payments
# A module whose group is anonymous on purpose: the slice inherits the group's decision (SKY0022) and states none of
# its own, so it never asks for an auth scheme the group waived.
g module Status
sed -i 's#^        //   <Slice>.Map(status);$#&\n        var status = app.MapGroup("/status").AllowAnonymous();#' "$API/Modules/Status/StatusModule.cs"
g slice Status Uptime
grep -q '        Uptime.Map(status);' "$API/Modules/Status/StatusModule.cs" \
  && ! grep -q 'RequireAuthorization' "$API/Modules/Status/Slices/Uptime.cs" \
  || { echo "FAIL: g slice did not map Uptime under the anonymous group without a posture of its own" >&2; exit 1; }

echo "==> rendering Single: auth --skip-tenancy --skip-cookies"
API="$(new_app Single)"
g auth --skip-tenancy --skip-cookies

echo "==> rendering Crud: auth + module + entity + crud"
API="$(new_app Crud)"
g auth; g module Catalog; g entity Catalog Product
# The owner's side, before `g crud` and only there: give the scaffolded entity its tenancy and its domain state
# (encapsulated, as `g entity` leaves it), and register its DbSet. crud then writes Open/Update/RowVersion into the
# entity, the slices, and the module's route group; nothing is edited after it.
ENTITY="$API/Modules/Catalog/Product.cs"
sed -i '1i using Crud.Api.Tenancy;\n' "$ENTITY"
sed -i 's#^public class Product$#public class Product : ITenantScoped#' "$ENTITY"
cat > "$WORK/product-fields.cs" <<'EOF'

    /// <summary>The owning org, stamped by the DbContext.</summary>
    public Guid OrgId { get; private set; }

    /// <summary>The product name shown in the catalog.</summary>
    public string Name { get; private set; } = "";

    /// <summary>The unit price.</summary>
    public decimal Price { get; private set; }

    /// <summary>When the product was opened.</summary>
    public DateTime CreatedAt { get; private set; }

    /// <summary>When the product last changed.</summary>
    public DateTime UpdatedAt { get; private set; }
EOF
sed -i "/^    public Guid Id { get; private set; }\$/r $WORK/product-fields.cs" "$ENTITY"
grep -q 'public class Product : ITenantScoped' "$ENTITY" && grep -q 'public decimal Price' "$ENTITY" \
  || { echo "FAIL: the g entity scaffold changed shape; the smoke's owner edit no longer applies" >&2; exit 1; }
sed -i 's#^    public DbSet<UserSession> UserSessions => Set<UserSession>();#&\n\n    public DbSet<Crud.Api.Modules.Catalog.Product> Products => Set<Crud.Api.Modules.Catalog.Product>();#' "$API/AppDb.cs"
grep -q 'DbSet<Crud.Api.Modules.Catalog.Product>' "$API/AppDb.cs" \
  || { echo "FAIL: AppDb.cs changed shape; the smoke could not register the DbSet" >&2; exit 1; }
g crud Catalog Product

doctor Full
specs Full
doctor Single
specs Single
doctor Crud
specs Crud

echo "==> auth-smoke OK"
