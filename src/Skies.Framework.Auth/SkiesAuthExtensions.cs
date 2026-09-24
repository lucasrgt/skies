using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.DependencyInjection.Extensions;

namespace Skies.Framework.Auth;

/// <summary>The auth mechanism's settings for <see cref="SkiesAuthExtensions.AddSkiesAuth{TSessionStore}"/>.</summary>
/// <param name="JwtSecret">The HMAC key access tokens are signed and validated with (at least 32 bytes).</param>
/// <param name="Issuer">The access tokens' issuer, minted and required.</param>
/// <param name="Audience">The access tokens' audience, minted and required.</param>
public sealed record SkiesAuthOptions(string JwtSecret, string Issuer, string Audience)
{
    /// <summary>Web refresh-cookie delivery: the cookie's name and path (and, for a multi-subdomain app, its domain
    /// and SameSite). <see langword="null"/> means body-only delivery and no <see cref="RefreshCookie"/> service.
    /// Its <see cref="RefreshCookieOptions.Lifetime"/> is overridden with <see cref="Sessions"/>' lifetime.</summary>
    public RefreshCookieOptions? RefreshCookie { get; init; }

    /// <summary>How long a login lasts (token lifetime and absolute family ceiling).</summary>
    public RefreshSessionOptions Sessions { get; init; } = RefreshSessionOptions.Default;
}

/// <summary>The one registration call an Account module makes for the auth mechanism. It wires, from one options
/// value: the JWT minter, validator, and <see cref="ICurrentUser"/> reader (<see
/// cref="JwtAccessTokenExtensions.AddJwtAccessTokens"/>), the <see cref="IPasswordHasher"/>, refresh sessions over the
/// app's own <see cref="IRefreshSessionStore"/>, and, when asked, the <see cref="RefreshCookie"/>. Explicit on purpose:
/// the app names its store type here, nothing is discovered.</summary>
public static class SkiesAuthExtensions
{
    /// <summary>Register the auth mechanism, with <typeparamref name="TSessionStore"/> (the app's adapter over its
    /// session table) behind <see cref="RefreshSessions"/>.</summary>
    /// <example>
    /// In the Account module's composition:
    /// <code>
    /// builder.Services.AddSkiesAuth&lt;UserSessionStore&gt;(new SkiesAuthOptions(jwtSecret, Issuer: "myapp", Audience: "myapp")
    /// {
    ///     RefreshCookie = new RefreshCookieOptions("myapp_refresh", Path: "/account"),
    /// });
    /// </code>
    /// </example>
    public static IServiceCollection AddSkiesAuth<TSessionStore>(this IServiceCollection services, SkiesAuthOptions options)
        where TSessionStore : class, IRefreshSessionStore
    {
        ArgumentNullException.ThrowIfNull(options);
        ArgumentOutOfRangeException.ThrowIfLessThanOrEqual(options.Sessions.Lifetime, TimeSpan.Zero, nameof(options));
        ArgumentOutOfRangeException.ThrowIfLessThanOrEqual(options.Sessions.FamilyMaxAge, TimeSpan.Zero, nameof(options));

        services.TryAddSingleton(TimeProvider.System);
        services.AddJwtAccessTokens(options.JwtSecret, options.Issuer, options.Audience);
        services.TryAddSingleton<IPasswordHasher, Argon2idPasswordHasher>();
        services.AddSingleton(options.Sessions);
        services.AddScoped<IRefreshSessionStore, TSessionStore>();
        services.AddScoped<RefreshSessions>();
        if (options.RefreshCookie is { } cookie)
        {
            services.AddSingleton(cookie with { Lifetime = options.Sessions.Lifetime });
            services.AddSingleton<RefreshCookie>();
        }
        return services;
    }

    /// <summary>Register single-use verification secrets (email links, SMS codes) over <typeparamref name="TStore"/>,
    /// the app's adapter over its verification table. The phone and email flows share it, so an app calls it once.</summary>
    public static IServiceCollection AddVerificationTokens<TStore>(this IServiceCollection services, VerificationOptions? options = null)
        where TStore : class, IVerificationStore
    {
        services.TryAddSingleton(TimeProvider.System);
        services.TryAddSingleton<IPasswordHasher, Argon2idPasswordHasher>();
        services.AddSingleton(options ?? VerificationOptions.Default);
        services.AddScoped<IVerificationStore, TStore>();
        services.AddScoped<VerificationTokens>();
        return services;
    }
}
