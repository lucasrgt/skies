namespace Skies.Framework.Auth;

/// <summary>How long a login lasts: the app's policy, with the defaults the Skies blueprint has always shipped.</summary>
public sealed record RefreshSessionOptions
{
    /// <summary>The defaults: a 14-day sliding token inside a 90-day absolute ceiling.</summary>
    public static RefreshSessionOptions Default { get; } = new();

    /// <summary>How long one refresh token stays valid. Every rotation slides it; it is also the web cookie's
    /// lifetime, so the row's expiry and the cookie's can never drift apart.</summary>
    public TimeSpan Lifetime { get; init; } = TimeSpan.FromDays(14);

    /// <summary>The absolute ceiling on a session's age, measured from its first token. A family may rotate freely
    /// inside it; past it the next refresh retires the family and forces a fresh login, however often it slid.</summary>
    public TimeSpan FamilyMaxAge { get; init; } = TimeSpan.FromDays(90);
}

/// <summary>A session just opened: the family (the access token's <c>sid</c>) and its first refresh token.</summary>
/// <param name="FamilyId">The new family; put it in the access token as the session id.</param>
/// <param name="Token">The raw refresh token, returned to the client once and stored only as a hash.</param>
/// <param name="ExpiresAt">When the token stops refreshing (UTC).</param>
public sealed record IssuedRefresh(Guid FamilyId, string Token, DateTime ExpiresAt);

/// <summary>Why a refresh did or did not rotate. Every value but <see cref="Rotated"/> is a refusal; the slice maps
/// each to its own error code.</summary>
public enum RefreshOutcome
{
    /// <summary>The token was live: it is spent now and <see cref="RefreshRotation.Token"/> replaces it.</summary>
    Rotated,

    /// <summary>No slot carries this token (never issued, revoked, or malformed).</summary>
    Unknown,

    /// <summary>The token outlived <see cref="RefreshSessionOptions.Lifetime"/>.</summary>
    Expired,

    /// <summary>The token was already rotated away, so it leaked: the whole family has been burned.</summary>
    Reused,

    /// <summary>The family outlived <see cref="RefreshSessionOptions.FamilyMaxAge"/>; it has been burned.</summary>
    FamilyExpired,

    /// <summary>A concurrent refresh of the same live token won the race and already delivered the replacement;
    /// transient, so the client retries with the token it now holds.</summary>
    Superseded,
}

/// <summary>The result of <see cref="RefreshSessions.RotateAsync"/>.</summary>
/// <param name="Outcome">What happened; only <see cref="RefreshOutcome.Rotated"/> carries a token.</param>
/// <param name="UserId">The session's user when the token was found, else <see cref="Guid.Empty"/>.</param>
/// <param name="FamilyId">The session (family) when the token was found, else <see cref="Guid.Empty"/>.</param>
/// <param name="Token">The replacement raw refresh token when rotated, else empty.</param>
/// <param name="ExpiresAt">The replacement's expiry (UTC) when rotated.</param>
public sealed record RefreshRotation(RefreshOutcome Outcome, Guid UserId, Guid FamilyId, string Token, DateTime ExpiresAt)
{
    /// <summary>Whether the token rotated and a replacement was issued.</summary>
    public bool Rotated => Outcome == RefreshOutcome.Rotated;

    internal static RefreshRotation Refused(RefreshOutcome outcome, RefreshSlot? slot = null) =>
        new(outcome, slot?.UserId ?? Guid.Empty, slot?.FamilyId ?? Guid.Empty, "", default);
}

/// <summary>Refresh sessions as a mechanism: opening a family on sign-in, rotating on every refresh with reuse
/// detection (a spent token presented again burns its whole family), the sliding lifetime and the absolute ceiling,
/// and revocation. The app keeps its session entity and table behind <see cref="IRefreshSessionStore"/>, and its
/// slices keep the error codes and who may revoke what; everything a silent edit could weaken lives here.</summary>
/// <remarks>Registered (scoped) by <see cref="SkiesAuthExtensions.AddSkiesAuth{TSessionStore}"/>.</remarks>
public sealed class RefreshSessions(IRefreshSessionStore store, RefreshSessionOptions options, TimeProvider clock)
{
    /// <summary>The lifetime policy in force.</summary>
    public RefreshSessionOptions Options => options;

    /// <summary>Open a new family for <paramref name="userId"/> (a sign-in) and issue its first token.</summary>
    public async Task<IssuedRefresh> StartAsync(Guid userId, CancellationToken ct)
    {
        var now = Now();
        var family = Guid.NewGuid();
        var (raw, hash) = OpaqueTokens.Issue();
        var expires = now.Add(options.Lifetime);
        await store.AddAsync(new RefreshSlot(Guid.NewGuid(), userId, family, hash, now, expires, null), ct);
        return new IssuedRefresh(family, raw, expires);
    }

    /// <summary>Exchange a presented refresh token for its replacement. A spent token burns its family
    /// (<see cref="RefreshOutcome.Reused"/>): the legitimate client already holds the live replacement, so a second
    /// presentation means the token leaked, and the thief's copy and the victim's alike must die. There is no replay
    /// grace window; a genuinely simultaneous refresh loses the store's concurrency check instead
    /// (<see cref="RefreshOutcome.Superseded"/>).</summary>
    public async Task<RefreshRotation> RotateAsync(string presentedToken, CancellationToken ct)
    {
        var slot = await FindAsync(presentedToken, ct);
        if (slot is null)
            return RefreshRotation.Refused(RefreshOutcome.Unknown);

        var now = Now();
        if (slot.UsedAt is not null)
        {
            await store.RemoveFamilyAsync(slot.FamilyId, ct);
            return RefreshRotation.Refused(RefreshOutcome.Reused, slot);
        }

        if (slot.ExpiresAt < now)
            return RefreshRotation.Refused(RefreshOutcome.Expired, slot);

        var familyStart = await store.FamilyStartedAtAsync(slot.FamilyId, ct);
        if (now - familyStart > options.FamilyMaxAge)
        {
            await store.RemoveFamilyAsync(slot.FamilyId, ct);
            return RefreshRotation.Refused(RefreshOutcome.FamilyExpired, slot);
        }

        var (raw, hash) = OpaqueTokens.Issue();
        var expires = now.Add(options.Lifetime);
        var next = new RefreshSlot(Guid.NewGuid(), slot.UserId, slot.FamilyId, hash, now, expires, null);
        if (!await store.TryRotateAsync(slot, now, next, ct))
            return RefreshRotation.Refused(RefreshOutcome.Superseded, slot);

        return new RefreshRotation(RefreshOutcome.Rotated, slot.UserId, slot.FamilyId, raw, expires);
    }

    /// <summary>Sign out the session a refresh token belongs to (its whole family). Idempotent and silent: an unknown
    /// token is a no-op, so the caller never learns whether a token was valid.</summary>
    public async Task RevokeAsync(string presentedToken, CancellationToken ct)
    {
        var slot = await FindAsync(presentedToken, ct);
        if (slot is not null)
            await store.RemoveFamilyAsync(slot.FamilyId, ct);
    }

    /// <summary>End one session (family) by its id. Whether the caller may is the slice's check, made first.</summary>
    public Task RevokeFamilyAsync(Guid familyId, CancellationToken ct) => store.RemoveFamilyAsync(familyId, ct);

    /// <summary>End every session of the user: after a credential change the attacker's families die too.</summary>
    public Task RevokeAllAsync(Guid userId, CancellationToken ct) => store.RemoveUserSessionsAsync(userId, null, ct);

    /// <summary>End every session of the user except <paramref name="keepFamilyId"/> ("sign out everywhere else").</summary>
    public Task RevokeOthersAsync(Guid userId, Guid keepFamilyId, CancellationToken ct) =>
        store.RemoveUserSessionsAsync(userId, keepFamilyId, ct);

    private async Task<RefreshSlot?> FindAsync(string presentedToken, CancellationToken ct)
    {
        if (string.IsNullOrEmpty(presentedToken))
            return null;
        var slot = await store.FindAsync(OpaqueTokens.Hash(presentedToken), ct);
        return slot is not null && OpaqueTokens.Matches(presentedToken, slot.TokenHash) ? slot : null;
    }

    private DateTime Now() => clock.GetUtcNow().UtcDateTime;
}
