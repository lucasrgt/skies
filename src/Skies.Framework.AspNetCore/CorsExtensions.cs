using System;
using System.Linq;
using Microsoft.AspNetCore.Builder;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.Hosting;

namespace Skies.Framework.AspNetCore;

/// <summary>
/// Cross-origin access for a web client served from another origin than the API. The allowed origins are
/// configuration: <c>Cors:Origins</c> lists them exactly (scheme, host, port). With no list, Development allows the
/// generated web app's dev server (<see cref="DevelopmentWebOrigin"/>) and every other environment stays
/// same-origin.
/// </summary>
public static class CorsExtensions
{
    /// <summary>The configuration key holding the allowed origins, e.g. <c>Cors__Origins__0=https://app.example.com</c>.</summary>
    public const string OriginsKey = "Cors:Origins";

    /// <summary>The origin a generated web app's dev server listens on (Vite, its port pinned in vite.config.ts).</summary>
    public const string DevelopmentWebOrigin = "http://localhost:5173";

    /// <summary>
    /// Allow the configured origins to call this API with credentials (the refresh cookie) and any header or
    /// method, so the browser's preflight succeeds. A wildcard is refused at startup: a credentialed wildcard would
    /// let any site act with the visitor's session.
    /// </summary>
    /// <param name="app">The built application.</param>
    /// <exception cref="InvalidOperationException">A configured origin is <c>*</c> or not an absolute http(s) origin.</exception>
    public static WebApplication UseSkiesCors(this WebApplication app)
    {
        var origins = app.Configuration.GetSection(OriginsKey).Get<string[]>() ?? [];
        if (origins.Length == 0 && app.Environment.IsDevelopment())
            origins = [DevelopmentWebOrigin];
        if (origins.Length == 0)
            return app;

        foreach (var origin in origins)
        {
            var valid = Uri.TryCreate(origin, UriKind.Absolute, out var uri)
                && (uri.Scheme == Uri.UriSchemeHttps || uri.Scheme == Uri.UriSchemeHttp)
                && uri.AbsolutePath == "/" && !origin.EndsWith('/');
            if (!valid)
                throw new InvalidOperationException(
                    $"{OriginsKey} holds '{origin}'. List exact origins such as https://app.example.com: no wildcard, " +
                    "path, or trailing slash, because the API allows them with credentials.");
        }

        app.UseCors(policy => policy
            .WithOrigins(origins.ToArray())
            .AllowCredentials()
            .AllowAnyHeader()
            .AllowAnyMethod());
        return app;
    }
}
