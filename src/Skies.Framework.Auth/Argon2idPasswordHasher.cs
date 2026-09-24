using System.Security.Cryptography;
using System.Text;
using Konscious.Security.Cryptography;

namespace Skies.Framework.Auth;

/// <summary>The default <see cref="IPasswordHasher"/>: argon2id at the OWASP baseline (19 MiB, 2 iterations, one
/// lane, 16-byte salt, 32-byte tag), encoded as <c>base64(salt).base64(hash)</c>. That encoding is the one the
/// Skies 4 blueprint wrote into every app's own <c>PasswordHash</c>, so hashes stored before the move to the package
/// keep verifying unchanged.</summary>
public sealed class Argon2idPasswordHasher : IPasswordHasher
{
    private const int SaltBytes = 16;
    private const int HashBytes = 32;

    // Fixed input for the no-account branch. The value is irrelevant; it only makes Verify do real work.
    private static readonly byte[] DummySalt = new byte[SaltBytes];
    private static readonly byte[] DummyHash = Derive("skies-login-timing-equalizer", DummySalt);

    /// <inheritdoc />
    public PasswordHash Hash(string password)
    {
        ArgumentException.ThrowIfNullOrEmpty(password);
        var salt = RandomNumberGenerator.GetBytes(SaltBytes);
        var hash = Derive(password, salt);
        return PasswordHash.FromStored($"{Convert.ToBase64String(salt)}.{Convert.ToBase64String(hash)}");
    }

    /// <inheritdoc />
    public bool Verify(string password, PasswordHash? stored)
    {
        // An empty password never has a hash (Hash refuses it), so it takes the dummy branch like a missing hash.
        if (!string.IsNullOrEmpty(password) && stored is { } hash && TryDecode(hash.Value, out var salt, out var expected))
            return CryptographicOperations.FixedTimeEquals(Derive(password, salt), expected);

        // The same derivation against a dummy, result discarded: a missing or unusable hash is not observable by timing.
        CryptographicOperations.FixedTimeEquals(Derive(string.IsNullOrEmpty(password) ? "-" : password, DummySalt), DummyHash);
        return false;
    }

    private static bool TryDecode(string encoded, out byte[] salt, out byte[] hash)
    {
        salt = hash = [];
        var parts = encoded.Split('.');
        if (parts.Length != 2)
            return false;
        try
        {
            salt = Convert.FromBase64String(parts[0]);
            hash = Convert.FromBase64String(parts[1]);
        }
        catch (FormatException)
        {
            return false;
        }
        return salt.Length > 0 && hash.Length == HashBytes;
    }

    private static byte[] Derive(string password, byte[] salt)
    {
        using var argon = new Argon2id(Encoding.UTF8.GetBytes(password))
        {
            Salt = salt,
            DegreeOfParallelism = 1,
            MemorySize = 19456,
            Iterations = 2,
        };
        return argon.GetBytes(HashBytes);
    }
}
