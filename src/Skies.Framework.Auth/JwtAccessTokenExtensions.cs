using System.Text;
using Microsoft.AspNetCore.Authentication.JwtBearer;
using Microsoft.AspNetCore.Http;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Options;
using Microsoft.IdentityModel.Tokens;

namespace Skies.Framework.Auth;

/// <summary>Wires the access-token mechanism end to end in one call: the minter (<see cref="IAccessTokens"/>),
/// the per-request reader (<see cref="ICurrentUser"/> via <see cref="HttpCurrentUser"/>), and the JwtBearer
/// validator — all from the same secret / issuer / audience. The agreement between minting and validating
/// (a token this app signs is one it accepts) used to be a hand-copied foot-gun across two code sites; here
/// it is structural, because both read the same three arguments.</summary>
public static class JwtAccessTokenExtensions
{
    /// <summary>Register JWT access-token minting + validation + the <see cref="ICurrentUser"/> reader.</summary>
    /// <example>
    /// In <c>Program.cs</c>, before <c>builder.Build()</c>:
    /// <code>
    /// builder.Services.AddJwtAccessTokens(jwtSecret, issuer: "myapp", audience: "myapp");
    /// </code>
    /// Then the usual <c>app.UseAuthentication(); app.UseAuthorization();</c> in the pipeline.
    /// </example>
    /// <remarks>The host refuses to start when <paramref name="secret"/> is missing, and, outside Development, when it
    /// is shorter than <see cref="SkiesAuthOptions.MinSecretBytes"/> bytes or a development key
    /// (<see cref="SkiesAuthOptions.DevelopmentSecret"/>). The check runs at host start, and its message names what to
    /// configure.</remarks>
    public static IServiceCollection AddJwtAccessTokens(this IServiceCollection services, string secret, string issuer, string audience)
    {
        services.AddSingleton<IValidateOptions<JwtBearerOptions>>(sp =>
            new JwtSecretPolicy(secret, sp.GetService<IHostEnvironment>()));
        services.AddOptions<JwtBearerOptions>(JwtBearerDefaults.AuthenticationScheme).ValidateOnStart();
        services.AddHttpContextAccessor();
        services.AddScoped<ICurrentUser, HttpCurrentUser>();
        services.AddSingleton<IAccessTokens>(sp => new AccessTokens(secret, issuer, audience, sp.GetRequiredService<TimeProvider>()));

        services
            .AddAuthentication(JwtBearerDefaults.AuthenticationScheme)
            .AddJwtBearer(options =>
            {
                options.MapInboundClaims = false;   // else "sub" is remapped to NameIdentifier and the claims break
                // A missing secret has no key to build; JwtSecretPolicy then refuses the start with what to configure.
                if (!string.IsNullOrEmpty(secret))
                    options.TokenValidationParameters = BuildValidationParameters(secret, issuer, audience);
            });

        return services;
    }

    /// <summary>The exact parameters the JwtBearer validator runs with — extracted so the agreement with
    /// <see cref="AccessTokens.Issue"/> (these tokens' claim names, signing alg, lifetime) is one testable
    /// place, not a hand-copied options lambda.</summary>
    /// <remarks>
    /// Three guards beyond the obvious issuer/audience/key checks:
    /// <list type="bullet">
    /// <item><description><c>RoleClaimType</c>/<c>NameClaimType</c> are pinned to the <c>role</c>/<c>name</c>
    /// claims this library mints. With <c>MapInboundClaims = false</c> and no mapping, the defaults
    /// (<c>ClaimTypes.Role</c>/<c>ClaimTypes.Name</c>) point at claim types the token never carries, so
    /// idiomatic <c>[Authorize(Roles = …)]</c>, <c>User.IsInRole(…)</c> and <c>User.Identity!.Name</c> would
    /// silently see nothing — a stranger-maintainable .NET app must just work.</description></item>
    /// <item><description><c>ValidAlgorithms</c> pins HS256: the validator never accepts a token signed with
    /// any other algorithm, closing algorithm-substitution off by construction rather than by the key type
    /// happening to be symmetric.</description></item>
    /// <item><description><c>ClockSkew</c> is 30 seconds, not the 5-minute default. A 15-minute token with a
    /// 5-minute skew lives ~20 minutes and stays accepted long after it "expired"; 30 seconds tolerates
    /// ordinary server clock drift without quietly tripling the token's effective window.</description></item>
    /// </list>
    /// </remarks>
    public static TokenValidationParameters BuildValidationParameters(string secret, string issuer, string audience) =>
        new()
        {
            ValidIssuer = issuer,
            ValidAudience = audience,
            IssuerSigningKey = new SymmetricSecurityKey(Encoding.UTF8.GetBytes(secret)),
            ValidateIssuer = true,
            ValidateAudience = true,
            ValidateLifetime = true,
            ValidateIssuerSigningKey = true,
            RequireSignedTokens = true,
            RequireExpirationTime = true,
            ValidAlgorithms = [SecurityAlgorithms.HmacSha256],
            RoleClaimType = "role",
            NameClaimType = "name",
            ClockSkew = TimeSpan.FromSeconds(30),
        };

    /// <summary>Register the web refresh-cookie delivery service with the app's cookie name and path. The
    /// optional <paramref name="domain"/> and <paramref name="sameSite"/> stay at the strict, host-only secure
    /// default; widen them only for a real multi-subdomain case (a cookie shared across sibling subdomains).
    /// httpOnly and Secure are never tunable.</summary>
    /// <example>
    /// <code>
    /// builder.Services.AddRefreshCookie("myapp_refresh", path: "/account");
    /// // multi-subdomain: a cookie that rides across *.example.com on a cross-persona hop
    /// builder.Services.AddRefreshCookie("myapp_refresh", domain: ".example.com", sameSite: SameSiteMode.Lax);
    /// </code>
    /// A slice's route then takes <c>RefreshCookie cookies</c> by DI and calls
    /// <c>cookies.IsWeb / RefreshFrom / SetRefresh / Clear</c>.
    /// </example>
    public static IServiceCollection AddRefreshCookie(
        this IServiceCollection services,
        string name,
        string path = "/",
        string? domain = null,
        SameSiteMode sameSite = SameSiteMode.Strict)
    {
        services.AddSingleton(new RefreshCookieOptions(name, path, domain, sameSite));
        services.AddSingleton<RefreshCookie>();
        return services;
    }
}
