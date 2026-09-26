using Skies.Framework.Auth;
using Skies.Framework.EntityFrameworkCore;
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
        services.AddAuthorizationBuilder()
            .AddPolicy(AppPolicies.AppAdmin, policy => policy.RequireRole(nameof(Role.Admin)));
        CredentialRateLimit.AddTo(services, configuration);
        services.AddSingleton<IAccountNotices, EmailAccountNotices>();
        services.AddVerificationTokens<VerificationTokenStore>();
        return services;
    }

    public static void Map(IEndpointRouteBuilder app)
    {
        var account = app.MapGroup("/account");
        // Everything that takes a credential or sends a message is throttled per client (CredentialRateLimit).
        var credentials = account.MapGroup("").RequireRateLimiting(CredentialRateLimit.Policy);
        Register.Map(credentials);
        Login.Map(credentials);
        Refresh.Map(credentials);
        Logout.Map(credentials);
        Me.Map(account);
        ListMySessions.Map(account);
        RevokeSession.Map(account);
        RevokeOtherSessions.Map(account);
        ResendPhoneCode.Map(credentials);
        VerifyPhone.Map(credentials);
        RegisterWithGoogle.Map(credentials);
        LoginWithGoogle.Map(credentials);
        RequestEmailVerification.Map(credentials);
        VerifyEmail.Map(credentials);
        RequestPasswordReset.Map(credentials);
        ResetPassword.Map(credentials);
    }
}
