using Skies.Framework.AspNetCore;
using Microsoft.AspNetCore.Authentication;
using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.Http;
using Microsoft.AspNetCore.TestHost;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.Logging;
using Microsoft.Extensions.Options;
using System.Net;
using System.Text.Encodings.Web;

namespace Skies.Framework.AspNetCore.Tests;

public class CorsExtensionsTests
{
    private const string WebOrigin = "http://localhost:5173";

    [Theory]
    [InlineData("/public")]
    [InlineData("/private")]
    public async Task A_listed_origin_passes_the_preflight_with_credentials(string path)
    {
        await using var app = await Start([WebOrigin]);

        var response = await app.GetTestClient().SendAsync(Preflight(path, WebOrigin));

        Assert.True(response.IsSuccessStatusCode, $"preflight answered {(int)response.StatusCode}");
        Assert.Equal(WebOrigin, response.Headers.GetValues("Access-Control-Allow-Origin").Single());
        Assert.Equal("true", response.Headers.GetValues("Access-Control-Allow-Credentials").Single());
    }

    [Fact]
    public async Task An_unlisted_origin_gets_no_allow_origin_header()
    {
        await using var app = await Start([WebOrigin]);
        var client = app.GetTestClient();

        var preflight = await client.SendAsync(Preflight("/public", "https://evil.example"));
        var request = new HttpRequestMessage(HttpMethod.Get, "/public");
        request.Headers.Add("Origin", "https://evil.example");
        var simple = await client.SendAsync(request);

        Assert.False(preflight.Headers.Contains("Access-Control-Allow-Origin"));
        Assert.False(simple.Headers.Contains("Access-Control-Allow-Origin"));
    }

    [Fact]
    public async Task No_configured_origin_keeps_the_api_same_origin_outside_development()
    {
        await using var app = await Start([], "Production");

        var response = await app.GetTestClient().SendAsync(Preflight("/public", WebOrigin));

        Assert.False(response.Headers.Contains("Access-Control-Allow-Origin"));
    }

    [Fact]
    public async Task Development_allows_the_web_dev_server_by_default()
    {
        await using var app = await Start([], "Development");

        var response = await app.GetTestClient().SendAsync(Preflight("/private", CorsExtensions.DevelopmentWebOrigin));

        Assert.Equal(CorsExtensions.DevelopmentWebOrigin, response.Headers.GetValues("Access-Control-Allow-Origin").Single());
    }

    [Theory]
    [InlineData("*")]
    [InlineData("https://app.example.com/")]
    [InlineData("app.example.com")]
    public async Task A_wildcard_or_malformed_origin_refuses_to_start(string origin)
    {
        var failure = await Assert.ThrowsAsync<InvalidOperationException>(() => Start([origin]));

        Assert.Contains("Cors:Origins", failure.Message, StringComparison.Ordinal);
    }

    private static HttpRequestMessage Preflight(string path, string origin)
    {
        var request = new HttpRequestMessage(HttpMethod.Options, path);
        request.Headers.Add("Origin", origin);
        request.Headers.Add("Access-Control-Request-Method", "POST");
        request.Headers.Add("Access-Control-Request-Headers", "content-type,x-client");
        return request;
    }

    private static async Task<WebApplication> Start(string[] origins, string environment = "Production")
    {
        var builder = WebApplication.CreateBuilder(new WebApplicationOptions { EnvironmentName = environment });
        builder.WebHost.UseTestServer();
        for (var i = 0; i < origins.Length; i++)
            builder.Configuration[$"Cors:Origins:{i}"] = origins[i];
        builder.Services.AddSkies();
        builder.Services.AddAuthentication("deny").AddScheme<AuthenticationSchemeOptions, DenyAll>("deny", null);
        builder.Services.AddAuthorization();

        var app = builder.Build();
        try
        {
            app.UseSkies();
            app.MapPost("/public", () => Results.Ok()).AllowAnonymous();
            app.MapGet("/public", () => Results.Ok()).AllowAnonymous();
            app.MapPost("/private", () => Results.Ok()).RequireAuthorization();
            await app.StartAsync();
            return app;
        }
        catch
        {
            await app.DisposeAsync();
            throw;
        }
    }

    private sealed class DenyAll(IOptionsMonitor<AuthenticationSchemeOptions> options, ILoggerFactory logger, UrlEncoder encoder)
        : AuthenticationHandler<AuthenticationSchemeOptions>(options, logger, encoder)
    {
        protected override Task<AuthenticateResult> HandleAuthenticateAsync() => Task.FromResult(AuthenticateResult.NoResult());
    }
}
