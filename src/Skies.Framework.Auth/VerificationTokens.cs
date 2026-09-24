using System.Globalization;
using System.Security.Cryptography;

namespace Skies.Framework.Auth;

/// <summary>The verification policy knobs, with the defaults the Skies blueprint has always shipped.</summary>
public sealed record VerificationOptions
{
    /// <summary>The defaults: five wrong guesses lock a code.</summary>
    public static VerificationOptions Default { get; } = new();

    /// <summary>Wrong guesses after which a code locks (is consumed) and even the right code fails. A 6-digit code
    /// spans only 10^6 values, so unbounded guessing is a brute force.</summary>
    public int MaxCodeAttempts { get; init; } = 5;
}

/// <summary>Why a verification did or did not succeed. The slice maps each refusal to its own error code.</summary>
public enum VerificationOutcome
{
    /// <summary>The secret matched and is consumed now; it never verifies again.</summary>
    Verified,

    /// <summary>No usable secret: unknown, for another purpose, expired, or already consumed.</summary>
    NotFound,

    /// <summary>A wrong code; the guess was counted and the code stays usable.</summary>
    WrongCode,

    /// <summary>A wrong code that hit <see cref="VerificationOptions.MaxCodeAttempts"/>; the code is consumed.</summary>
    Locked,
}

/// <summary>The result of consuming a link token or verifying a code.</summary>
/// <param name="Outcome">What happened.</param>
/// <param name="UserId">Whose secret it was, when one was found; else <see cref="Guid.Empty"/>.</param>
/// <param name="Target">What was verified (the phone a code went to), when the record carried one.</param>
public sealed record VerificationCheck(VerificationOutcome Outcome, Guid UserId, string? Target)
{
    /// <summary>Whether the secret verified.</summary>
    public bool Verified => Outcome == VerificationOutcome.Verified;

    internal static VerificationCheck Refused(VerificationOutcome outcome, VerificationRecord? record = null) =>
        new(outcome, record?.UserId ?? Guid.Empty, record?.Target);
}

/// <summary>Single-use verification secrets as a mechanism: emailed link tokens (confirm an email, reset a password)
/// and SMS codes (verify a phone). Issuance, the storage form of each kind, expiry, single use, purpose binding, and
/// the brute-force cap live here; the app keeps its table behind <see cref="IVerificationStore"/>, and its slices keep
/// each purpose's lifetime, the message text, and what a verified secret unlocks.</summary>
/// <remarks>Registered (scoped) by <see cref="SkiesAuthExtensions.AddVerificationTokens{TStore}"/>.</remarks>
public sealed class VerificationTokens(
    IVerificationStore store,
    IPasswordHasher hasher,
    VerificationOptions options,
    TimeProvider clock)
{
    /// <summary>Issue a link token for <paramref name="userId"/>, valid for <paramref name="lifetime"/>. Returns the raw
    /// token to send (once); only its hash is stored.</summary>
    public async Task<string> IssueLinkAsync(Guid userId, string purpose, TimeSpan lifetime, CancellationToken ct)
    {
        var now = Now();
        var (raw, hash) = OpaqueTokens.Issue();
        await store.AddAsync(new VerificationRecord(Guid.NewGuid(), userId, purpose, null, hash, now, now.Add(lifetime), null, 0), ct);
        return raw;
    }

    /// <summary>Consume a presented link token of <paramref name="purpose"/>. A token verifies once, before it
    /// expires, and only for the purpose it was issued for.</summary>
    public async Task<VerificationCheck> ConsumeLinkAsync(string purpose, string token, CancellationToken ct)
    {
        if (string.IsNullOrEmpty(token))
            return VerificationCheck.Refused(VerificationOutcome.NotFound);

        var now = Now();
        var record = await store.FindBySecretAsync(purpose, OpaqueTokens.Hash(token), ct);
        if (record is null || record.Purpose != purpose || !OpaqueTokens.Matches(token, record.SecretHash) || !IsActive(record, now))
            return VerificationCheck.Refused(VerificationOutcome.NotFound);

        await store.ConsumeAsync(record.Id, now, ct);
        return new VerificationCheck(VerificationOutcome.Verified, record.UserId, record.Target);
    }

    /// <summary>Issue a 6-digit code for <paramref name="userId"/> proving <paramref name="target"/> (the phone it is
    /// sent to), valid for <paramref name="lifetime"/>. Returns the code to send (once); it is stored only through the
    /// slow hasher, since a fast hash of 10^6 values is brute-forced offline in moments.</summary>
    public async Task<string> IssueCodeAsync(Guid userId, string purpose, string target, TimeSpan lifetime, CancellationToken ct)
    {
        var now = Now();
        var code = RandomNumberGenerator.GetInt32(0, 1_000_000).ToString("D6", CultureInfo.InvariantCulture);
        var hash = hasher.Hash(code).Value;
        await store.AddAsync(new VerificationRecord(Guid.NewGuid(), userId, purpose, target, hash, now, now.Add(lifetime), null, 0), ct);
        return code;
    }

    /// <summary>Check <paramref name="code"/> against the user's latest active code of <paramref name="purpose"/>.
    /// Every wrong guess is counted; the one that reaches <see cref="VerificationOptions.MaxCodeAttempts"/> consumes the
    /// code, so no further guess (right or wrong) is ever tried. A right code within the limit always wins.</summary>
    public async Task<VerificationCheck> VerifyCodeAsync(Guid userId, string purpose, string code, CancellationToken ct)
    {
        var now = Now();
        var record = await store.FindLatestActiveAsync(userId, purpose, now, ct);
        if (record is null || record.UserId != userId || record.Purpose != purpose || !IsActive(record, now))
            return VerificationCheck.Refused(VerificationOutcome.NotFound);

        if (record.Attempts >= options.MaxCodeAttempts)
        {
            await store.ConsumeAsync(record.Id, now, ct);
            return VerificationCheck.Refused(VerificationOutcome.Locked, record);
        }

        if (!hasher.Verify(code ?? "", PasswordHash.FromStored(record.SecretHash)))
        {
            var locked = record.Attempts + 1 >= options.MaxCodeAttempts;
            await store.RecordFailedAttemptAsync(record.Id, locked ? now : null, ct);
            return VerificationCheck.Refused(locked ? VerificationOutcome.Locked : VerificationOutcome.WrongCode, record);
        }

        await store.ConsumeAsync(record.Id, now, ct);
        return new VerificationCheck(VerificationOutcome.Verified, record.UserId, record.Target);
    }

    private static bool IsActive(VerificationRecord record, DateTime now) =>
        record.ConsumedAt is null && record.ExpiresAt > now;

    private DateTime Now() => clock.GetUtcNow().UtcDateTime;
}
