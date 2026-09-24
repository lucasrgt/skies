using Skies.Framework.Abstractions;

namespace Skies.Framework.Identity;

/// <summary>
/// Verifies an external identity provider's OIDC id_token — Google, Apple, Microsoft, and the like all
/// issue one — and returns the verified user. A <b>vendor-neutral</b> port: the framework never names a
/// provider. This synchronous port predates <see cref="IExternalIdentityVerifier"/>, which a real verifier needs
/// (it fetches the provider's keys) and which generated slices call; it stays for the implementations already
/// written against it.
/// </summary>
public interface IExternalIdentity
{
    /// <summary>Verify <paramref name="idToken"/> and return the verified external user, or an error.</summary>
    Result<ExternalUser> Verify(string idToken);
}

/// <summary>A verified external identity: which provider vouched for it, the stable subject id, and the
/// verified email address.</summary>
/// <param name="Provider">The provider that verified the identity (e.g. "google").</param>
/// <param name="Subject">The provider's stable, unique id for the user.</param>
/// <param name="Email">The verified email address.</param>
public sealed record ExternalUser(string Provider, string Subject, string Email);
