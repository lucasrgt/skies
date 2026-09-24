using Skies.Framework.AspNetCore;
using Sample.Api;
using Sample.Api.Modules;
using Sample.Api.Modules.Wallets;

var builder = WebApplication.CreateBuilder(args);
builder.Services.AddSkies();                        // framework conventions: slice-aware OpenAPI + enum-as-name JSON
builder.Services.AddPlatform(builder.Configuration); // the app's cross-cutting infra (here: the demo store)
builder.Services.AddModules(builder.Configuration);  // each module's own services (the explicit registry)

var app = builder.Build();

app.UseSkies();    // serve the OpenAPI contract at /openapi/v1.json
app.MapModules();   // each module's routes (the explicit registry)

// Boot: seed demo data so the sample reads non-empty on first run. `OnStartup` runs it in a scope on a real boot
// but skips it when `dotnet build` is only emitting the OpenAPI contract — so the build needs no database. The
// seed content lives in the module that owns it (WalletsModule.Seed).
app.OnStartup(sp => WalletsModule.Seed(sp.GetRequiredService<AppDb>()));

app.Run();

// Exposed so WebApplicationFactory<Program> can boot the real app in the spec E2E.
public partial class Program { }
