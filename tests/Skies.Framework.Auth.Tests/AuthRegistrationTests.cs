using Microsoft.AspNetCore.Http;
using Microsoft.Extensions.DependencyInjection;
using Skies.Framework.Auth;

namespace Skies.Framework.Auth.Tests;

// AddSkiesAuth is the one call a module makes, so it must wire everything a slice asks for from the same options,
// and the refresh cookie must deliver with the session lifetime it was given, never a second hand-copied number.
public class AuthRegistrationTests
{
    private const string Secret = "test-secret-for-jwt-signing-please-64-chars-long-enough-for-hs512";

    private static ServiceProvider Build(Action<IServiceCollection> configure)
    {
        var services = new ServiceCollection().AddLogging();
        configure(services);
        return services.BuildServiceProvider(new ServiceProviderOptions { ValidateScopes = true, ValidateOnBuild = true });
    }

    [Fact]
    public void Add_skies_auth_wires_tokens_hashing_sessions_and_the_cookie()
    {
        var sessions = new RefreshSessionOptions { Lifetime = TimeSpan.FromDays(3) };
        using var provider = Build(services => services.AddSkiesAuth<ListRefreshStore>(
            new SkiesAuthOptions(Secret, "myapp", "myapp") { RefreshCookie = new("myapp_refresh", "/account"), Sessions = sessions }));
        using var scope = provider.CreateScope();

        Assert.IsType<Argon2idPasswordHasher>(scope.ServiceProvider.GetRequiredService<IPasswordHasher>());
        Assert.NotNull(scope.ServiceProvider.GetRequiredService<IAccessTokens>());
        Assert.NotNull(scope.ServiceProvider.GetRequiredService<ICurrentUser>());
        Assert.Same(sessions, scope.ServiceProvider.GetRequiredService<RefreshSessions>().Options);
        Assert.Equal(TimeSpan.FromDays(3), scope.ServiceProvider.GetRequiredService<RefreshCookieOptions>().Lifetime);
        Assert.NotNull(scope.ServiceProvider.GetRequiredService<RefreshCookie>());
    }

    [Fact]
    public void Without_a_cookie_there_is_no_cookie_service()
    {
        using var provider = Build(services => services.AddSkiesAuth<ListRefreshStore>(new SkiesAuthOptions(Secret, "myapp", "myapp")));

        Assert.Null(provider.GetService<RefreshCookie>());
    }

    [Fact]
    public void A_non_positive_lifetime_is_refused_at_registration()
    {
        var options = new SkiesAuthOptions(Secret, "myapp", "myapp") { Sessions = new() { Lifetime = TimeSpan.Zero } };

        Assert.Throws<ArgumentOutOfRangeException>(() => new ServiceCollection().AddSkiesAuth<ListRefreshStore>(options));
    }

    [Fact]
    public void An_app_hasher_registered_first_is_kept()
    {
        var mine = new Argon2idPasswordHasher();
        using var provider = Build(services =>
        {
            services.AddSingleton<IPasswordHasher>(mine);
            services.AddSkiesAuth<ListRefreshStore>(new SkiesAuthOptions(Secret, "myapp", "myapp"));
            services.AddVerificationTokens<ListVerificationStore>();
        });

        Assert.Same(mine, provider.GetRequiredService<IPasswordHasher>());
        using var scope = provider.CreateScope();
        Assert.NotNull(scope.ServiceProvider.GetRequiredService<VerificationTokens>());
    }

    [Fact]
    public async Task Deliver_plants_the_cookie_for_the_web_and_keeps_the_token_out_of_its_body()
    {
        var clock = new ManualClock();
        var cookie = new RefreshCookie(new RefreshCookieOptions("app_refresh", "/account") { Lifetime = TimeSpan.FromDays(3) }, clock);
        var http = Context(web: true);

        var result = cookie.Deliver(http, "the-token", new { accessToken = "a", refreshToken = "the-token" }, new { accessToken = "a" });
        await result.ExecuteAsync(http);

        var setCookie = http.Response.Headers.SetCookie.ToString();
        Assert.Contains("app_refresh=the-token", setCookie, StringComparison.Ordinal);
        Assert.Contains("httponly", setCookie, StringComparison.OrdinalIgnoreCase);
        Assert.Contains("path=/account", setCookie, StringComparison.OrdinalIgnoreCase);
        Assert.Contains(clock.Now.AddDays(3).ToString("R"), setCookie, StringComparison.OrdinalIgnoreCase);
        Assert.DoesNotContain("refreshToken", await Body(http), StringComparison.Ordinal);
    }

    [Fact]
    public async Task Deliver_gives_a_non_web_client_the_full_body_and_no_cookie()
    {
        var cookie = new RefreshCookie(new RefreshCookieOptions("app_refresh"));
        var http = Context(web: false);

        await cookie.Deliver(http, "the-token", new { refreshToken = "the-token" }, new { accessToken = "a" }).ExecuteAsync(http);

        Assert.Equal(0, http.Response.Headers.SetCookie.Count);
        Assert.Contains("the-token", await Body(http), StringComparison.Ordinal);
    }

    private static DefaultHttpContext Context(bool web)
    {
        var http = new DefaultHttpContext
        {
            RequestServices = new ServiceCollection().AddLogging().BuildServiceProvider(),
        };
        http.Request.Scheme = "https";
        http.Request.Host = new HostString("app.example.com");
        if (web)
            http.Request.Headers["X-Client"] = "web";
        http.Response.Body = new MemoryStream();
        return http;
    }

    private static async Task<string> Body(HttpContext http)
    {
        http.Response.Body.Position = 0;
        return await new StreamReader(http.Response.Body).ReadToEndAsync();
    }
}
