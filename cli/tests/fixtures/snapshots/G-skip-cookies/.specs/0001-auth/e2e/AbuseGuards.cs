using System.Net;
using System.Net.Http.Json;
using System.Security.Claims;
using Microsoft.AspNetCore.Authorization;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.IdentityModel.JsonWebTokens;
using Golden.Api;
using Golden.Api.Modules.Account;
using Golden.Tests;

namespace Specs.S0001;

/// <summary>The guards against abuse rather than against a wrong credential: the throttle on guessing, and the role
/// no one can grant themselves.</summary>
public class AbuseGuards
{
    // The window is a minute by default and the cases run in well under one, so the count is deterministic.
    [Fact(DisplayName = "FM-11: past the permitted attempts, login answers 429 with the platform error and a retry window")]
    public async Task Login_is_throttled_per_client()
    {
        await using var app = new TestApp();
        var client = app.CreateClient();
        var limit = app.Services.GetRequiredService<CredentialRateLimit>();

        for (var attempt = 0; attempt < limit.PermitLimit; attempt++)
            Assert.Equal(HttpStatusCode.Unauthorized, (await Guess(client)).StatusCode);
        var throttled = await Guess(client);

        Assert.Equal(HttpStatusCode.TooManyRequests, throttled.StatusCode);
        Assert.Contains("platform.rate_limited", await throttled.Content.ReadAsStringAsync(), StringComparison.Ordinal);
        Assert.NotNull(throttled.Headers.RetryAfter);
        var profile = await client.GetAsync("/account/me");
        Assert.Equal(HttpStatusCode.Unauthorized, profile.StatusCode);
    }

    [Fact(DisplayName = "FM-14: a registered account's token does not satisfy the app-admin policy; an Admin's does")]
    public async Task Registration_never_grants_app_admin()
    {
        await using var app = new TestApp();
        var client = app.CreateClient();
        var tokens = await AuthApi.SignUp(client, "member@example.com");
        var authorization = app.Services.GetRequiredService<IAuthorizationService>();

        var member = Principal(tokens.AccessToken);
        var admin = Principal(tokens.AccessToken);
        admin.Identities.Single().AddClaim(new Claim("role", nameof(Role.Admin)));

        Assert.False((await authorization.AuthorizeAsync(member, AppPolicies.AppAdmin)).Succeeded);
        Assert.True((await authorization.AuthorizeAsync(admin, AppPolicies.AppAdmin)).Succeeded);
    }

    private static Task<HttpResponseMessage> Guess(HttpClient client) =>
        client.PostAsJsonAsync("/account/login", new { email = "ghost@example.com", password = "guess-guess1" });

    // The claims exactly as the JwtBearer validator reads them: role and name claim types pinned to the token's own.
    private static ClaimsPrincipal Principal(string accessToken) =>
        new(new ClaimsIdentity(new JsonWebTokenHandler().ReadJsonWebToken(accessToken).Claims, "Bearer", "name", "role"));
}
