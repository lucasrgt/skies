using System.Net;
using System.Net.Http.Json;
using Golden.Api.Modules.Account;
using Golden.Tests;

namespace Specs.S0001;

/// <summary>Registration and password sign-in: the identity invariant (one email, one account) and the bypass
/// guard (a bad credential never yields a token).</summary>
public class Credentials
{
    private const string Email = "user@example.com";

    [Fact(DisplayName = "FM-1: a registration signs in by its normalized email and reads its own profile")]
    public async Task Registration_signs_in_by_its_normalized_email()
    {
        await using var app = new TestApp();
        var client = app.CreateClient();

        await AuthApi.Register(client, "Alice@Example.com ", "s3cret-pw");
        var tokens = await AuthApi.Login(client, "alice@example.com", "s3cret-pw");

        var profile = await AuthApi.Profile(client, tokens.AccessToken);
        Assert.Equal("alice@example.com", profile.Email);
    }

    [Fact(DisplayName = "FM-2: a malformed email is rejected as a validation error")]
    public async Task Malformed_email_is_rejected()
    {
        await using var app = new TestApp();
        var client = app.CreateClient();

        var response = await client.PostAsJsonAsync("/account/register", new { email = "not-an-email", password = AuthApi.Password });

        Assert.Equal(HttpStatusCode.BadRequest, response.StatusCode);
    }

    [Fact(DisplayName = "FM-3: a duplicate email is rejected and the first account still signs in")]
    public async Task Duplicate_email_is_rejected_and_the_first_account_survives()
    {
        await using var app = new TestApp();
        var client = app.CreateClient();
        await AuthApi.Register(client, Email);

        var duplicate = await client.PostAsJsonAsync("/account/register", new { email = Email, password = "another-pw1" });

        Assert.Equal(HttpStatusCode.Conflict, duplicate.StatusCode);
        Assert.Contains(AccountErrorCodes.EmailTaken, await duplicate.Content.ReadAsStringAsync(), StringComparison.Ordinal);
        await AuthApi.Login(client, Email);
    }

    [Fact(DisplayName = "FM-4: valid credentials yield a token pair whose access token reads the caller's profile")]
    public async Task Valid_credentials_yield_a_usable_token_pair()
    {
        await using var app = new TestApp();
        var client = app.CreateClient();
        await AuthApi.Register(client, Email);

        var tokens = await AuthApi.Login(client, Email);

        Assert.False(string.IsNullOrWhiteSpace(tokens.RefreshToken));
        var profile = await AuthApi.Profile(client, tokens.AccessToken);
        Assert.Equal(Email, profile.Email);
        Assert.Equal(RegistrationStep.EmailPending, profile.Step);
    }

    [Fact(DisplayName = "FM-5: a wrong password is denied and issues no token")]
    public async Task Wrong_password_is_denied_and_issues_no_token()
    {
        await using var app = new TestApp();
        var client = app.CreateClient();
        await AuthApi.Register(client, Email);

        var response = await client.PostAsJsonAsync("/account/login", new { email = Email, password = "wrong-password" });

        Assert.Equal(HttpStatusCode.Unauthorized, response.StatusCode);
        Assert.DoesNotContain("accessToken", await response.Content.ReadAsStringAsync(), StringComparison.OrdinalIgnoreCase);
    }

    // A missing account and a wrong password must be the same response, so login cannot be used to find out who has
    // an account. Timing is equalized in Handle by verifying against a dummy hash; a wall-clock assertion would be
    // flaky, so the structural pin is response equality.
    [Fact(DisplayName = "FM-6: an unknown email and a wrong password get the same response")]
    public async Task Unknown_email_and_wrong_password_are_indistinguishable()
    {
        await using var app = new TestApp();
        var client = app.CreateClient();
        await AuthApi.Register(client, Email);

        var wrongPassword = await client.PostAsJsonAsync("/account/login", new { email = Email, password = "nope-nope1" });
        var unknownEmail = await client.PostAsJsonAsync("/account/login", new { email = "ghost@example.com", password = "nope-nope1" });

        Assert.Equal(HttpStatusCode.Unauthorized, wrongPassword.StatusCode);
        Assert.Equal(wrongPassword.StatusCode, unknownEmail.StatusCode);
        Assert.Equal(await wrongPassword.Content.ReadAsStringAsync(), await unknownEmail.Content.ReadAsStringAsync());
    }

    [Fact(DisplayName = "FM-7: the profile endpoint requires an access token")]
    public async Task Profile_requires_a_token()
    {
        await using var app = new TestApp();
        var client = app.CreateClient();

        var response = await client.GetAsync("/account/me");

        Assert.Equal(HttpStatusCode.Unauthorized, response.StatusCode);
    }
}
