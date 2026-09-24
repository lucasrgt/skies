using Golden.Api;
using Golden.Api.Modules;

var builder = WebApplication.CreateBuilder(args);
builder.Services.AddSkies();                        // framework conventions: slice-aware OpenAPI + enum-as-name JSON
builder.Services.AddPlatform(builder.Configuration, builder.Environment);
builder.Services.AddModules(builder.Configuration);  // each module's own services (the explicit registry)

var app = builder.Build();

app.UsePlatform();  // the platform's middleware: the rate limiter the modules' throttles need

app.UseSkies();    // serve the OpenAPI contract at /openapi/v1.json
app.MapModules();   // each module's routes (the explicit registry)

app.Run();

// Exposed so WebApplicationFactory<Program> can boot the real app in the spec E2E.
public partial class Program { }
