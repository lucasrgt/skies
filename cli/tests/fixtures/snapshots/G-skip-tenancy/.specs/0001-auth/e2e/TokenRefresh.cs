using System.Net;
using System.Net.Http.Json;
using Golden.Api;
using Golden.Api.Modules.Account;
using Golden.Tests;
using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.DependencyInjection;

namespace Specs.S0001;

/// <summary>Refresh rotation, theft detection, and logout. These are properties of the wired stack (the token,
/// the rotation, the family), so they are driven end to end.</summary>
public class TokenRefresh
{
    private const string Email = "user@example.com";

    [Fact(DisplayName = "FM-8: refresh rotates to a new token pair")]
    public async Task Refresh_rotates_to_a_new_token_pair()
    {
        await using var app = new TestApp();
        var client = app.CreateClient();
        var first = await AuthApi.SignUp(client, Email);

        var response = await AuthApi.RefreshWith(client, first.RefreshToken);

        response.EnsureSuccessStatusCode();
        var rotated = (await response.Content.ReadFromJsonAsync<AuthApi.Tokens>(AppJson.Options))!;
        Assert.NotEqual(first.RefreshToken, rotated.RefreshToken);
        Assert.Equal(Email, (await AuthApi.Profile(client, rotated.AccessToken)).Email);
    }

    // A spent token presented again has leaked (the legitimate client holds its replacement), so the whole family
    // burns: the thief's copy and the victim's live token alike.
    [Fact(DisplayName = "FM-9: a replayed refresh token is rejected and burns the live one")]
    public async Task Replayed_refresh_token_is_rejected_and_burns_the_family()
    {
        await using var app = new TestApp();
        var client = app.CreateClient();
        var first = await AuthApi.SignUp(client, Email);
        var rotated = (await (await AuthApi.RefreshWith(client, first.RefreshToken)).Content
            .ReadFromJsonAsync<AuthApi.Tokens>(AppJson.Options))!;

        var replay = await AuthApi.RefreshWith(client, first.RefreshToken);

        Assert.Equal(HttpStatusCode.Unauthorized, replay.StatusCode);
        Assert.Contains(AccountErrorCodes.SessionRevoked, await replay.Content.ReadAsStringAsync(), StringComparison.Ordinal);
        var live = await AuthApi.RefreshWith(client, rotated.RefreshToken);
        Assert.Equal(HttpStatusCode.Unauthorized, live.StatusCode);
    }

    [Fact(DisplayName = "FM-10: an unknown refresh token is rejected")]
    public async Task Unknown_refresh_token_is_rejected()
    {
        await using var app = new TestApp();
        var client = app.CreateClient();

        var response = await AuthApi.RefreshWith(client, "not-a-real-token");

        Assert.Equal(HttpStatusCode.Unauthorized, response.StatusCode);
    }

    // The family is aged past the ceiling without expiring the token itself (its first-seen time is backdated through
    // EF metadata, since CreatedAt is encapsulated), so it is the absolute-age rule that must retire it.
    [Fact(DisplayName = "FM-11: a family past the absolute maximum age no longer refreshes")]
    public async Task Family_past_the_maximum_age_no_longer_refreshes()
    {
        await using var app = new TestApp();
        var client = app.CreateClient();
        var tokens = await AuthApi.SignUp(client, Email);
        await using (var scope = app.Services.CreateAsyncScope())
        {
            var db = scope.ServiceProvider.GetRequiredService<AppDb>();
            var slot = await db.UserSessions.SingleAsync();
            db.Entry(slot).Property(s => s.CreatedAt).CurrentValue =
                DateTime.UtcNow - SessionToken.FamilyMaxAge - TimeSpan.FromDays(1);
            await db.SaveChangesAsync();
        }

        var response = await AuthApi.RefreshWith(client, tokens.RefreshToken);

        Assert.Equal(HttpStatusCode.Unauthorized, response.StatusCode);
    }

    [Fact(DisplayName = "FM-12: logout revokes the session so its refresh token is dead")]
    public async Task Logout_revokes_the_session()
    {
        await using var app = new TestApp();
        var client = app.CreateClient();
        var tokens = await AuthApi.SignUp(client, Email);

        (await client.PostAsJsonAsync("/account/logout", new { refreshToken = tokens.RefreshToken })).EnsureSuccessStatusCode();

        var response = await AuthApi.RefreshWith(client, tokens.RefreshToken);
        Assert.Equal(HttpStatusCode.Unauthorized, response.StatusCode);
    }

    [Fact(DisplayName = "FM-13: logout with an unknown token succeeds like any other")]
    public async Task Logout_with_an_unknown_token_succeeds()
    {
        await using var app = new TestApp();
        var client = app.CreateClient();

        var response = await client.PostAsJsonAsync("/account/logout", new { refreshToken = "not-a-real-token" });

        Assert.Equal(HttpStatusCode.OK, response.StatusCode);
    }
}
