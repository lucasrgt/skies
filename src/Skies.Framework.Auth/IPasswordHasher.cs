namespace Skies.Framework.Auth;

/// <summary>Hashes and checks low-entropy secrets: passwords and short one-time codes. The mechanism (a slow, salted
/// derivation compared in constant time) is the package's; what counts as an acceptable password (length, strength)
/// is the app's policy and stays in its slices. <see cref="Argon2idPasswordHasher"/> is the default, registered by
/// <see cref="SkiesAuthExtensions.AddSkiesAuth{TSessionStore}"/>; register another implementation after that call
/// to move to a different algorithm without touching a slice.</summary>
public interface IPasswordHasher
{
    /// <summary>The longest password, in characters, a hasher accepts. A slow hash is the cost an attacker pays per
    /// guess, and an unbounded input lets one request buy far more of it than a real password needs, so both members
    /// refuse anything longer before any derivation runs. 128 characters holds any passphrase or manager-generated
    /// password; the app's validation reports the limit to the user with this same constant.</summary>
    const int MaxPasswordLength = 128;

    /// <summary>Hash <paramref name="password"/> with a fresh random salt. An empty password, or one longer than
    /// <see cref="MaxPasswordLength"/>, is refused with an <see cref="ArgumentException"/>: no account can be given
    /// one, so none can ever verify.</summary>
    PasswordHash Hash(string password);

    /// <summary>Whether <paramref name="password"/> matches <paramref name="stored"/>. A <see langword="null"/> hash
    /// (no account was found), an unreadable one, or a password longer than <see cref="MaxPasswordLength"/> still
    /// costs one bounded derivation against a fixed dummy and returns <see langword="false"/>, so "no such account"
    /// and "wrong password" take the same time: pass the looked-up user's hash straight in, found or not.</summary>
    bool Verify(string password, PasswordHash? stored);
}
