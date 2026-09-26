using System;
using System.Reflection;
using System.Text.Json.Serialization;
using Microsoft.AspNetCore.Builder;
using Microsoft.Extensions.DependencyInjection;

namespace Skies.Framework.AspNetCore;

/// <summary>
/// The framework's composition-root wiring in one pair of calls, so an app's <c>Program.cs</c> reads as a thin
/// index rather than re-deriving Skies's conventions every time. <see cref="AddSkies"/> registers what every
/// Skies app shares — the slice-aware OpenAPI document (<see cref="OpenApiExtensions.AddSkiesOpenApi(IServiceCollection)"/>) and
/// the enum-as-name JSON convention; <see cref="UseSkies"/> serves the contract. Auth, persistence, and vendor
/// adapters stay the app's own calls — the framework ships the conventions, not the app's choices.
/// </summary>
public static class SkiesExtensions
{
    /// <summary>Register Skies's universal conventions: the slice-aware OpenAPI document, enum-as-name JSON, and the
    /// CORS services <see cref="CorsExtensions.UseSkiesCors"/> applies when origins are configured.</summary>
    public static IServiceCollection AddSkies(this IServiceCollection services)
    {
        services.AddSkiesOpenApi();
        services.AddCors();
        // Enums cross the wire as their names ("host", not 0) — readable, and stable against reordering.
        services.ConfigureHttpJsonOptions(options =>
            options.SerializerOptions.Converters.Add(new JsonStringEnumConverter()));
        return services;
    }

    /// <summary>Allow the configured cross-origin web clients (<see cref="CorsExtensions.UseSkiesCors"/>), then serve
    /// Skies's framework endpoints — the OpenAPI document at <c>/openapi/v1.json</c>, the typed contract a client
    /// generates from.</summary>
    public static WebApplication UseSkies(this WebApplication app)
    {
        app.UseSkiesCors();
        app.MapOpenApi();
        return app;
    }

    /// <summary>
    /// <see langword="true"/> when the process is the build-time OpenAPI document generator
    /// (<c>GetDocument.Insider</c>, run by <c>dotnet build</c>) rather than a real server boot. The generator
    /// builds the host and runs <c>Program.cs</c> through to <c>app.Run()</c>, so any unguarded migrate/seed would
    /// otherwise demand a database the build does not have. Guard such startup side effects with this flag — or use
    /// <see cref="OnStartup"/> — so the contract emits at build time with no running database.
    /// </summary>
    public static bool IsGeneratingOpenApiDocument =>
        Assembly.GetEntryAssembly()?.GetName().Name == "GetDocument.Insider";

    /// <summary>
    /// Run a startup side effect — a database migration, data seeding — inside a fresh service scope, but skip it
    /// when the process is only generating the OpenAPI document at build time
    /// (<see cref="IsGeneratingOpenApiDocument"/>). This is what lets <c>dotnet build</c> emit the contract with no
    /// database while a real boot still migrates and seeds. The persistence types stay in the app: the framework
    /// owns the doc-gen awareness and the scope, the caller's <paramref name="task"/> owns what actually runs.
    /// </summary>
    /// <param name="app">The built application.</param>
    /// <param name="task">The side effect to run on a real boot, given a scoped <see cref="IServiceProvider"/>.</param>
    public static WebApplication OnStartup(this WebApplication app, Action<IServiceProvider> task)
    {
        if (!IsGeneratingOpenApiDocument)
        {
            using var scope = app.Services.CreateScope();
            task(scope.ServiceProvider);
        }
        return app;
    }
}
