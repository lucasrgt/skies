namespace Skies.Framework.Auth;

/// <summary>One single-use verification secret as <see cref="VerificationTokens"/> sees it: a row of the app's own
/// table. Two kinds share the shape. A <em>link</em> token (email confirmation, password reset) is high-entropy and
/// stored as an <see cref="OpaqueTokens.Hash"/>, so it is found by that hash. A <em>code</em> (a 6-digit SMS code) is
/// low-entropy, so it is stored through <see cref="IPasswordHasher"/> and found by user and purpose instead.</summary>
/// <param name="Id">The record's identity, assigned by <see cref="VerificationTokens"/> when it is issued.</param>
/// <param name="UserId">The user the secret was issued to.</param>
/// <param name="Purpose">What the secret proves (the app's vocabulary, e.g. <c>password-reset</c>). A secret only
/// ever verifies for the purpose it was issued for.</param>
/// <param name="Target">What is being verified, when it is not the account itself (the phone number a code went
/// to); <see langword="null"/> otherwise.</param>
/// <param name="SecretHash">The stored form of the secret; the secret itself is never stored.</param>
/// <param name="CreatedAt">When it was issued (UTC).</param>
/// <param name="ExpiresAt">When it stops verifying (UTC).</param>
/// <param name="ConsumedAt">When it was used or locked (UTC); <see langword="null"/> while it can still verify.</param>
/// <param name="Attempts">Wrong guesses so far (codes only): the brute-force counter.</param>
public sealed record VerificationRecord(
    Guid Id,
    Guid UserId,
    string Purpose,
    string? Target,
    string SecretHash,
    DateTime CreatedAt,
    DateTime ExpiresAt,
    DateTime? ConsumedAt,
    int Attempts);

/// <summary>The persistence <see cref="VerificationTokens"/> runs on, implemented by the app over its own entity and
/// DbContext. The package owns issuance, expiry, single use, and the attempt cap; the app owns the table. Every
/// member persists its change before it returns.</summary>
/// <remarks>Keep the implementation a plain data adapter. The service re-checks purpose, expiry, use, and (for link
/// tokens) the hash in constant time on whatever it is handed, so a loose store can only make a secret fail.</remarks>
public interface IVerificationStore
{
    /// <summary>Persist a newly issued record.</summary>
    Task AddAsync(VerificationRecord record, CancellationToken ct);

    /// <summary>The record of <paramref name="purpose"/> whose secret hash is exactly <paramref name="secretHash"/>,
    /// in whatever state, or <see langword="null"/> (link tokens).</summary>
    Task<VerificationRecord?> FindBySecretAsync(string purpose, string secretHash, CancellationToken ct);

    /// <summary>The user's most recently issued record of <paramref name="purpose"/> that is neither consumed nor
    /// expired at <paramref name="now"/>, or <see langword="null"/> (codes).</summary>
    Task<VerificationRecord?> FindLatestActiveAsync(Guid userId, string purpose, DateTime now, CancellationToken ct);

    /// <summary>Count one wrong guess against the record and, when <paramref name="lockedAt"/> is given, consume it
    /// at that time so no further guess can ever be tried.</summary>
    Task RecordFailedAttemptAsync(Guid id, DateTime? lockedAt, CancellationToken ct);

    /// <summary>Consume the record at <paramref name="consumedAt"/>: a used secret never verifies again.</summary>
    Task ConsumeAsync(Guid id, DateTime consumedAt, CancellationToken ct);
}
