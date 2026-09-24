using Golden.Api.Modules.Account;
using Golden.Api.Modules;

var builder = WebApplication.CreateBuilder(args);
builder.Services.AddSkies();                        // framework conventions: slice-aware OpenAPI + enum-as-name JSON
builder.Services.AddModules(builder.Configuration);  // each module's own services (the explicit registry)

builder.AddAccount();

var app = builder.Build();

app.UseSkies();    // serve the OpenAPI contract at /openapi/v1.json
app.MapModules();   // each module's routes (the explicit registry)

AccountModule.Map(app);

app.Run();

// Exposed so WebApplicationFactory<Program> can boot the real app in the spec E2E.
public partial class Program { }
