using Skies.Framework.Abstractions;

namespace Skies.Framework.Identity;

/// <summary>
/// Verifies an external provider's OIDC id_token and returns the verified user: the port an app's "sign in with …"
/// slices call. Asynchronous because a real verifier fetches (and caches) the provider's signing keys.
/// <see cref="OidcIdTokenVerifier"/> is the standards-based implementation for any OIDC provider (Google, Apple,
/// Microsoft, an in-house issuer), configured with the provider's authority and the app's client id; no provider is
/// named in code. <see cref="FakeExternalIdentity"/> stands in for development and tests.
/// </summary>
public interface IExternalIdentityVerifier
{
    /// <summary>Verify <paramref name="idToken"/> and return the verified external user, or an unauthorized error.
    /// A token that fails any check (signature, issuer, audience, lifetime, verified email) yields the error, never a
    /// user.</summary>
    Task<Result<ExternalUser>> VerifyAsync(string idToken, CancellationToken ct = default);
}
