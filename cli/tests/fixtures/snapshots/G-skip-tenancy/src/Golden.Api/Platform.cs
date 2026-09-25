using Microsoft.EntityFrameworkCore;
using Skies.Framework.Auth;

namespace Golden.Api;

/// <summary>Owns the database, external providers, and the request pipeline's cross-cutting middleware. Replace the
/// local providers when deploying this app.</summary>
public static class Platform
{
    public static IServiceCollection AddPlatform(this IServiceCollection services, IConfiguration configuration,
        IHostEnvironment environment)
    {
        // `dotnet build` also boots the app, only to write the OpenAPI contract; that boot never opens the database.
        if (environment.IsDevelopment() || SkiesExtensions.IsGeneratingOpenApiDocument)
        {
            // Local only: a signing key every copy of this source shares, a store that forgets on restart, and
            // providers that print instead of sending. Outside Development, Jwt:Secret comes from configuration, and
            // the auth package refuses to start on a missing, short, or development key.
            configuration["Jwt:Secret"] ??= SkiesAuthOptions.DevelopmentSecret;
            services.AddDbContext<AppDb>(options => options.UseInMemoryDatabase("golden"));
        }
        else
        {
            throw new InvalidOperationException(
                "Configure a persistent AppDb provider in Platform.AddPlatform before running outside Development.");
        }

        services.ConfigureHttpJsonOptions(options => AppJson.Configure(options.SerializerOptions));
        return services;
    }

    public static WebApplication UsePlatform(this WebApplication app)
    {
        // Runs the throttles route groups require, such as the Account module's credential endpoints. Behind a proxy,
        // call UseForwardedHeaders first so each client keeps its own address.
        app.UseRateLimiter();
        return app;
    }
}
