using Skies.Framework.Auth;

namespace Skies.Framework.Auth.Tests;

// Refresh rotation is the security feature the Account module rests on: a token is good once, a spent token
// presented again burns its whole family (theft), a session cannot outlive its absolute ceiling, and revocation ends
// exactly the sessions it names. Each case drives RefreshSessions over a list-backed store.
public class RefreshSessionsTests
{
    private static readonly Guid User = Guid.NewGuid();
    private readonly ManualClock clock = new();
    private readonly ListRefreshStore store = new();

    private RefreshSessions Sessions(RefreshSessionOptions? options = null) =>
        new(store, options ?? RefreshSessionOptions.Default, clock);

    [Fact]
    public async Task Starting_a_session_opens_a_family_and_stores_only_the_hash()
    {
        var issued = await Sessions().StartAsync(User, default);

        var slot = Assert.Single(store.Slots);
        Assert.Equal(issued.FamilyId, slot.FamilyId);
        Assert.Equal(User, slot.UserId);
        Assert.Equal(OpaqueTokens.Hash(issued.Token), slot.TokenHash);
        Assert.DoesNotContain(store.Slots, s => s.TokenHash == issued.Token);
        Assert.Equal(clock.Now.UtcDateTime + RefreshSessionOptions.Default.Lifetime, issued.ExpiresAt);
        Assert.Equal(issued.ExpiresAt, slot.ExpiresAt);
    }

    [Fact]
    public async Task Rotation_spends_the_token_and_issues_a_new_one_in_the_same_family()
    {
        var sessions = Sessions();
        var issued = await sessions.StartAsync(User, default);

        var rotation = await sessions.RotateAsync(issued.Token, default);

        Assert.True(rotation.Rotated);
        Assert.Equal(User, rotation.UserId);
        Assert.Equal(issued.FamilyId, rotation.FamilyId);
        Assert.NotEqual(issued.Token, rotation.Token);
        Assert.Equal(2, store.Slots.Count(s => s.FamilyId == issued.FamilyId));
        Assert.NotNull(store.Slots.Single(s => s.TokenHash == OpaqueTokens.Hash(issued.Token)).UsedAt);
        Assert.True((await sessions.RotateAsync(rotation.Token, default)).Rotated);
    }

    [Fact]
    public async Task Replaying_a_spent_token_burns_the_whole_family_including_the_live_token()
    {
        var sessions = Sessions();
        var issued = await sessions.StartAsync(User, default);
        var other = await sessions.StartAsync(User, default);
        var live = await sessions.RotateAsync(issued.Token, default);

        var replay = await sessions.RotateAsync(issued.Token, default);

        Assert.Equal(RefreshOutcome.Reused, replay.Outcome);
        Assert.Equal(issued.FamilyId, replay.FamilyId);
        Assert.DoesNotContain(store.Slots, s => s.FamilyId == issued.FamilyId);
        Assert.Equal(RefreshOutcome.Unknown, (await sessions.RotateAsync(live.Token, default)).Outcome);
        Assert.True((await sessions.RotateAsync(other.Token, default)).Rotated);   // another family is untouched
    }

    [Fact]
    public async Task An_expired_token_does_not_rotate()
    {
        var sessions = Sessions();
        var issued = await sessions.StartAsync(User, default);
        clock.Advance(RefreshSessionOptions.Default.Lifetime + TimeSpan.FromSeconds(1));

        var rotation = await sessions.RotateAsync(issued.Token, default);

        Assert.Equal(RefreshOutcome.Expired, rotation.Outcome);
        Assert.Empty(rotation.Token);
    }

    [Fact]
    public async Task A_family_past_its_ceiling_is_burned_even_while_it_keeps_sliding()
    {
        var sessions = Sessions(new RefreshSessionOptions { Lifetime = TimeSpan.FromDays(10), FamilyMaxAge = TimeSpan.FromDays(25) });
        var token = (await sessions.StartAsync(User, default)).Token;
        for (var hop = 0; hop < 4; hop++)
        {
            clock.Advance(TimeSpan.FromDays(8));
            var rotation = await sessions.RotateAsync(token, default);
            if (!rotation.Rotated)
            {
                Assert.Equal(RefreshOutcome.FamilyExpired, rotation.Outcome);
                Assert.Equal(3, hop);   // day 32: still inside its 10-day token lifetime, past the 25-day ceiling
                Assert.Empty(store.Slots);
                return;
            }
            token = rotation.Token;
        }
        Assert.Fail("a family 32 days old slid past a 25-day ceiling");
    }

    [Theory]
    [InlineData("")]
    [InlineData("never-issued")]
    public async Task An_unknown_token_is_refused(string token)
    {
        await Sessions().StartAsync(User, default);

        Assert.Equal(RefreshOutcome.Unknown, (await Sessions().RotateAsync(token, default)).Outcome);
        Assert.Single(store.Slots);
    }

    // The service re-checks the hash itself, so a store answering with the wrong row cannot hand out a session.
    [Fact]
    public async Task A_store_that_returns_the_wrong_row_does_not_turn_a_bad_token_into_a_session()
    {
        var sessions = Sessions();
        await sessions.StartAsync(User, default);
        store.AnswerEveryLookupWith = store.Slots[0];

        Assert.Equal(RefreshOutcome.Unknown, (await sessions.RotateAsync("attacker-guess", default)).Outcome);
        Assert.Null(store.Slots[0].UsedAt);
    }

    [Fact]
    public async Task Losing_the_concurrent_rotation_race_is_superseded_not_theft()
    {
        var sessions = Sessions();
        var issued = await sessions.StartAsync(User, default);
        store.LoseNextRotation = true;

        var rotation = await sessions.RotateAsync(issued.Token, default);

        Assert.Equal(RefreshOutcome.Superseded, rotation.Outcome);
        Assert.Single(store.Slots);   // the family survives; the winner's token is the live one
    }

    [Fact]
    public async Task Revoking_by_token_ends_that_family_and_is_silent_for_an_unknown_token()
    {
        var sessions = Sessions();
        var issued = await sessions.StartAsync(User, default);
        var other = await sessions.StartAsync(User, default);

        await sessions.RevokeAsync("never-issued", default);
        await sessions.RevokeAsync(issued.Token, default);

        Assert.Equal(RefreshOutcome.Unknown, (await sessions.RotateAsync(issued.Token, default)).Outcome);
        Assert.True((await sessions.RotateAsync(other.Token, default)).Rotated);
    }

    [Fact]
    public async Task Revoking_others_keeps_the_current_family_and_other_users()
    {
        var sessions = Sessions();
        var current = await sessions.StartAsync(User, default);
        var second = await sessions.StartAsync(User, default);
        var bystander = await sessions.StartAsync(Guid.NewGuid(), default);

        await sessions.RevokeOthersAsync(User, current.FamilyId, default);

        Assert.True((await sessions.RotateAsync(current.Token, default)).Rotated);
        Assert.Equal(RefreshOutcome.Unknown, (await sessions.RotateAsync(second.Token, default)).Outcome);
        Assert.True((await sessions.RotateAsync(bystander.Token, default)).Rotated);
    }

    [Fact]
    public async Task Revoking_all_ends_every_family_of_the_user_only()
    {
        var sessions = Sessions();
        await sessions.StartAsync(User, default);
        await sessions.StartAsync(User, default);
        var bystander = await sessions.StartAsync(Guid.NewGuid(), default);

        await sessions.RevokeAllAsync(User, default);

        Assert.DoesNotContain(store.Slots, s => s.UserId == User);
        Assert.True((await sessions.RotateAsync(bystander.Token, default)).Rotated);
    }

    [Fact]
    public async Task Revoking_a_family_by_id_ends_it()
    {
        var sessions = Sessions();
        var issued = await sessions.StartAsync(User, default);

        await sessions.RevokeFamilyAsync(issued.FamilyId, default);

        Assert.Empty(store.Slots);
    }
}
