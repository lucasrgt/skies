using Skies.Framework.Auth;
using Golden.Api.Tenancy;
using Microsoft.EntityFrameworkCore;

namespace Golden.Api.Modules.Account;

/// <summary>Composition for the Account module: the database plus the framework-owned auth mechanism. One call,
/// <c>AddSkiesAuth</c>, wires the JWT minter and validator and the <see cref="ICurrentUser"/> reader (from one
/// secret/issuer/audience, so a minted token is exactly one the app accepts), the password hasher, and refresh
/// sessions over this module's <see cref="UserSessionStore"/>. Program.cs calls <c>builder.AddAccount()</c> once; the
/// routes are wired by <see cref="AccountModule.Map"/>.</summary>
public static class AccountSetup
{
    /// <summary>Register the Account module's services and authentication on <paramref name="builder"/>.</summary>
    public static void AddAccount(this WebApplicationBuilder builder)
    {
        var jwtSecret = builder.Configuration["Jwt:Secret"]
            ?? "golden-dev-jwt-secret-change-in-production-please";

        builder.Services.AddDbContext<AppDb>(options => options.UseInMemoryDatabase("golden"));
        builder.Services.ConfigureHttpJsonOptions(options => AppJson.Configure(options.SerializerOptions));
        builder.Services.AddSingleton(TimeProvider.System);
        builder.Services.AddScoped<ITenant, RequestTenant>();
        builder.Services.AddSkiesAuth<UserSessionStore>(new SkiesAuthOptions(jwtSecret, Issuer: "golden", Audience: "golden"));
        builder.Services.AddAuthorization();
    }
}
