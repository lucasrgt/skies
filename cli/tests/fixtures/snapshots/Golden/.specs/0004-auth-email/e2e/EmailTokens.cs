using System.Net;
using System.Net.Http.Headers;
using System.Net.Http.Json;
using Golden.Api;
using Golden.Tests;
using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.Mvc.Testing;
using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.DependencyInjection;
using Skies.Framework.Mail;

namespace Specs.S0004;

/// <summary>Password reset and email verification end to end, reading the emailed token through a capturing
/// <see cref="IEmailSender"/> layered over the booted app (the last registration wins).</summary>
public class EmailTokens
{
    private const string Email = "reset@example.com";
    private const string Password = "password1";

    private sealed record Tokens(string AccessToken, string RefreshToken);

    /// <summary>The Request* slices end the body with the raw token ("...with this token: ABC").</summary>
    private sealed class CapturingEmailSender : IEmailSender
    {
        public string? LastToken { get; private set; }

        public int Sent { get; private set; }

        public Task SendAsync(EmailMessage message, CancellationToken ct = default)
        {
            LastToken = message.Body[(message.Body.LastIndexOf(' ') + 1)..];
            Sent++;
            return Task.CompletedTask;
        }
    }

    [Fact(DisplayName = "FM-1: the emailed reset token sets a new password that signs in")]
    public async Task A_reset_token_sets_a_new_password()
    {
        var mail = new CapturingEmailSender();
        await using var app = new TestApp();
        using var factory = WithMail(app, mail);
        var client = factory.CreateClient();
        await Register(client);
        (await client.PostAsJsonAsync("/account/password-reset/request", new { email = Email })).EnsureSuccessStatusCode();

        var reset = await client.PostAsJsonAsync("/account/password-reset", new { token = mail.LastToken, newPassword = "new-password1" });

        reset.EnsureSuccessStatusCode();
        await Login(client, "new-password1");
    }

    [Fact(DisplayName = "FM-2: a wrong reset token is rejected and the password is unchanged")]
    public async Task A_wrong_reset_token_leaves_the_password_unchanged()
    {
        var mail = new CapturingEmailSender();
        await using var app = new TestApp();
        using var factory = WithMail(app, mail);
        var client = factory.CreateClient();
        await Register(client);
        (await client.PostAsJsonAsync("/account/password-reset/request", new { email = Email })).EnsureSuccessStatusCode();

        var reset = await client.PostAsJsonAsync("/account/password-reset", new { token = "not-a-real-token", newPassword = "new-password1" });

        Assert.Equal(HttpStatusCode.Unauthorized, reset.StatusCode);
        await Login(client, Password);
        var attempted = await client.PostAsJsonAsync("/account/login", new { email = Email, password = "new-password1" });
        Assert.Equal(HttpStatusCode.Unauthorized, attempted.StatusCode);
    }

    [Fact(DisplayName = "FM-3: a reset to a too-short password is rejected and the password is unchanged")]
    public async Task A_short_password_is_rejected()
    {
        var mail = new CapturingEmailSender();
        await using var app = new TestApp();
        using var factory = WithMail(app, mail);
        var client = factory.CreateClient();
        await Register(client);
        (await client.PostAsJsonAsync("/account/password-reset/request", new { email = Email })).EnsureSuccessStatusCode();

        var reset = await client.PostAsJsonAsync("/account/password-reset", new { token = mail.LastToken, newPassword = "short" });

        Assert.Equal(HttpStatusCode.BadRequest, reset.StatusCode);
        await Login(client, Password);
    }

    // After a takeover the attacker holds a live refresh token; the reset must end it along with every other session.
    [Fact(DisplayName = "FM-4: a password reset ends the sessions opened before it")]
    public async Task A_reset_ends_existing_sessions()
    {
        var mail = new CapturingEmailSender();
        await using var app = new TestApp();
        using var factory = WithMail(app, mail);
        var client = factory.CreateClient();
        await Register(client);
        var before = await Login(client, Password);
        (await client.PostAsJsonAsync("/account/password-reset/request", new { email = Email })).EnsureSuccessStatusCode();

        (await client.PostAsJsonAsync("/account/password-reset", new { token = mail.LastToken, newPassword = "new-password1" }))
            .EnsureSuccessStatusCode();

        var refresh = await client.PostAsJsonAsync("/account/refresh", new { refreshToken = before.RefreshToken });
        Assert.Equal(HttpStatusCode.Unauthorized, refresh.StatusCode);
    }

    [Fact(DisplayName = "FM-5: a reset for an unknown email answers like a known one and mails nothing")]
    public async Task A_reset_for_an_unknown_email_reveals_nothing()
    {
        var mail = new CapturingEmailSender();
        await using var app = new TestApp();
        using var factory = WithMail(app, mail);
        var client = factory.CreateClient();
        await Register(client);

        var known = await client.PostAsJsonAsync("/account/password-reset/request", new { email = Email });
        var unknown = await client.PostAsJsonAsync("/account/password-reset/request", new { email = "nobody@example.com" });

        Assert.Equal(HttpStatusCode.OK, known.StatusCode);
        Assert.Equal(known.StatusCode, unknown.StatusCode);
        Assert.Equal(await known.Content.ReadAsStringAsync(), await unknown.Content.ReadAsStringAsync());
        Assert.Equal(1, mail.Sent);
    }

    [Fact(DisplayName = "FM-6: the emailed verification token marks the email verified")]
    public async Task An_email_token_confirms_the_email()
    {
        var mail = new CapturingEmailSender();
        await using var app = new TestApp();
        using var factory = WithMail(app, mail);
        var client = factory.CreateClient();
        await Authenticate(client);
        (await client.PostAsJsonAsync("/account/verify-email/request", new { })).EnsureSuccessStatusCode();

        var verify = await client.PostAsJsonAsync("/account/verify-email", new { token = mail.LastToken });

        verify.EnsureSuccessStatusCode();
        Assert.True(await IsEmailVerified(factory));
    }

    [Fact(DisplayName = "FM-7: a wrong verification token confirms nothing and leaves the real one usable")]
    public async Task A_wrong_email_token_is_rejected_and_the_real_one_still_confirms()
    {
        var mail = new CapturingEmailSender();
        await using var app = new TestApp();
        using var factory = WithMail(app, mail);
        var client = factory.CreateClient();
        await Authenticate(client);
        (await client.PostAsJsonAsync("/account/verify-email/request", new { })).EnsureSuccessStatusCode();

        var wrong = await client.PostAsJsonAsync("/account/verify-email", new { token = "not-a-real-token" });

        Assert.Equal(HttpStatusCode.Unauthorized, wrong.StatusCode);
        Assert.False(await IsEmailVerified(factory));
        (await client.PostAsJsonAsync("/account/verify-email", new { token = mail.LastToken })).EnsureSuccessStatusCode();
    }

    private static WebApplicationFactory<Program> WithMail(TestApp app, IEmailSender mail) =>
        app.WithWebHostBuilder(builder => builder.ConfigureServices(services => services.AddSingleton(mail)));

    private static async Task Register(HttpClient client) =>
        (await client.PostAsJsonAsync("/account/register", new { email = Email, password = Password })).EnsureSuccessStatusCode();

    private static async Task<Tokens> Login(HttpClient client, string password)
    {
        var login = await client.PostAsJsonAsync("/account/login", new { email = Email, password });
        login.EnsureSuccessStatusCode();
        return (await login.Content.ReadFromJsonAsync<Tokens>(AppJson.Options))!;
    }

    private static async Task Authenticate(HttpClient client)
    {
        await Register(client);
        var tokens = await Login(client, Password);
        client.DefaultRequestHeaders.Authorization = new AuthenticationHeaderValue("Bearer", tokens.AccessToken);
    }

    private static async Task<bool> IsEmailVerified(WebApplicationFactory<Program> factory)
    {
        await using var scope = factory.Services.CreateAsyncScope();
        var user = await scope.ServiceProvider.GetRequiredService<AppDb>().Users.IgnoreQueryFilters().SingleAsync();
        return user.IsEmailVerified;
    }
}
