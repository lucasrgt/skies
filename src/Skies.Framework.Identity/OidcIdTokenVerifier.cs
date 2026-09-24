using Microsoft.IdentityModel.JsonWebTokens;
using Microsoft.IdentityModel.Protocols;
using Microsoft.IdentityModel.Protocols.OpenIdConnect;
using Microsoft.IdentityModel.Tokens;
using Skies.Framework.Abstractions;

namespace Skies.Framework.Identity;

/// <summary>Which OIDC provider an <see cref="OidcIdTokenVerifier"/> trusts, and for which client.</summary>
/// <param name="Provider">The name reported on <see cref="ExternalUser.Provider"/> (e.g. "google").</param>
/// <param name="Authority">The issuer's base URL; its <c>/.well-known/openid-configuration</c> names the signing keys
/// and the issuer (e.g. <c>https://accounts.google.com</c>).</param>
/// <param name="ClientId">The app's client id at the provider: the audience every accepted token must carry.</param>
public sealed record OidcProviderOptions(string Provider, string Authority, string ClientId)
{
    /// <summary>Further issuer spellings the provider uses beside its discovery document's (Google also issues as
    /// <c>accounts.google.com</c>).</summary>
    public IReadOnlyList<string> AdditionalIssuers { get; init; } = [];

    /// <summary>Whether a token must assert <c>email_verified</c>. On by default: an app that signs users in by email
    /// must not trust an address the provider never proved.</summary>
    public bool RequireVerifiedEmail { get; init; } = true;
}

/// <summary>
/// Verifies OIDC id_tokens against a provider's published keys: signature (asymmetric algorithms only, keys from the
/// discovery document, refreshed once on an unknown key id for key rotation), issuer, audience (the app's client id),
/// lifetime, and a verified email. The standard is the dependency, not a vendor: any OIDC provider is a configuration
/// value. Register one per provider the app trusts.
/// </summary>
/// <example>
/// <code>
/// builder.Services.AddSingleton&lt;IExternalIdentityVerifier&gt;(new OidcIdTokenVerifier(
///     new OidcProviderOptions("google", "https://accounts.google.com", googleClientId)
///     {
///         AdditionalIssuers = ["accounts.google.com"],
///     }));
/// </code>
/// </example>
public sealed class OidcIdTokenVerifier : IExternalIdentityVerifier
{
    /// <summary>The error code of every refusal, the same one <see cref="FakeExternalIdentity"/> reports.</summary>
    public const string InvalidTokenCode = "identity.invalid_token";

    // Asymmetric only: a provider never signs with a shared secret, so accepting HS* would let anyone holding the
    // public key forge a token (algorithm substitution), and "none" is never a signature.
    private static readonly string[] Algorithms =
    [
        SecurityAlgorithms.RsaSha256, SecurityAlgorithms.RsaSha384, SecurityAlgorithms.RsaSha512,
        SecurityAlgorithms.RsaSsaPssSha256, SecurityAlgorithms.RsaSsaPssSha384, SecurityAlgorithms.RsaSsaPssSha512,
        SecurityAlgorithms.EcdsaSha256, SecurityAlgorithms.EcdsaSha384, SecurityAlgorithms.EcdsaSha512,
    ];

    private readonly OidcProviderOptions options;
    private readonly IConfigurationManager<OpenIdConnectConfiguration> configuration;
    private readonly JsonWebTokenHandler handler = new();

    /// <summary>Trust the provider at <see cref="OidcProviderOptions.Authority"/>, reading its discovery document over
    /// HTTPS (cached and refreshed by IdentityModel's configuration manager).</summary>
    public OidcIdTokenVerifier(OidcProviderOptions options)
        : this(options, new ConfigurationManager<OpenIdConnectConfiguration>(
            $"{options.Authority.TrimEnd('/')}/.well-known/openid-configuration",
            new OpenIdConnectConfigurationRetriever(),
            new HttpDocumentRetriever { RequireHttps = true }))
    {
    }

    /// <summary>Trust the provider whose configuration <paramref name="configuration"/> supplies: the seam for a
    /// pre-fetched or static configuration (an offline test, an air-gapped issuer).</summary>
    public OidcIdTokenVerifier(OidcProviderOptions options, IConfigurationManager<OpenIdConnectConfiguration> configuration)
    {
        ArgumentNullException.ThrowIfNull(options);
        ArgumentNullException.ThrowIfNull(configuration);
        this.options = options;
        this.configuration = configuration;
    }

    /// <inheritdoc />
    public async Task<Result<ExternalUser>> VerifyAsync(string idToken, CancellationToken ct = default)
    {
        if (string.IsNullOrWhiteSpace(idToken))
            return Invalid("identity token is missing");

        var result = await ValidateAsync(idToken, ct);
        if (!result.IsValid && result.Exception is SecurityTokenSignatureKeyNotFoundException)
        {
            // An unknown key id usually means the provider rotated its keys since the last fetch: refresh once.
            configuration.RequestRefresh();
            result = await ValidateAsync(idToken, ct);
        }
        if (!result.IsValid || result.SecurityToken is not JsonWebToken token)
            return Invalid("identity token failed verification");

        if (!token.TryGetPayloadValue<string>("email", out var email) || string.IsNullOrWhiteSpace(email))
            return Invalid("identity token carries no email");
        if (options.RequireVerifiedEmail && !EmailVerified(token))
            return Invalid("identity token email is not verified");

        return new ExternalUser(options.Provider, token.Subject, email);
    }

    private async Task<TokenValidationResult> ValidateAsync(string idToken, CancellationToken ct)
    {
        var provider = await configuration.GetConfigurationAsync(ct);
        return await handler.ValidateTokenAsync(idToken, new TokenValidationParameters
        {
            ValidIssuers = [provider.Issuer, .. options.AdditionalIssuers],
            ValidAudience = options.ClientId,
            IssuerSigningKeys = provider.SigningKeys,
            ValidAlgorithms = Algorithms,
            ValidateIssuer = true,
            ValidateAudience = true,
            ValidateLifetime = true,
            ValidateIssuerSigningKey = true,
            RequireSignedTokens = true,
            RequireExpirationTime = true,
            ClockSkew = TimeSpan.FromMinutes(1),
        });
    }

    // Providers disagree on the claim's JSON type: Google sends a boolean, some issuers the string "true".
    private static bool EmailVerified(JsonWebToken token) =>
        (token.TryGetPayloadValue<bool>("email_verified", out var flag) && flag)
        || (token.TryGetPayloadValue<string>("email_verified", out var text)
            && string.Equals(text, "true", StringComparison.OrdinalIgnoreCase));

    private static Error Invalid(string message) => Error.Unauthorized(InvalidTokenCode, message);
}
