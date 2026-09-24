using System.Security.Cryptography;
using System.Text;
using Microsoft.IdentityModel.JsonWebTokens;
using Microsoft.IdentityModel.Protocols;
using Microsoft.IdentityModel.Protocols.OpenIdConnect;
using Microsoft.IdentityModel.Tokens;
using Skies.Framework.Abstractions;
using Skies.Framework.Identity;

namespace Skies.Framework.Ports.Tests;

// The OIDC verifier is the only thing standing between a forged "sign in with Google" and an account takeover. Each
// case mints a real signed id_token and runs it through the verifier against a static provider configuration, so no
// network is involved and every refusal is the verifier's own.
public sealed class OidcIdTokenVerifierTests : IDisposable
{
    private const string Issuer = "https://issuer.example.test";
    private const string ClientId = "my-client-id";

    private readonly RSA providerRsa = RSA.Create(2048);
    private readonly RsaSecurityKey providerKey;

    public OidcIdTokenVerifierTests() =>
        providerKey = new RsaSecurityKey(providerRsa) { KeyId = "provider-key" };

    public void Dispose() => providerRsa.Dispose();

    private OidcIdTokenVerifier Verifier(bool requireVerifiedEmail = true)
    {
        var configuration = new OpenIdConnectConfiguration { Issuer = Issuer };
        configuration.SigningKeys.Add(providerKey);
        return new OidcIdTokenVerifier(
            new OidcProviderOptions("example", Issuer, ClientId) { RequireVerifiedEmail = requireVerifiedEmail },
            new StaticConfigurationManager<OpenIdConnectConfiguration>(configuration));
    }

    private string Mint(
        string issuer = Issuer,
        string audience = ClientId,
        DateTime? expires = null,
        SecurityKey? key = null,
        string algorithm = SecurityAlgorithms.RsaSha256,
        object? emailVerified = null,
        string? email = "person@example.test")
    {
        var claims = new Dictionary<string, object> { ["sub"] = "subject-123", ["email_verified"] = emailVerified ?? true };
        if (email is not null)
            claims["email"] = email;
        var now = DateTime.UtcNow;
        return new JsonWebTokenHandler().CreateToken(new SecurityTokenDescriptor
        {
            Issuer = issuer,
            Audience = audience,
            IssuedAt = (expires ?? now.AddMinutes(5)).AddMinutes(-10),
            NotBefore = (expires ?? now.AddMinutes(5)).AddMinutes(-10),
            Expires = expires ?? now.AddMinutes(5),
            Claims = claims,
            SigningCredentials = new SigningCredentials(key ?? providerKey, algorithm),
        });
    }

    [Fact]
    public async Task A_valid_token_yields_the_provider_subject_and_email()
    {
        var result = await Verifier().VerifyAsync(Mint());

        Assert.True(result.IsSuccess);
        Assert.Equal(new ExternalUser("example", "subject-123", "person@example.test"), result.Value);
    }

    [Fact]
    public async Task A_token_for_another_client_is_refused()
    {
        AssertRefused(await Verifier().VerifyAsync(Mint(audience: "someone-elses-client")));
    }

    [Fact]
    public async Task A_token_from_another_issuer_is_refused()
    {
        AssertRefused(await Verifier().VerifyAsync(Mint(issuer: "https://evil.example.test")));
    }

    [Fact]
    public async Task An_expired_token_is_refused()
    {
        AssertRefused(await Verifier().VerifyAsync(Mint(expires: DateTime.UtcNow.AddMinutes(-5))));
    }

    [Fact]
    public async Task A_token_signed_by_another_key_is_refused()
    {
        using var attacker = RSA.Create(2048);
        AssertRefused(await Verifier().VerifyAsync(Mint(key: new RsaSecurityKey(attacker) { KeyId = "provider-key" })));
    }

    // Algorithm substitution: a shared-secret signature must never be accepted, whatever the key bytes.
    [Fact]
    public async Task A_symmetrically_signed_token_is_refused()
    {
        var secret = new SymmetricSecurityKey(Encoding.UTF8.GetBytes(new string('k', 64))) { KeyId = "provider-key" };
        AssertRefused(await Verifier().VerifyAsync(Mint(key: secret, algorithm: SecurityAlgorithms.HmacSha256)));
    }

    [Fact]
    public async Task A_tampered_token_is_refused()
    {
        var token = Mint();
        var tampered = token[..^4] + (token[^4] == 'A' ? "B" : "A") + token[^3..];

        AssertRefused(await Verifier().VerifyAsync(tampered));
    }

    [Theory]
    [InlineData(false)]
    [InlineData("false")]
    public async Task An_unverified_email_is_refused_by_default(object verified)
    {
        AssertRefused(await Verifier().VerifyAsync(Mint(emailVerified: verified)));
        Assert.True((await Verifier(requireVerifiedEmail: false).VerifyAsync(Mint(emailVerified: verified))).IsSuccess);
    }

    [Fact]
    public async Task A_string_true_email_verified_claim_is_accepted()
    {
        Assert.True((await Verifier().VerifyAsync(Mint(emailVerified: "true"))).IsSuccess);
    }

    [Fact]
    public async Task A_token_without_an_email_is_refused()
    {
        AssertRefused(await Verifier().VerifyAsync(Mint(email: null)));
    }

    [Theory]
    [InlineData("")]
    [InlineData("   ")]
    [InlineData("not.a.jwt")]
    public async Task Garbage_is_refused(string token)
    {
        AssertRefused(await Verifier().VerifyAsync(token));
    }

    [Fact]
    public async Task The_fake_serves_the_async_port_the_same_way()
    {
        IExternalIdentityVerifier fake = new FakeExternalIdentity();

        Assert.Equal(new ExternalUser("fake", "dev@example.test", "dev@example.test"), (await fake.VerifyAsync("dev@example.test")).Value);
        AssertRefused(await fake.VerifyAsync(""));
    }

    private static void AssertRefused(Result<ExternalUser> result)
    {
        Assert.True(result.IsFailure);
        Assert.Equal(ErrorKind.Unauthorized, result.Error.Kind);
        Assert.Equal(OidcIdTokenVerifier.InvalidTokenCode, result.Error.Code);
    }
}
