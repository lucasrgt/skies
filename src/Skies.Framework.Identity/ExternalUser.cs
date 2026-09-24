namespace Skies.Framework.Identity;

/// <summary>A verified external identity: which provider vouched for it, the stable subject id, and the
/// verified email address.</summary>
/// <param name="Provider">The provider that verified the identity (e.g. "google").</param>
/// <param name="Subject">The provider's stable, unique id for the user.</param>
/// <param name="Email">The verified email address.</param>
public sealed record ExternalUser(string Provider, string Subject, string Email);
