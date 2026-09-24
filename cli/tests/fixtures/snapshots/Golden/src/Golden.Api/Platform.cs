using Skies.Framework.Mail;
using Skies.Framework.Identity;
using Skies.Framework.Sms;
using Microsoft.EntityFrameworkCore;

namespace Golden.Api;

/// <summary>Owns the database and external providers. Replace the local providers when deploying this app.</summary>
public static class Platform
{
    public static IServiceCollection AddPlatform(this IServiceCollection services, IConfiguration configuration,
        IHostEnvironment environment)
    {
        if (!environment.IsDevelopment())
            throw new InvalidOperationException(
                "Configure a persistent AppDb provider in Platform.AddPlatform before running outside Development.");

        configuration["Jwt:Secret"] ??= "golden-local-development-key-not-for-deployment";
        services.AddDbContext<AppDb>(options => options.UseInMemoryDatabase("golden"));
        services.ConfigureHttpJsonOptions(options => AppJson.Configure(options.SerializerOptions));
        services.AddSingleton<ISmsSender, ConsoleSmsSender>();
        services.AddSingleton<IExternalIdentityVerifier, FakeExternalIdentity>();
        services.AddSingleton<IEmailSender, ConsoleEmailSender>();
        return services;
    }
}
