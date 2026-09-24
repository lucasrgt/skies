#!/usr/bin/env bash
# auth-smoke — render apps with the `skies` binary, restore them against this working tree's Skies.Framework.*
# packages, and prove the generated code builds doctor-clean and its spec E2E pass. This is the guard that catches a
# template regression (a generated app that does not compile, trips a SKY rule, or ships a red spec) before a release
# does.
#
# It tests what adopters install: the framework is `dotnet pack`ed at the working-tree version into a local feed, and
# the generated apps restore Skies.Framework.* from that feed through a temporary NuGet.config (every other package
# from nuget.org), into a dedicated packages folder whose entries for that version are purged first. So a package that
# lacks a type the templates use (an API that only exists as source) fails here, as it would for an adopter.
#
# Three apps, each rendered with `skies new`:
#   Full   — g auth + auth:otp + auth:oauth + auth:email, plus module/slice/entity/vo/hub and a slice under an
#            anonymous module group.
#   Single — g auth --skip-tenancy --skip-cookies.
#   Crud   — g auth + module + g entity (given tenancy and fields) + g crud, and an app-wide entity with its own crud,
#            with no edit to the generated code after crud. Its spec drives the crud over HTTP: another org's row is
#            a not-found, a stale version is a conflict, and only an app admin writes the app-wide entity.
# The owner's part is done by hand, as an author would: each generated module's ctx gets real boundaries and design
# notes (the doctor refuses the skeleton), and each crud entity gets its domain state before crud reads it.
#
# Legs per app:
#   PACKAGE — the generated API holds no auth mechanics of its own (no crypto primitive, no argon2 package): hashing,
#            token minting, rotation, and code checks are Skies.Framework.Auth's, so a fix there reaches every app.
#   DOCTOR — run `skies doctor` in the app: the workspace leg (every root entry declared in Skies.toml, SKYWS*) and
#            the tests project's build (and through it the API's) with the SKY* analyzers ON. Every leg must be clean
#            (errors and warnings alike, for every app).
#   SPECS  — build and run the tests project (analyzers off), which compiles .specs/*/e2e; every case must pass
#            and the count of passed tests must equal the number of FM cases in the specs.
#   PROOFS — `skies proof run` on every spec with the app's own runner: each passes, and the engine counts exactly the
#            `- FM-<n>` lines its spec.md lists (a spec whose modes do not parse fails instead of passing on zero).
#
# Headless and Docker-free: the in-memory provider backs the tests. Needs cargo and the .NET 10 SDK on PATH (e.g.
# `mise exec rust@latest dotnet@10 -- tools/auth-smoke.sh`). Set SKIES to reuse a prebuilt binary, and
# SKIES_SMOKE_PACKAGES to move the packages folder (default ~/.cache/skies/smoke-packages).
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
FEED="$WORK/feed"
PACKAGES="${SKIES_SMOKE_PACKAGES:-$HOME/.cache/skies/smoke-packages}"
VERSION="$(sed -nE 's#.*<Version>([^<]+)</Version>.*#\1#p' "$REPO/build/Skies.Framework.Library.props" | head -1)"
[ -n "$VERSION" ] || { echo "FAIL: no <Version> in build/Skies.Framework.Library.props" >&2; exit 1; }

if [ -z "${SKIES:-}" ]; then
  echo "==> building the skies binary"
  cargo build --quiet --manifest-path "$REPO/Cargo.toml" -p skies-cli
  SKIES="$REPO/target/debug/skies"
fi

echo "==> packing Skies.Framework.* $VERSION into a local feed"
mkdir -p "$FEED"
for project in "$REPO"/src/*/*.csproj "$REPO/analyzers/Skies.Framework.Doctor/Skies.Framework.Doctor.csproj"; do
  dotnet pack "$project" -c Release -o "$FEED" -p:ContinuousIntegrationBuild=true --nologo -v quiet >/dev/null \
    || { echo "FAIL: dotnet pack $project" >&2; exit 1; }
done
ls "$FEED"/Skies.Framework.Auth."$VERSION".nupkg >/dev/null

# Hermetic restore: Skies.Framework.* only from the feed (source mapping), everything else from nuget.org, into a
# packages folder of its own. A cached copy of this version (the published package, or an older pack) is purged so
# the restore takes the working tree's.
mkdir -p "$PACKAGES"
rm -rf "$PACKAGES"/skies.framework*/"$VERSION"
cat > "$WORK/NuGet.config" <<EOF
<?xml version="1.0" encoding="utf-8"?>
<configuration>
  <packageSources>
    <clear />
    <add key="skies-working-tree" value="$FEED" />
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

# new <App> — render an app into $WORK/<App> (under the feed's NuGet.config) and print its API directory.
new_app() {
  (cd "$WORK" && "$SKIES" new "$1" >/dev/null)
  echo "$WORK/$1/src/$1.Api"
}

g() { (cd "$API" && "$SKIES" g "$@" >/dev/null); }

# ctx <Module> <inside> <outside> <design note> — the author's part of `g module`: the module's own boundaries and
# design notes, replacing the skeleton's commented hints.
ctx() {
  local file="$API/Modules/$1/$1.ctx.md"
  grep -q '^<!-- ' "$file" || { echo "FAIL: $file is not the g module skeleton" >&2; exit 1; }
  cat > "$file" <<EOF
# $(echo "$1" | tr '[:upper:]' '[:lower:]')

The $1 module of the smoke app.

## Boundaries

- **Inside**: $2
- **Outside**: $3

## Design notes

$4
EOF
}

# uses_packages <App> — every Skies.Framework reference is a package at the working-tree version, never a project.
uses_packages() {
  local app="$1" csproj
  for csproj in "$WORK/$app/src/$app.Api/$app.Api.csproj" "$WORK/$app/tests/$app.Tests/$app.Tests.csproj"; do
    if grep -q 'ProjectReference Include="[^"]*Skies\.Framework' "$csproj" \
      || grep -E 'PackageReference Include="Skies\.Framework[^"]*"' "$csproj" | grep -vq "Version=\"$VERSION\""; then
      echo "FAIL: $csproj does not reference Skies.Framework.* $VERSION as packages" >&2
      exit 1
    fi
  done
}

# doctor <App> — run `skies doctor` in the app: the workspace leg, then the tests project's build (it references the
# API) with the analyzers on. Every leg in its table must read `clean`: any finding, error or warning, fails.
doctor() {
  local app="$1" out ok=1 unclean
  echo "==> [$app] DOCTOR: skies doctor (declared root + build with the SKY* analyzers on, packages $VERSION)"
  uses_packages "$app"
  out="$(cd "$WORK/$app" && "$SKIES" doctor 2>&1)" || ok=0
  # The table's rows sit between its `leg` header and its `total` line.
  unclean="$(echo "$out" | awk '/^leg /{t=1;next} /^total /{t=0} t && $0 !~ / clean +0 /')"
  echo "$out" | grep -qE '^workspace +clean ' || unclean="${unclean:-the workspace leg did not run}"
  if [ "$ok" -ne 1 ] || [ -n "$unclean" ]; then
    echo "FAIL: [$app] must be doctor-clean. Reported:"; echo "$out" | head -60
    exit 1
  fi
  if [ ! -f "$PACKAGES/skies.framework.abstractions/$VERSION/skies.framework.abstractions.$VERSION.nupkg" ]; then
    echo "FAIL: [$app] did not restore Skies.Framework.Abstractions $VERSION from the feed" >&2
    exit 1
  fi
  echo "ok: [$app] skies doctor is clean against the packed packages: the root is declared, zero SKY diagnostics"
}

# package <App> — fail when the generated API re-implements a mechanism the package owns.
package() {
  local app="$1" found
  echo "==> [$app] PACKAGE: the auth mechanics stay in Skies.Framework.Auth"
  found="$(grep -rlE 'System\.Security\.Cryptography|RandomNumberGenerator|FixedTimeEquals|SHA256|Konscious' \
    "$WORK/$app/src/$app.Api" --include='*.cs' --include='*.csproj' || true)"
  if [ -n "$found" ]; then
    echo "FAIL: [$app] generated code carries auth mechanics the package owns:"; echo "$found"
    exit 1
  fi
  echo "ok: [$app] no crypto or hashing package in the generated API"
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

# proofs <App> — run every spec through the engine (`skies proof run`, the app's own runner): each must pass, and
# the engine must see every `- FM-<n>` line of its spec.md, so a spec whose failure modes do not parse (zero FMs, or
# fewer than written) fails here instead of passing vacuously.
proofs() {
  local app="$1" spec name modes out
  echo "==> [$app] PROOFS: skies proof run on every spec"
  for spec in "$WORK/$app"/.specs/*/spec.md; do
    name="$(basename "$(dirname "$spec")")"
    modes="$(awk '/^## /{s=(tolower($0) ~ /^## failure modes/)} s && /^[[:space:]]*[-*][[:space:]]+FM-[0-9]+([: ]|$)/{n++} END{print n+0}' "$spec")"
    if [ "$modes" -eq 0 ]; then
      echo "FAIL: [$app] $name/spec.md lists no \`- FM-<n>\` line"; exit 1
    fi
    if ! out="$(cd "$WORK/$app" && "$SKIES" proof run "$name" 2>&1)" \
      || ! echo "$out" | grep -q "^$modes/$modes FMs pass$"; then
      echo "$out" | tail -40
      echo "FAIL: [$app] skies proof run $name must pass all $modes failure modes spec.md lists"
      exit 1
    fi
    echo "ok: [$app] $name: $modes/$modes FMs pass through the engine"
  done
}

echo "==> rendering Full: auth + otp + oauth + email + module/slice/entity/vo/hub"
API="$(new_app Full)"
g auth; g auth:otp; g auth:oauth; g auth:email
g module Billing; g slice Billing CreateInvoice; g slice Billing GetInvoice; g entity Billing Invoice
g vo Money; g hub Billing Payments
ctx Billing "invoices and their lifecycle, which only this module writes." \
  "payments settle through a provider the Payments hub reports on; accounts are referenced by id." \
  "An invoice is immutable once issued: a correction is a new invoice, so a paid amount never changes under a payer."
# A module whose group is anonymous on purpose: the slice inherits the group's decision and states none of its own,
# so it never asks for an auth scheme the group waived.
g module Status
sed -i '/^    public static void Map(IEndpointRouteBuilder app)$/{n;s#$#\n        var status = app.MapGroup("/status").AllowAnonymous();#}' \
  "$API/Modules/Status/StatusModule.cs"
grep -q '^        var status = app.MapGroup("/status").AllowAnonymous();$' "$API/Modules/Status/StatusModule.cs" \
  || { echo "FAIL: the g module scaffold changed shape; the smoke could not declare an anonymous group" >&2; exit 1; }
g slice Status Uptime
grep -q '        Uptime.Map(status);' "$API/Modules/Status/StatusModule.cs" \
  && ! grep -q 'RequireAuthorization' "$API/Modules/Status/Slices/Uptime.cs" \
  || { echo "FAIL: g slice did not map Uptime under the anonymous group without a posture of its own" >&2; exit 1; }
ctx Status "the public uptime probe status pages poll." "health of dependencies, which belongs to real monitoring." \
  "Status is anonymous as a whole because a status page must answer for signed-out visitors."

echo "==> rendering Single: auth --skip-tenancy --skip-cookies"
API="$(new_app Single)"
g auth --skip-tenancy --skip-cookies

echo "==> rendering Crud: auth + module + entity + crud, tenant-scoped and app-wide"
API="$(new_app Crud)"
g auth; g module Catalog; g entity Catalog Product; g entity Catalog Tag
# The owner's side, before `g crud` and only there: give the scaffolded entities their domain state (encapsulated, as
# `g entity` leaves it) and Product its tenancy. crud then registers each DbSet in AppDb and writes Open/Update/
# RowVersion into the entity, its view record, the slices, and the module's route group; nothing is edited after it.
ENTITY="$API/Modules/Catalog/Product.cs"
sed -i '1i using Skies.Framework.EntityFrameworkCore;\n' "$ENTITY"
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
cat > "$WORK/tag-fields.cs" <<'EOF'

    /// <summary>The label every org shares.</summary>
    public string Label { get; private set; } = "";
EOF
sed -i "/^    public Guid Id { get; private set; }\$/r $WORK/tag-fields.cs" "$API/Modules/Catalog/Tag.cs"
g crud Catalog Product
g crud Catalog Tag
grep -q 'public DbSet<Product> Products => Set<Product>();' "$API/AppDb.cs" \
  && grep -q 'public DbSet<Tag> Tags => Set<Tag>();' "$API/AppDb.cs" \
  || { echo "FAIL: g crud did not register the DbSets in AppDb.cs" >&2; exit 1; }
ctx Catalog "the products an org sells and the tags every org shares." \
  "pricing rules and stock, which belong to their own modules." \
  "Products are scoped to their org by the DbContext's tenant filter; tags are app-wide, so any org may read them."

# Exercise a real domain rejection: a tracked object must retain its original state after a failed update.
sed -i 's/\.Check(Id != Guid.Empty, "id", CatalogErrorCodes.IdRequired, "is required");/.Check(Id != Guid.Empty, "id", CatalogErrorCodes.IdRequired, "is required")\n            .Check(!string.IsNullOrWhiteSpace(Name), "name", CatalogErrorCodes.NameRequired, "is required");/' "$ENTITY"
sed -i '/^public static class CatalogErrorCodes/{n;a\
    public const string NameRequired = "catalog.name_required";
}' "$API/Modules/Catalog/CatalogErrorCodes.cs"
mkdir -p "$WORK/Crud/.specs/9999-crud/e2e"
cat > "$WORK/Crud/.specs/9999-crud/spec.md" <<'EOF'
---
id: "9999"
runner: api
---
# Generated CRUD state transitions

## Failure modes
- FM-1 invalid input changes an existing entity.
- FM-2 valid input fails to update an existing entity.
- FM-3 a signed-in user reads or changes another org's product.
- FM-4 an update or delete made against a version someone else changed since overwrites their change.
- FM-5 a signed-in member creates or deletes a tag every org shares.
EOF
cat > "$WORK/Crud/.specs/9999-crud/e2e/Mutation.cs" <<'EOF'
using Crud.Api.Modules.Catalog;

namespace Specs.S9999;

public class Mutation
{
    [Fact(DisplayName = "FM-1: failed validation preserves the original fields and timestamp")]
    public void Rejected_update_preserves_state()
    {
        var now = DateTime.UtcNow;
        var item = Product.Open(Guid.NewGuid(), "Original", 10m, now).Value;

        var result = item.Update("", 20m, now.AddMinutes(1));

        Assert.True(result.IsFailure);
        Assert.Equal("Original", item.Name);
        Assert.Equal(10m, item.Price);
        Assert.Equal(now, item.UpdatedAt);
    }

    [Fact(DisplayName = "FM-2: successful validation applies changes to the same entity")]
    public void Accepted_update_changes_the_same_instance()
    {
        var now = DateTime.UtcNow;
        var item = Product.Open(Guid.NewGuid(), "Original", 10m, now).Value;

        var result = item.Update("Updated", 20m, now.AddMinutes(1));

        Assert.True(result.IsSuccess);
        Assert.Same(item, result.Value);
        Assert.Equal("Updated", item.Name);
        Assert.Equal(20m, item.Price);
        Assert.Equal(now.AddMinutes(1), item.UpdatedAt);
    }
}
EOF

cat > "$WORK/Crud/.specs/9999-crud/e2e/CatalogOverHttp.cs" <<'EOF'
using System.Net;
using System.Net.Http.Headers;
using System.Net.Http.Json;
using Crud.Api;
using Crud.Api.BuildingBlocks;
using Crud.Api.Modules.Account;
using Crud.Tests;
using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.DependencyInjection;

namespace Specs.S9999;

public class CatalogOverHttp
{
    private sealed record Written(Guid Id, Guid Version);

    private sealed record Tokens(string AccessToken);

    [Fact(DisplayName = "FM-3: another org's product is a not-found to read, list, and update")]
    public async Task Another_orgs_product_is_not_found()
    {
        await using var app = new TestApp();
        var alice = await SignedIn(app, "alice@example.com");
        var bob = await SignedIn(app, "bob@example.com");

        var lamp = await Write(await alice.PostAsJsonAsync("/catalog/products", new { name = "Lamp", price = 10m }));

        Assert.Equal(HttpStatusCode.NotFound, (await bob.GetAsync($"/catalog/products/{lamp.Id}")).StatusCode);
        Assert.DoesNotContain("Lamp", await bob.GetStringAsync("/catalog/products"), StringComparison.Ordinal);
        var takeover = await bob.PutAsJsonAsync($"/catalog/products/{lamp.Id}", new { name = "Mine", price = 1m, version = lamp.Version });
        Assert.Equal(HttpStatusCode.NotFound, takeover.StatusCode);
        Assert.Contains("Lamp", await alice.GetStringAsync("/catalog/products"), StringComparison.Ordinal);
    }

    [Fact(DisplayName = "FM-4: a write against an outdated version is a conflict and the newer change survives")]
    public async Task A_stale_version_is_a_conflict()
    {
        await using var app = new TestApp();
        var alice = await SignedIn(app, "alice@example.com");
        var lamp = await Write(await alice.PostAsJsonAsync("/catalog/products", new { name = "Lamp", price = 10m }));
        var renamed = await Write(await alice.PutAsJsonAsync($"/catalog/products/{lamp.Id}", new { name = "Desk lamp", price = 12m, version = lamp.Version }));

        var staleUpdate = await alice.PutAsJsonAsync($"/catalog/products/{lamp.Id}", new { name = "Lost", price = 1m, version = lamp.Version });
        var staleDelete = await alice.DeleteAsync($"/catalog/products/{lamp.Id}?version={lamp.Version}");

        Assert.Equal(HttpStatusCode.Conflict, staleUpdate.StatusCode);
        Assert.Contains("catalog.product_changed", await staleUpdate.Content.ReadAsStringAsync(), StringComparison.Ordinal);
        Assert.Equal(HttpStatusCode.Conflict, staleDelete.StatusCode);
        Assert.Contains("Desk lamp", await alice.GetStringAsync($"/catalog/products/{lamp.Id}"), StringComparison.Ordinal);
        (await alice.DeleteAsync($"/catalog/products/{lamp.Id}?version={renamed.Version}")).EnsureSuccessStatusCode();
    }

    [Fact(DisplayName = "FM-5: a member reads tags but cannot write them; an app admin can")]
    public async Task Only_an_app_admin_writes_app_wide_tags()
    {
        await using var app = new TestApp();
        var member = await SignedIn(app, "member@example.com");
        var admin = await SignedIn(app, "admin@example.com", asAdmin: true);

        var denied = await member.PostAsJsonAsync("/catalog/tags", new { label = "sale" });
        var sale = await Write(await admin.PostAsJsonAsync("/catalog/tags", new { label = "sale" }));
        var deniedDelete = await member.DeleteAsync($"/catalog/tags/{sale.Id}?version={sale.Version}");

        Assert.Equal(HttpStatusCode.Forbidden, denied.StatusCode);
        Assert.Equal(HttpStatusCode.Forbidden, deniedDelete.StatusCode);
        Assert.Contains("sale", await member.GetStringAsync("/catalog/tags"), StringComparison.Ordinal);
    }

    private static async Task<Written> Write(HttpResponseMessage response)
    {
        response.EnsureSuccessStatusCode();
        return (await response.Content.ReadFromJsonAsync<Written>(AppJson.Options))!;
    }

    // The operator's part of making an admin: the role is assigned out of band, never through the API.
    private static async Task<HttpClient> SignedIn(TestApp app, string email, bool asAdmin = false)
    {
        var client = app.CreateClient();
        (await client.PostAsJsonAsync("/account/register", new { email, password = "password1" })).EnsureSuccessStatusCode();
        if (asAdmin)
        {
            await using var scope = app.Services.CreateAsyncScope();
            var db = scope.ServiceProvider.GetRequiredService<AppDb>();
            var address = Email.FromStored(email);
            var user = await db.Users.IgnoreQueryFilters().SingleAsync(u => u.Email == address);
            db.Entry(user).Property(u => u.Role).CurrentValue = Role.Admin;
            await db.SaveChangesAsync();
        }
        var login = await client.PostAsJsonAsync("/account/login", new { email, password = "password1" });
        login.EnsureSuccessStatusCode();
        var tokens = (await login.Content.ReadFromJsonAsync<Tokens>(AppJson.Options))!;
        client.DefaultRequestHeaders.Authorization = new AuthenticationHeaderValue("Bearer", tokens.AccessToken);
        return client;
    }
}
EOF

package Full
doctor Full
specs Full
proofs Full
package Single
doctor Single
specs Single
proofs Single
package Crud
doctor Crud
specs Crud
proofs Crud

echo "==> auth-smoke OK"
