using Skies.Framework.Abstractions;

namespace Skies.Framework.Identity;

/// <summary>
/// A dev/test identity verifier that treats the token as the email, so OAuth flows run without a real provider,
/// reporting the provider as "fake". An empty token is rejected the way a real verifier rejects a bad signature. It
/// serves both ports: <see cref="IExternalIdentityVerifier"/>, which generated slices call, and the older synchronous
/// <see cref="IExternalIdentity"/>. Production registers <see cref="OidcIdTokenVerifier"/> in its place.
/// </summary>
public sealed class FakeExternalIdentity : IExternalIdentity, IExternalIdentityVerifier
{
    /// <inheritdoc />
    public Result<ExternalUser> Verify(string idToken) =>
        string.IsNullOrWhiteSpace(idToken)
            ? Error.Unauthorized("identity.invalid_token", "invalid identity token")
            : new ExternalUser("fake", idToken, idToken);

    /// <inheritdoc />
    public Task<Result<ExternalUser>> VerifyAsync(string idToken, CancellationToken ct = default) =>
        Task.FromResult(Verify(idToken));
}
