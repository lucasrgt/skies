using Skies.Framework.Mail;
using Skies.Framework.Identity;
using Skies.Framework.Sms;
using Microsoft.EntityFrameworkCore;

namespace Golden.Api;

/// <summary>Owns the database, external providers, and the request pipeline's cross-cutting middleware. Replace the
/// local providers when deploying this app.</summary>
public static class Platform
{
    public static IServiceCollection AddPlatform(this IServiceCollection services, IConfiguration configuration,
        IHostEnvironment environment)
    {
        // `dotnet build` also boots the app, only to write the OpenAPI contract; that boot never opens the database.
        if (!environment.IsDevelopment())
        {
            if (!SkiesExtensions.IsGeneratingOpenApiDocument)
                throw new InvalidOperationException(
                    "Configure a persistent AppDb provider in Platform.AddPlatform before running outside Development.");
        }

        configuration["Jwt:Secret"] ??= "golden-local-development-key-not-for-deployment";
        services.AddDbContext<AppDb>(options => options.UseInMemoryDatabase("golden"));
        services.ConfigureHttpJsonOptions(options => AppJson.Configure(options.SerializerOptions));
        services.AddSingleton<ISmsSender, ConsoleSmsSender>();
        services.AddSingleton<IExternalIdentityVerifier, FakeExternalIdentity>();
        services.AddSingleton<IEmailSender, ConsoleEmailSender>();
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
