using System.Net;
using System.Net.Http.Headers;
using System.Net.Http.Json;
using Golden.Api;
using Golden.Api.Modules.Account;
using Golden.Tests;
using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.DependencyInjection;

namespace Specs.S0003;

/// <summary>Google sign-up and sign-in over HTTP. An empty id token is the forged identity: the dev verifier rejects
/// it the way a real one rejects a bad signature.</summary>
public class GoogleSignIn
{
    private const string Identity = "g-user@example.com";

    private sealed record Session(string AccessToken, string RefreshToken, RegistrationStep Step);

    [Fact(DisplayName = "FM-1: a verified identity creates an email-verified account with a live session")]
    public async Task A_verified_identity_creates_an_account()
    {
        await using var app = new TestApp();
        var client = app.CreateClient();

        var response = await client.PostAsJsonAsync("/account/register/google", new { idToken = Identity });

        response.EnsureSuccessStatusCode();
        var session = (await response.Content.ReadFromJsonAsync<Session>(AppJson.Options))!;
        Assert.Equal(RegistrationStep.PhonePending, session.Step);
        Assert.Equal(Identity, (await Profile(client, session.AccessToken)).Email);
        await using var scope = app.Services.CreateAsyncScope();
        var user = await scope.ServiceProvider.GetRequiredService<AppDb>().Users.IgnoreQueryFilters().SingleAsync();
        Assert.True(user.IsEmailVerified);
    }

    [Fact(DisplayName = "FM-2: an unverifiable token creates no account and no session")]
    public async Task An_unverifiable_token_creates_no_account()
    {
        await using var app = new TestApp();
        var client = app.CreateClient();

        var response = await client.PostAsJsonAsync("/account/register/google", new { idToken = "" });

        Assert.Equal(HttpStatusCode.Unauthorized, response.StatusCode);
        Assert.DoesNotContain("accessToken", await response.Content.ReadAsStringAsync(), StringComparison.OrdinalIgnoreCase);
        await using var scope = app.Services.CreateAsyncScope();
        Assert.False(await scope.ServiceProvider.GetRequiredService<AppDb>().Users.IgnoreQueryFilters().AnyAsync());
    }

    [Fact(DisplayName = "FM-3: an identity whose email already has an account is a conflict")]
    public async Task A_taken_email_is_a_conflict()
    {
        await using var app = new TestApp();
        var client = app.CreateClient();
        (await client.PostAsJsonAsync("/account/register/google", new { idToken = Identity })).EnsureSuccessStatusCode();

        var response = await client.PostAsJsonAsync("/account/register/google", new { idToken = Identity });

        Assert.Equal(HttpStatusCode.Conflict, response.StatusCode);
        await using var scope = app.Services.CreateAsyncScope();
        Assert.Equal(1, await scope.ServiceProvider.GetRequiredService<AppDb>().Users.IgnoreQueryFilters().CountAsync());
    }

    [Fact(DisplayName = "FM-4: a known identity signs in to its own account")]
    public async Task A_known_identity_signs_in()
    {
        await using var app = new TestApp();
        var client = app.CreateClient();
        (await client.PostAsJsonAsync("/account/register/google", new { idToken = Identity })).EnsureSuccessStatusCode();

        var response = await client.PostAsJsonAsync("/account/login/google", new { idToken = Identity });

        response.EnsureSuccessStatusCode();
        var session = (await response.Content.ReadFromJsonAsync<Session>(AppJson.Options))!;
        Assert.Equal(Identity, (await Profile(client, session.AccessToken)).Email);
    }

    [Fact(DisplayName = "FM-5: an unverifiable token is denied")]
    public async Task An_unverifiable_token_is_denied()
    {
        await using var app = new TestApp();
        var client = app.CreateClient();
        (await client.PostAsJsonAsync("/account/register/google", new { idToken = Identity })).EnsureSuccessStatusCode();

        var response = await client.PostAsJsonAsync("/account/login/google", new { idToken = "" });

        Assert.Equal(HttpStatusCode.Unauthorized, response.StatusCode);
    }

    [Fact(DisplayName = "FM-6: a verified identity without an account is denied")]
    public async Task An_identity_without_an_account_is_denied()
    {
        await using var app = new TestApp();
        var client = app.CreateClient();

        var response = await client.PostAsJsonAsync("/account/login/google", new { idToken = "nobody@example.com" });

        Assert.Equal(HttpStatusCode.Unauthorized, response.StatusCode);
        Assert.Contains(AccountErrorCodes.NoAccount, await response.Content.ReadAsStringAsync(), StringComparison.Ordinal);
    }

    private static async Task<Me.Output> Profile(HttpClient client, string accessToken)
    {
        var request = new HttpRequestMessage(HttpMethod.Get, "/account/me");
        request.Headers.Authorization = new AuthenticationHeaderValue("Bearer", accessToken);
        var response = await client.SendAsync(request);
        response.EnsureSuccessStatusCode();
        return (await response.Content.ReadFromJsonAsync<Me.Output>(AppJson.Options))!;
    }
}
