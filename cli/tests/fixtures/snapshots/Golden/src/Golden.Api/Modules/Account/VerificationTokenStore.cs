using Skies.Framework.Auth;
using Microsoft.EntityFrameworkCore;

namespace Golden.Api.Modules.Account;

/// <summary>The Account module's side of verification secrets: the <see cref="VerificationToken"/> table behind the
/// framework's <see cref="VerificationTokens"/>. Plain data access with no decisions in it; expiry, single use, purpose
/// binding, and the attempt cap are the package's. Registered by <c>AddVerificationTokens</c> in <see cref="AccountModule"/>.</summary>
public sealed class VerificationTokenStore(AppDb db) : IVerificationStore
{
    public async Task AddAsync(VerificationRecord record, CancellationToken ct)
    {
        db.VerificationTokens.Add(VerificationToken
            .Issue(record.UserId, record.Purpose, record.Target, record.SecretHash, record.CreatedAt, record.ExpiresAt).Value);
        await db.SaveChangesAsync(ct);
    }

    public async Task<VerificationRecord?> FindBySecretAsync(string purpose, string secretHash, CancellationToken ct) =>
        Record(await db.VerificationTokens.FirstOrDefaultAsync(t => t.Purpose == purpose && t.SecretHash == secretHash, ct));

    public async Task<VerificationRecord?> FindLatestActiveAsync(Guid userId, string purpose, DateTime now, CancellationToken ct) =>
        Record(await db.VerificationTokens
            .Where(t => t.UserId == userId && t.Purpose == purpose && t.ConsumedAt == null && t.ExpiresAt > now)
            .OrderByDescending(t => t.CreatedAt)
            .FirstOrDefaultAsync(ct));

    public async Task RecordFailedAttemptAsync(Guid id, DateTime? lockedAt, CancellationToken ct)
    {
        var token = await db.VerificationTokens.SingleAsync(t => t.Id == id, ct);
        token.RecordFailedAttempt();
        if (lockedAt is { } at)
            token.Consume(at);
        await db.SaveChangesAsync(ct);
    }

    public async Task ConsumeAsync(Guid id, DateTime consumedAt, CancellationToken ct)
    {
        var token = await db.VerificationTokens.SingleAsync(t => t.Id == id, ct);
        token.Consume(consumedAt);
        await db.SaveChangesAsync(ct);
    }

    private static VerificationRecord? Record(VerificationToken? t) =>
        t is null ? null : new(t.Id, t.UserId, t.Purpose, t.Target, t.SecretHash, t.CreatedAt, t.ExpiresAt, t.ConsumedAt, t.Attempts);
}
