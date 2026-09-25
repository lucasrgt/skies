using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.TestHost;
using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.DependencyInjection;
using Golden.Api;
using Golden.Api.Modules;
using Golden.Tests;
using Skies.Framework.Auth;

namespace Specs.S0001;

public class Deployment
{
    [Fact(DisplayName = "FM-27: local providers cannot start in Production")]
    public void Production_requires_configuration() => CannotStart("Production");

    [Fact(DisplayName = "FM-27: local providers cannot start in Staging")]
    public void Staging_requires_configuration() => CannotStart("Staging");

    // The app as deployed once the owner has replaced Platform's development branch with a real store: nothing local
    // is left to supply a key, so the signing secret must come from configuration and be one of the app's own.
    [Fact(DisplayName = "FM-28: outside Development the app refuses to start without a signing secret of its own")]
    public async Task Production_refuses_a_missing_short_or_public_secret()
    {
        foreach (var secret in new[] { null, "too-short-to-sign", SkiesAuthOptions.DevelopmentSecret })
        {
            var refused = await Record.ExceptionAsync(() => StartDeployed(secret));
            Assert.NotNull(refused);
            Assert.Contains("Jwt:Secret", refused.Message, StringComparison.Ordinal);
        }

        await StartDeployed("a-deployment-secret-of-well-over-thirty-two-random-bytes");   // a real secret starts
    }

    private static void CannotStart(string environment)
    {
        using var app = new TestApp().WithWebHostBuilder(builder => builder.UseEnvironment(environment));
        var error = Assert.Throws<InvalidOperationException>(() => app.CreateClient());
        Assert.Contains("Configure a persistent AppDb provider", error.Message, StringComparison.Ordinal);
    }

    // Production, the modules' own registrations, and a stand-in for the persistent store, started on a test server.
    private static async Task StartDeployed(string? secret)
    {
        var builder = WebApplication.CreateBuilder(new WebApplicationOptions { EnvironmentName = "Production" });
        builder.WebHost.UseTestServer();
        builder.Configuration["Jwt:Secret"] = secret;
        builder.Services.AddDbContext<AppDb>(options => options.UseInMemoryDatabase(Guid.NewGuid().ToString()));
        builder.Services.AddModules(builder.Configuration);
        await using var app = builder.Build();
        await app.StartAsync();
        await app.StopAsync();
    }
}
