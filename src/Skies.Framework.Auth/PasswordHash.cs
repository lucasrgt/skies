namespace Skies.Framework.Auth;

/// <summary>A stored password hash: an opaque string an <see cref="IPasswordHasher"/> produced and alone can check.
/// The app persists it on its own user entity (one string column, through <see cref="FromStored"/> and
/// <see cref="Value"/>), while the algorithm, its parameters, and the constant-time check live in the package, so a
/// hashing fix reaches every app through a version bump instead of a hand-edited copy.</summary>
/// <remarks>The type carries no crypto on purpose: a value that could hash itself would pin the algorithm into the
/// entity. It is a value, not a service, so an entity factory can take one without seeing the hasher.</remarks>
public readonly record struct PasswordHash
{
    /// <summary>The encoded hash, persisted as-is. Its format belongs to the hasher that produced it.</summary>
    public string Value { get; }

    private PasswordHash(string value) => Value = value;

    /// <summary>No usable password: the account signs in some other way (an external identity). No hasher ever
    /// produces it and every verification against it fails after the same work a real check costs, so a password
    /// attempt on such an account is indistinguishable by timing from a wrong password.</summary>
    public static PasswordHash None { get; } = new("");

    /// <summary>Rehydrate a hash read back from storage (the EF value converter's read side). It only wraps the
    /// string; verification decides what it is worth.</summary>
    public static PasswordHash FromStored(string value) => new(value ?? "");

    /// <summary>Redacted, so a hash never lands in a log line or an exception message by accident.</summary>
    public override string ToString() => "PasswordHash(redacted)";
}
