using System.Text.Json;
using Skies.Framework.Testing;

namespace Skies.Framework.AspNetCore.Tests;

// How SpecEvidence can fail: write outside a proof run (polluting the working directory), lose enum names (a
// verdict reading 0 instead of Pass in review), escape the evidence folder through the name, or write nothing the
// engine can read back. Every test here owns SKIES_EVIDENCE for its duration; xUnit runs one class's tests serially.
public sealed class SpecEvidenceTests : IDisposable
{
    private readonly string? _previous = Environment.GetEnvironmentVariable(SpecEvidence.Variable);
    private readonly string _folder = Path.Combine(Path.GetTempPath(), $"skies-evidence-{Guid.NewGuid():N}");

    public void Dispose()
    {
        Environment.SetEnvironmentVariable(SpecEvidence.Variable, _previous);
        if (Directory.Exists(_folder))
            Directory.Delete(_folder, recursive: true);
    }

    private enum Status { Pass, Fail }

    private sealed record Result(string CriterionId, Status Status);

    private sealed record Verdict(string Subject, IReadOnlyList<Result> Results);

    [Fact]
    public void Outside_a_proof_run_nothing_is_written()
    {
        Environment.SetEnvironmentVariable(SpecEvidence.Variable, null);

        Assert.Null(SpecEvidence.Directory);
        Assert.Null(SpecEvidence.Save("avp-FM-1.json", new { ok = true }));
    }

    [Fact]
    public void A_verdict_is_saved_as_indented_camel_case_json_with_enum_names()
    {
        Environment.SetEnvironmentVariable(SpecEvidence.Variable, _folder);
        var verdict = new Verdict("Deposit", [new Result("idempotency-key-honored", Status.Pass)]);

        var path = SpecEvidence.Save("avp-FM-5.json", verdict);

        Assert.Equal(Path.Combine(_folder, "avp-FM-5.json"), path);
        var text = File.ReadAllText(path!);
        Assert.Contains("\n", text.TrimEnd());
        using var json = JsonDocument.Parse(text);
        var result = json.RootElement.GetProperty("results")[0];
        Assert.Equal("idempotency-key-honored", result.GetProperty("criterionId").GetString());
        Assert.Equal("Pass", result.GetProperty("status").GetString());
    }

    [Fact]
    public void Subfolders_are_created_inside_the_evidence_folder()
    {
        Environment.SetEnvironmentVariable(SpecEvidence.Variable, _folder);

        var path = SpecEvidence.Save(Path.Combine("http", "deposit.json"), new { status = 200 });

        Assert.True(File.Exists(path));
        Assert.Equal(_folder, SpecEvidence.Directory);
    }

    [Theory]
    [InlineData("../escape.json")]
    [InlineData("a/../../escape.json")]
    public void A_name_that_climbs_out_of_the_folder_is_refused(string name)
    {
        Environment.SetEnvironmentVariable(SpecEvidence.Variable, _folder);

        Assert.Throws<ArgumentException>(() => SpecEvidence.Save(name, new { }));
        Assert.False(File.Exists(Path.Combine(Path.GetTempPath(), "escape.json")));
    }
}
