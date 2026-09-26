using System.Buffers.Text;
using System.Security.Cryptography;
using System.Text;

namespace Skies.Framework.Auth;

/// <summary>A freshly minted opaque token: the <paramref name="Raw"/> value handed to the client exactly once and the
/// <paramref name="Hash"/> the app stores in its place.</summary>
/// <param name="Raw">The secret itself: 256 random bits, Base64Url, so it rides a cookie or URL without escaping.</param>
/// <param name="Hash">Its SHA256 lookup hash (uppercase hex). Store this, never <paramref name="Raw"/>.</param>
public readonly record struct OpaqueToken(string Raw, string Hash);

/// <summary>High-entropy bearer secrets (refresh tokens, email links): minted from the OS CSPRNG and stored as a plain
/// SHA256 hash. The opposite of a password on purpose: a random 256-bit token needs no slow salted hash, and a
/// deterministic hash is what lets a presented token be looked up at all. Low-entropy secrets (passwords, 6-digit
/// codes) go through <see cref="IPasswordHasher"/> instead.</summary>
public static class OpaqueTokens
{
    private const int TokenBytes = 32;

    /// <summary>Mint a fresh token and its lookup hash.</summary>
    public static OpaqueToken Issue()
    {
        var raw = Base64Url.EncodeToString(RandomNumberGenerator.GetBytes(TokenBytes));
        return new OpaqueToken(raw, Hash(raw));
    }

    /// <summary>The lookup hash of a presented token (SHA256, uppercase hex).</summary>
    public static string Hash(string raw) =>
        Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes(raw ?? "")));

    /// <summary>Whether <paramref name="raw"/> hashes to <paramref name="storedHash"/>, compared in constant time.
    /// The services re-check every row a store hands back with this, so a store whose lookup is looser than exact (a
    /// case-insensitive collation, a trimmed column) can never turn a near-miss into a match.</summary>
    public static bool Matches(string raw, string storedHash)
    {
        if (string.IsNullOrEmpty(raw) || string.IsNullOrEmpty(storedHash))
            return false;
        return CryptographicOperations.FixedTimeEquals(
            Encoding.ASCII.GetBytes(Hash(raw)),
            Encoding.ASCII.GetBytes(storedHash));
    }
}
