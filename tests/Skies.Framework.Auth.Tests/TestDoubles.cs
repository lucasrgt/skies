using Skies.Framework.Auth;

namespace Skies.Framework.Auth.Tests;

/// <summary>A clock the test moves by hand.</summary>
internal sealed class ManualClock(DateTimeOffset start) : TimeProvider
{
    public ManualClock() : this(new DateTimeOffset(2026, 1, 1, 12, 0, 0, TimeSpan.Zero)) { }

    public DateTimeOffset Now { get; set; } = start;

    public override DateTimeOffset GetUtcNow() => Now;

    public void Advance(TimeSpan by) => Now += by;
}

/// <summary>The app side of <see cref="IRefreshSessionStore"/>, over a list: exactly what an EF adapter does, minus
/// the database. <see cref="LoseNextRotation"/> plays the concurrency conflict a relational provider raises.</summary>
internal sealed class ListRefreshStore : IRefreshSessionStore
{
    public List<RefreshSlot> Slots { get; } = [];

    public bool LoseNextRotation { get; set; }

    /// <summary>Plays a store whose lookup is looser than exact: it answers every hash with this slot.</summary>
    public RefreshSlot? AnswerEveryLookupWith { get; set; }

    public Task<RefreshSlot?> FindAsync(string tokenHash, CancellationToken ct) =>
        Task.FromResult(AnswerEveryLookupWith ?? Slots.FirstOrDefault(s => s.TokenHash == tokenHash));

    public Task<DateTime> FamilyStartedAtAsync(Guid familyId, CancellationToken ct) =>
        Task.FromResult(Slots.Where(s => s.FamilyId == familyId).Min(s => s.CreatedAt));

    public Task AddAsync(RefreshSlot slot, CancellationToken ct)
    {
        Slots.Add(slot);
        return Task.CompletedTask;
    }

    public Task<bool> TryRotateAsync(RefreshSlot spent, DateTime usedAt, RefreshSlot next, CancellationToken ct)
    {
        if (LoseNextRotation)
        {
            LoseNextRotation = false;
            return Task.FromResult(false);
        }
        var index = Slots.FindIndex(s => s.Id == spent.Id);
        Slots[index] = Slots[index] with { UsedAt = usedAt };
        Slots.Add(next);
        return Task.FromResult(true);
    }

    public Task RemoveFamilyAsync(Guid familyId, CancellationToken ct)
    {
        Slots.RemoveAll(s => s.FamilyId == familyId);
        return Task.CompletedTask;
    }

    public Task RemoveUserSessionsAsync(Guid userId, Guid? keepFamilyId, CancellationToken ct)
    {
        Slots.RemoveAll(s => s.UserId == userId && s.FamilyId != keepFamilyId);
        return Task.CompletedTask;
    }
}

/// <summary>The app side of <see cref="IVerificationStore"/>, over a list.</summary>
internal sealed class ListVerificationStore : IVerificationStore
{
    public List<VerificationRecord> Records { get; } = [];

    public Task AddAsync(VerificationRecord record, CancellationToken ct)
    {
        Records.Add(record);
        return Task.CompletedTask;
    }

    public Task<VerificationRecord?> FindBySecretAsync(string purpose, string secretHash, CancellationToken ct) =>
        Task.FromResult(Records.FirstOrDefault(r => r.Purpose == purpose && r.SecretHash == secretHash));

    public Task<VerificationRecord?> FindLatestActiveAsync(Guid userId, string purpose, DateTime now, CancellationToken ct) =>
        Task.FromResult(Records
            .Where(r => r.UserId == userId && r.Purpose == purpose && r.ConsumedAt == null && r.ExpiresAt > now)
            .OrderByDescending(r => r.CreatedAt)
            .FirstOrDefault());

    public Task RecordFailedAttemptAsync(Guid id, DateTime? lockedAt, CancellationToken ct)
    {
        Update(id, r => r with { Attempts = r.Attempts + 1, ConsumedAt = lockedAt ?? r.ConsumedAt });
        return Task.CompletedTask;
    }

    public Task ConsumeAsync(Guid id, DateTime consumedAt, CancellationToken ct)
    {
        Update(id, r => r with { ConsumedAt = consumedAt });
        return Task.CompletedTask;
    }

    private void Update(Guid id, Func<VerificationRecord, VerificationRecord> change)
    {
        var index = Records.FindIndex(r => r.Id == id);
        Records[index] = change(Records[index]);
    }
}
