using System.Linq;
using System.Text.Json;
using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.Mvc.Testing;
using Skies.Framework.AspNetCore;
using Sample.Tests;

namespace Specs.S0005;

// The error-code contract: the framework enumerates every *ErrorCodes constant the app owns into ErrorBody.code
// (SKY0018 keeps the registries the complete set), so the generated client is typed on the closed set of codes and
// the frontend can be checked for an exhaustive translation of each.
public class ErrorCodeContractSpec
{
    [Fact(DisplayName = "FM-1: ErrorBody.code enumerates the app's registry codes")]
    public async Task ErrorBody_code_enumerates_the_registries()
    {
        await using var app = new TestApp();

        var codes = await ErrorCodes(app);

        Assert.Contains("wallets.not_found", codes);
        Assert.Contains("wallets.insufficient_funds", codes);
        Assert.Contains("wallet.id.required", codes);
        Assert.Contains("money.negative", codes);
        Assert.Contains(PlatformErrorCodes.RateLimited, codes);
    }

    [Fact(DisplayName = "FM-2: a dependency loaded in the process does not widen the contract")]
    public async Task A_loaded_dependency_does_not_widen_the_contract()
    {
        await using var app = new TestApp();

        var codes = await ErrorCodes(app);

        Assert.Contains("wallets.not_found", codes);
        Assert.DoesNotContain(DependencyErrorCodes.UniqueViolation, codes);
    }

    [Fact(DisplayName = "FM-2: an explicitly registered assembly is enumerated once, beside the app's own codes")]
    public async Task An_explicitly_registered_assembly_is_enumerated_once()
    {
        await using var app = new TestApp();
        await using var modularApp = app.WithWebHostBuilder(builder => builder.ConfigureServices(services =>
            services.AddSkiesOpenApi(typeof(DependencyErrorCodes).Assembly)));

        var codes = await ErrorCodes(modularApp);

        Assert.Contains(DependencyErrorCodes.UniqueViolation, codes);
        Assert.Contains("wallets.not_found", codes);
        Assert.Contains(PlatformErrorCodes.RateLimited, codes);
        Assert.Equal(codes.Count, codes.Distinct().Count());
    }

    private static async Task<List<string?>> ErrorCodes(WebApplicationFactory<Program> app)
    {
        using var client = app.CreateClient();
        using var document = JsonDocument.Parse(await client.GetStringAsync("/openapi/v1.json"));
        return document.RootElement
            .GetProperty("components").GetProperty("schemas")
            .GetProperty("ErrorBody").GetProperty("properties")
            .GetProperty("code").GetProperty("enum")
            .EnumerateArray().Select(value => value.GetString()).ToList();
    }

    // Loaded with the test process, but not owned by the hosted application. A vendor such as Npgsql also exposes
    // *ErrorCodes constants; loading that library must not widen the API.
    internal static class DependencyErrorCodes
    {
        public const string UniqueViolation = "23505";
    }
}
