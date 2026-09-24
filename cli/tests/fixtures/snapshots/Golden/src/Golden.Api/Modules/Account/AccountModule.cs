using Skies.Framework.Auth;
using Golden.Api.Tenancy;

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
        services.AddScoped<ITenant, RequestTenant>();
        services.AddSkiesAuth<UserSessionStore>(new SkiesAuthOptions(jwtSecret, Issuer: "golden", Audience: "golden")
        {
            RefreshCookie = new RefreshCookieOptions("golden_refresh", Path: "/account"),
        });
        services.AddAuthorization();
        services.AddVerificationTokens<VerificationTokenStore>();
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
        ResendPhoneCode.Map(account);
        VerifyPhone.Map(account);
        RegisterWithGoogle.Map(account);
        LoginWithGoogle.Map(account);
        RequestEmailVerification.Map(account);
        VerifyEmail.Map(account);
        RequestPasswordReset.Map(account);
        ResetPassword.Map(account);
    }
}
