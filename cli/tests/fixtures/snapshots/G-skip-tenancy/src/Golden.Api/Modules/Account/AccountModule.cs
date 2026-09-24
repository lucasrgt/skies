using Skies.Framework.Auth;

namespace Golden.Api.Modules.Account;

/// <summary>Registers account services and routes through the application's module registry.</summary>
[Module]
public static class AccountModule
{
    public static IServiceCollection AddServices(IServiceCollection services, IConfiguration configuration)
    {
        var jwtSecret = configuration["Jwt:Secret"]
            ?? throw new InvalidOperationException("Configure Jwt:Secret before starting the application.");
        services.AddSingleton(TimeProvider.System);
        services.AddSkiesAuth<UserSessionStore>(new SkiesAuthOptions(jwtSecret, Issuer: "golden", Audience: "golden")
        {
            RefreshCookie = new RefreshCookieOptions("golden_refresh", Path: "/account"),
        });
        services.AddAuthorization();
        return services;
    }

    public static void Map(IEndpointRouteBuilder app)
    {
        var account = app.MapGroup("/account");
        Register.Map(account);
        Login.Map(account);
        Refresh.Map(account);
        Logout.Map(account);
        Me.Map(account);
        ListMySessions.Map(account);
        RevokeSession.Map(account);
        RevokeOtherSessions.Map(account);
    }
}
