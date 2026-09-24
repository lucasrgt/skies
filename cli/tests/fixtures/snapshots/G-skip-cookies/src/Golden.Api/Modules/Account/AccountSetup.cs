using Skies.Framework.Auth;
using Golden.Api.Tenancy;
using Microsoft.EntityFrameworkCore;

namespace Golden.Api.Modules.Account;

/// <summary>Composition for the Account module: the database plus the framework-owned auth mechanism. One
/// call — <c>AddJwtAccessTokens</c> — wires the token minter (<see cref="IAccessTokens"/>), the
/// <see cref="ICurrentUser"/> reader, and the JwtBearer validator from the same secret/issuer/audience, so
/// a minted token is exactly one the app accepts. Program.cs calls <c>builder.AddAccount()</c> once; the
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
        builder.Services.AddJwtAccessTokens(jwtSecret, issuer: "golden", audience: "golden");
        builder.Services.AddAuthorization();
    }
}
