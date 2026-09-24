using System.Net;
using Golden.Api;
using Golden.Api.Modules.Account;
using Golden.Tests;
using Microsoft.Extensions.DependencyInjection;
using Skies.Framework.Auth;

namespace Specs.S0001;

/// <summary>Session management: each login is a family the owner can see and end, and nobody else can touch.</summary>
public class SessionManagement
{
    private const string Email = "user@example.com";

    [Fact(DisplayName = "FM-16: the list shows each live family once, flags the current one, and hides expired slots")]
    public async Task Lists_live_families_and_flags_the_current_one()
    {
        await using var app = new TestApp();
        var client = app.CreateClient();
        var current = await AuthApi.SignUp(client, Email);
        var other = await AuthApi.Login(client, Email);
        (await AuthApi.RefreshWith(client, other.RefreshToken)).EnsureSuccessStatusCode();   // leaves a rotated slot
        var userId = (await AuthApi.Profile(client, current.AccessToken)).UserId;
        await using (var scope = app.Services.CreateAsyncScope())
        {
            var db = scope.ServiceProvider.GetRequiredService<AppDb>();
            var expiredAt = DateTime.UtcNow - RefreshSessionOptions.Default.Lifetime - TimeSpan.FromDays(1);
            db.UserSessions.Add(UserSession.Start(userId, Guid.NewGuid(), "expired-slot", expiredAt, expiredAt + RefreshSessionOptions.Default.Lifetime).Value);
            await db.SaveChangesAsync();
        }

        var sessions = await AuthApi.ListSessions(client, current.AccessToken);

        Assert.Equal(2, sessions.Count);
        Assert.Single(sessions, s => s.IsCurrent);
    }

    [Fact(DisplayName = "FM-17: revoking one of my sessions ends it and leaves the current one")]
    public async Task Revoking_a_session_ends_only_that_session()
    {
        await using var app = new TestApp();
        var client = app.CreateClient();
        var current = await AuthApi.SignUp(client, Email);
        var target = await AuthApi.Login(client, Email);
        var targetId = Assert.Single(await AuthApi.ListSessions(client, current.AccessToken), s => !s.IsCurrent).SessionId;

        var response = await client.SendAsync(
            AuthApi.As(current.AccessToken, HttpMethod.Post, "/account/sessions/revoke", new { sessionId = targetId }));

        response.EnsureSuccessStatusCode();
        Assert.Equal(HttpStatusCode.Unauthorized, (await AuthApi.RefreshWith(client, target.RefreshToken)).StatusCode);
        Assert.Equal(HttpStatusCode.OK, (await AuthApi.RefreshWith(client, current.RefreshToken)).StatusCode);
    }

    [Fact(DisplayName = "FM-18: another user's session is not found and stays alive")]
    public async Task Another_users_session_cannot_be_revoked()
    {
        await using var app = new TestApp();
        var client = app.CreateClient();
        var attacker = await AuthApi.SignUp(client, "attacker@example.com");
        var victim = await AuthApi.SignUp(client, "victim@example.com");
        var victimSession = Assert.Single(await AuthApi.ListSessions(client, victim.AccessToken)).SessionId;

        var response = await client.SendAsync(
            AuthApi.As(attacker.AccessToken, HttpMethod.Post, "/account/sessions/revoke", new { sessionId = victimSession }));

        Assert.Equal(HttpStatusCode.NotFound, response.StatusCode);
        Assert.Contains(AccountErrorCodes.SessionNotFound, await response.Content.ReadAsStringAsync(), StringComparison.Ordinal);
        Assert.Equal(HttpStatusCode.OK, (await AuthApi.RefreshWith(client, victim.RefreshToken)).StatusCode);
    }

    [Fact(DisplayName = "FM-19: signing out everywhere else keeps the current session and nobody else's")]
    public async Task Revoking_other_sessions_keeps_the_current_one()
    {
        await using var app = new TestApp();
        var client = app.CreateClient();
        var current = await AuthApi.SignUp(client, Email);
        var second = await AuthApi.Login(client, Email);
        var third = await AuthApi.Login(client, Email);
        var bystander = await AuthApi.SignUp(client, "bystander@example.com");

        var response = await client.SendAsync(AuthApi.As(current.AccessToken, HttpMethod.Post, "/account/sessions/revoke-others"));

        response.EnsureSuccessStatusCode();
        Assert.Equal(HttpStatusCode.Unauthorized, (await AuthApi.RefreshWith(client, second.RefreshToken)).StatusCode);
        Assert.Equal(HttpStatusCode.Unauthorized, (await AuthApi.RefreshWith(client, third.RefreshToken)).StatusCode);
        Assert.Equal(HttpStatusCode.OK, (await AuthApi.RefreshWith(client, current.RefreshToken)).StatusCode);
        Assert.Equal(HttpStatusCode.OK, (await AuthApi.RefreshWith(client, bystander.RefreshToken)).StatusCode);
    }
}
