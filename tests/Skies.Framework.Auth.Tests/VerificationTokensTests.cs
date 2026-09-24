using Skies.Framework.Auth;

namespace Skies.Framework.Auth.Tests;

// Verification secrets unlock account changes (a confirmed email, a new password, a verified phone), so each is
// good once, only before it expires, only for its purpose, and a code cannot be guessed without limit.
public class VerificationTokensTests
{
    private const string Reset = "password-reset";
    private const string Confirm = "email-verification";
    private const string Phone = "phone";
    private static readonly Guid User = Guid.NewGuid();
    private readonly ManualClock clock = new();
    private readonly ListVerificationStore store = new();

    private VerificationTokens Tokens() => new(store, new Argon2idPasswordHasher(), VerificationOptions.Default, clock);

    [Fact]
    public async Task A_link_token_verifies_once_and_is_stored_only_as_its_hash()
    {
        var tokens = Tokens();
        var raw = await tokens.IssueLinkAsync(User, Reset, TimeSpan.FromHours(1), default);

        Assert.Equal(OpaqueTokens.Hash(raw), Assert.Single(store.Records).SecretHash);
        var first = await tokens.ConsumeLinkAsync(Reset, raw, default);
        var second = await tokens.ConsumeLinkAsync(Reset, raw, default);

        Assert.True(first.Verified);
        Assert.Equal(User, first.UserId);
        Assert.Equal(VerificationOutcome.NotFound, second.Outcome);
    }

    [Fact]
    public async Task A_link_token_only_verifies_for_the_purpose_it_was_issued_for()
    {
        var tokens = Tokens();
        var raw = await tokens.IssueLinkAsync(User, Confirm, TimeSpan.FromHours(24), default);

        Assert.Equal(VerificationOutcome.NotFound, (await tokens.ConsumeLinkAsync(Reset, raw, default)).Outcome);
        Assert.True((await tokens.ConsumeLinkAsync(Confirm, raw, default)).Verified);
    }

    [Fact]
    public async Task An_expired_link_token_is_refused()
    {
        var tokens = Tokens();
        var raw = await tokens.IssueLinkAsync(User, Reset, TimeSpan.FromHours(1), default);
        clock.Advance(TimeSpan.FromHours(1));

        Assert.Equal(VerificationOutcome.NotFound, (await tokens.ConsumeLinkAsync(Reset, raw, default)).Outcome);
        Assert.Null(Assert.Single(store.Records).ConsumedAt);
    }

    [Theory]
    [InlineData("")]
    [InlineData("not-a-token")]
    public async Task An_unknown_link_token_is_refused(string token)
    {
        await Tokens().IssueLinkAsync(User, Reset, TimeSpan.FromHours(1), default);

        Assert.Equal(VerificationOutcome.NotFound, (await Tokens().ConsumeLinkAsync(Reset, token, default)).Outcome);
    }

    [Fact]
    public async Task A_code_is_six_digits_stored_through_the_slow_hasher_and_verifies_once()
    {
        var tokens = Tokens();
        var code = await tokens.IssueCodeAsync(User, Phone, "+5511999999999", TimeSpan.FromMinutes(10), default);

        Assert.Matches("^[0-9]{6}$", code);
        var record = Assert.Single(store.Records);
        Assert.DoesNotContain(code, record.SecretHash, StringComparison.Ordinal);
        Assert.NotEqual(OpaqueTokens.Hash(code), record.SecretHash);
        var check = await tokens.VerifyCodeAsync(User, Phone, code, default);
        Assert.True(check.Verified);
        Assert.Equal("+5511999999999", check.Target);
        Assert.Equal(VerificationOutcome.NotFound, (await tokens.VerifyCodeAsync(User, Phone, code, default)).Outcome);
    }

    [Fact]
    public async Task A_wrong_code_is_counted_and_the_real_code_still_verifies()
    {
        var tokens = Tokens();
        var code = await tokens.IssueCodeAsync(User, Phone, "+1", TimeSpan.FromMinutes(10), default);

        Assert.Equal(VerificationOutcome.WrongCode, (await tokens.VerifyCodeAsync(User, Phone, Wrong(code), default)).Outcome);
        Assert.Equal(1, Assert.Single(store.Records).Attempts);
        Assert.True((await tokens.VerifyCodeAsync(User, Phone, code, default)).Verified);
    }

    [Fact]
    public async Task The_fifth_wrong_code_locks_so_even_the_right_one_fails()
    {
        var tokens = Tokens();
        var code = await tokens.IssueCodeAsync(User, Phone, "+1", TimeSpan.FromMinutes(10), default);

        var outcomes = new List<VerificationOutcome>();
        for (var attempt = 0; attempt < 5; attempt++)
            outcomes.Add((await tokens.VerifyCodeAsync(User, Phone, Wrong(code), default)).Outcome);

        Assert.Equal([.. Enumerable.Repeat(VerificationOutcome.WrongCode, 4), VerificationOutcome.Locked], outcomes);
        Assert.NotNull(Assert.Single(store.Records).ConsumedAt);
        Assert.Equal(VerificationOutcome.NotFound, (await tokens.VerifyCodeAsync(User, Phone, code, default)).Outcome);
    }

    [Fact]
    public async Task A_right_code_within_the_limit_wins()
    {
        var tokens = Tokens();
        var code = await tokens.IssueCodeAsync(User, Phone, "+1", TimeSpan.FromMinutes(10), default);
        for (var attempt = 0; attempt < 4; attempt++)
            await tokens.VerifyCodeAsync(User, Phone, Wrong(code), default);

        Assert.True((await tokens.VerifyCodeAsync(User, Phone, code, default)).Verified);
    }

    [Fact]
    public async Task The_attempt_cap_is_the_apps_policy()
    {
        var tokens = new VerificationTokens(store, new Argon2idPasswordHasher(), new VerificationOptions { MaxCodeAttempts = 2 }, clock);
        var code = await tokens.IssueCodeAsync(User, Phone, "+1", TimeSpan.FromMinutes(10), default);

        Assert.Equal(VerificationOutcome.WrongCode, (await tokens.VerifyCodeAsync(User, Phone, Wrong(code), default)).Outcome);
        Assert.Equal(VerificationOutcome.Locked, (await tokens.VerifyCodeAsync(User, Phone, Wrong(code), default)).Outcome);
    }

    [Fact]
    public async Task An_expired_code_is_refused()
    {
        var tokens = Tokens();
        var code = await tokens.IssueCodeAsync(User, Phone, "+1", TimeSpan.FromMinutes(10), default);
        clock.Advance(TimeSpan.FromMinutes(10));

        Assert.Equal(VerificationOutcome.NotFound, (await tokens.VerifyCodeAsync(User, Phone, code, default)).Outcome);
    }

    [Fact]
    public async Task A_code_belongs_to_its_user()
    {
        var tokens = Tokens();
        var code = await tokens.IssueCodeAsync(User, Phone, "+1", TimeSpan.FromMinutes(10), default);

        Assert.Equal(VerificationOutcome.NotFound, (await tokens.VerifyCodeAsync(Guid.NewGuid(), Phone, code, default)).Outcome);
        Assert.Equal(0, Assert.Single(store.Records).Attempts);
    }

    private static string Wrong(string code) => code == "000000" ? "000001" : "000000";
}
