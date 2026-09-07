using System.Text.Json;
using Skies.Framework.Cli;

namespace Skies.Framework.Cli.Tests;

public sealed class GateAttemptTests : IDisposable
{
    private readonly string root = Path.Combine(Path.GetTempPath(), "skies-attempt-test-" + Guid.NewGuid().ToString("N"));

    [Fact]
    public void A_failed_attempt_blocks_repetition_and_scope_changes_without_review()
    {
        using (var first = GateAttempt.Begin(root, "affected", null, TextWriter.Null))
        {
            Assert.NotNull(first);
            first.Complete(1);
        }
        var error = new StringWriter();
        Assert.Null(GateAttempt.Begin(root, "full", null, error));
        Assert.Contains("loop", error.ToString());
        Assert.Contains("One automatic attempt", error.ToString());
    }

    [Fact]
    public void An_active_attempt_blocks_parallel_work_and_an_interrupted_attempt_requires_review()
    {
        using (var first = GateAttempt.Begin(root, "affected", null, TextWriter.Null))
        {
            Assert.NotNull(first);
            Assert.Null(GateAttempt.Begin(root, "affected", null, TextWriter.Null));
        }
        Assert.Null(GateAttempt.Begin(root, "affected", null, TextWriter.Null));
    }

    [Fact]
    public void A_review_allows_exactly_one_attempt_and_cannot_be_replayed_after_another_failure()
    {
        using (var first = GateAttempt.Begin(root, "affected", null, TextWriter.Null))
            first!.Complete(1);
        var previous = JsonSerializer.Deserialize<GateAttempt.Receipt>(File.ReadAllText(Path.Combine(root, "attempt.json")))!;
        var review = Path.Combine(root, "review.json");
        File.WriteAllText(review, JsonSerializer.Serialize(new GateAttempt.Review(previous.Id,
            "The fixture retained expired credentials", "Renew the fixture session", "Session renewal regression passed")));
        using (var next = GateAttempt.Begin(root, "affected", review, TextWriter.Null))
        {
            Assert.NotNull(next);
            next.Complete(1);
        }
        Assert.Null(GateAttempt.Begin(root, "affected", review, TextWriter.Null));
    }

    [Fact]
    public void A_successful_attempt_allows_future_verification()
    {
        using (var first = GateAttempt.Begin(root, "affected", null, TextWriter.Null))
            first!.Complete(0);
        using var next = GateAttempt.Begin(root, "affected", null, TextWriter.Null);
        Assert.NotNull(next);
    }

    [Theory]
    [InlineData("not json")]
    [InlineData("null")]
    [InlineData("{}")]
    public void Corrupt_state_fails_closed(string state)
    {
        Directory.CreateDirectory(root);
        File.WriteAllText(Path.Combine(root, "attempt.json"), state);
        Assert.Null(GateAttempt.Begin(root, "affected", null, TextWriter.Null));
    }

    public void Dispose()
    {
        if (Directory.Exists(root))
            Directory.Delete(root, true);
    }
}
