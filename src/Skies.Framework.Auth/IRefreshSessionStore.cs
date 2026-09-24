namespace Skies.Framework.Auth;

/// <summary>One refresh-token slot as <see cref="RefreshSessions"/> sees it: a row of the app's own session table.
/// A login opens a <paramref name="FamilyId"/> (the session, carried as the access token's <c>sid</c>); every
/// rotation spends the presented slot and appends the next one to the same family.</summary>
/// <param name="Id">The row's identity. The service only ever hands back an id the store itself returned, so a store
/// may keep its own ids and ignore the one on a slot it is asked to add.</param>
/// <param name="UserId">The user the session belongs to.</param>
/// <param name="FamilyId">The lineage the slot belongs to; revocation and reuse detection act on it.</param>
/// <param name="TokenHash">The token's <see cref="OpaqueTokens.Hash"/>; the raw token is never stored.</param>
/// <param name="CreatedAt">When the slot was minted (UTC).</param>
/// <param name="ExpiresAt">When the slot's token stops refreshing (UTC).</param>
/// <param name="UsedAt">When the slot was rotated away (UTC); <see langword="null"/> while it is the live token.</param>
public sealed record RefreshSlot(
    Guid Id,
    Guid UserId,
    Guid FamilyId,
    string TokenHash,
    DateTime CreatedAt,
    DateTime ExpiresAt,
    DateTime? UsedAt);

/// <summary>The persistence <see cref="RefreshSessions"/> runs on, implemented by the app over its own entity and
/// DbContext (a few lines of EF per member). The package owns the decisions (rotation, reuse detection, the family
/// burn, the age ceiling); the app owns the table. Every member persists its change before it returns.</summary>
/// <remarks>Keep the implementation a plain data adapter: no decisions, no filtering beyond what each member says.
/// The service re-checks what it is handed (the token hash in constant time, expiry, use), so a loose store cannot
/// widen what a token is worth, only make it fail.</remarks>
public interface IRefreshSessionStore
{
    /// <summary>The slot whose token hash is exactly <paramref name="tokenHash"/>, in whatever state, or
    /// <see langword="null"/>. Spent and expired slots must come back too: a spent one is how theft is detected.</summary>
    Task<RefreshSlot?> FindAsync(string tokenHash, CancellationToken ct);

    /// <summary>The earliest <see cref="RefreshSlot.CreatedAt"/> in the family: when the session was opened.</summary>
    Task<DateTime> FamilyStartedAtAsync(Guid familyId, CancellationToken ct);

    /// <summary>Persist a new slot (a login opening a family).</summary>
    Task AddAsync(RefreshSlot slot, CancellationToken ct);

    /// <summary>Mark <paramref name="spent"/> used at <paramref name="usedAt"/> and persist <paramref name="next"/>, in
    /// one save. Return <see langword="false"/> when an optimistic-concurrency check shows a concurrent rotation of the
    /// same slot already won (EF's <c>DbUpdateConcurrencyException</c> on a row version); that is the benign race of
    /// two tabs sharing a cookie, not theft.</summary>
    Task<bool> TryRotateAsync(RefreshSlot spent, DateTime usedAt, RefreshSlot next, CancellationToken ct);

    /// <summary>Delete every slot of the family.</summary>
    Task RemoveFamilyAsync(Guid familyId, CancellationToken ct);

    /// <summary>Delete every slot of the user, except those of <paramref name="keepFamilyId"/> when one is given.</summary>
    Task RemoveUserSessionsAsync(Guid userId, Guid? keepFamilyId, CancellationToken ct);
}
