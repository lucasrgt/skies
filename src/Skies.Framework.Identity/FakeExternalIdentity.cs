using Skies.Framework.Abstractions;

namespace Skies.Framework.Identity;

/// <summary>A development/test verifier that treats a non-empty token as an email.
/// Register only in development or tests; deployments use <see cref="OidcIdTokenVerifier"/>.</summary>
public sealed class FakeExternalIdentity : IExternalIdentityVerifier
{
    /// <inheritdoc />
    public Task<Result<ExternalUser>> VerifyAsync(string idToken, CancellationToken ct = default)
    {
        ct.ThrowIfCancellationRequested();
        Result<ExternalUser> result = string.IsNullOrWhiteSpace(idToken)
            ? Error.Unauthorized("identity.invalid_token", "invalid identity token")
            : new ExternalUser("fake", idToken, idToken);
        return Task.FromResult(result);
    }
}
