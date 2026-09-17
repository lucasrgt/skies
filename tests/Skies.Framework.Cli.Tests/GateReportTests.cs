using Skies.Framework.Cli;
using Assay.Net;

namespace Skies.Framework.Cli.Tests;

public class GateReportTests
{
    [Fact]
    public void An_affected_console_report_summarizes_unaffected_inventory_and_keeps_findings_visible()
    {
        var matrix = GateMatrix.Build(
            [new ManifestFile("Wallets.spec.toml", SpecManifest.Parse("module = \"Wallets\"\n[slices.Withdraw]\ncriteria = [\"valid\"]"), null)],
            [new AvpProof("Wallets", "Withdraw", "valid", "W.cs", "WithdrawTests", "Valid")],
            [new SliceSite("Wallets", "Withdraw", "W.cs")], [], new HashSet<string>());
        var output = new StringWriter();

        GateReport.WriteConsole(matrix, new GateLegs(0, 0, Scope: "staged-fast"), output);

        Assert.Contains("1 criteria not affected", output.ToString());
        Assert.DoesNotContain("WithdrawTests", output.ToString());
        Assert.Contains("GREEN", output.ToString());
        var failed = new StringWriter();
        GateReport.WriteConsole(SampleMatrix(passing: false), new GateLegs(0, 1, Scope: "affected"), failed);
        Assert.Contains("Withdraw", failed.ToString());
        Assert.Contains("creep:", failed.ToString());
        Assert.Contains("RED", failed.ToString());
    }

    [Fact]
    public void The_markdown_artifact_carries_the_verdict_the_rows_and_the_findings()
    {
        var matrix = SampleMatrix(passing: false);

        var md = GateReport.Markdown(matrix, new GateLegs(Doctor: 0, Tests: 1), DateTimeOffset.UnixEpoch);

        Assert.Contains("# Verification matrix", md);
        Assert.Contains("**Gate verdict: RED**", md);
        Assert.Contains("| Withdraw | `idempotency-key-honored` |", md);
        Assert.Contains("FAIL", md);
        Assert.Contains("Creep", md);   // the undeclared stray proof surfaces as a finding
    }

    [Fact]
    public void The_json_artifact_is_camel_cased_and_machine_readable()
    {
        var matrix = SampleMatrix(passing: true);

        var json = GateReport.Json(matrix, new GateLegs(0, 0), DateTimeOffset.UnixEpoch);

        Assert.Contains("\"verdict\": \"red\"", json);   // the stray proof keeps the matrix blocking
        Assert.Contains("\"criterion\": \"idempotency-key-honored\"", json);
        Assert.Contains("\"orphanProofs\"", json);
    }

    [Fact]
    public void The_gate_is_green_only_when_every_leg_and_the_matrix_hold()
    {
        var clean = GateMatrix.Build(
            [new ManifestFile("Wallets.spec.toml",
                SpecManifest.Parse("module = \"Wallets\"\n[slices.Withdraw]\ncriteria = [\"idempotency-key-honored\"]"), null)],
            [new AvpProof("Wallets", "Withdraw", "idempotency-key-honored", "W.cs", "WithdrawAvpProof", "Honors_the_key")],
            [new SliceSite("Wallets", "Withdraw", "W.cs")],
            [new TestVerdict("Sample.WithdrawAvpProof", "Honors_the_key", "Passed")]);

        Assert.True(GateReport.Green(clean, new GateLegs(0, 0)));
        Assert.False(GateReport.Green(clean, new GateLegs(0, 1)));   // a red leg is never argued away by the matrix
        Assert.False(GateReport.Green(clean, new GateLegs(0, 0, SkippedTests: 1)));
        Assert.False(GateReport.Green(clean, new GateLegs(0, 0,
            [new FrontendGateLeg("web", FrontendPackageRole.Surface, Tests: 0, Avp: 1, FeatureE2e: 0, E2eShape: 0, E2e: 0)])));
        Assert.True(GateReport.Green(clean, new GateLegs(0, 0,
            [new FrontendGateLeg("core", FrontendPackageRole.Core, Tests: 0, Avp: 0, FeatureE2e: 0, E2eShape: null, E2e: null)])));
        Assert.True(GateReport.Green(clean, new GateLegs(0, 0,
            [new FrontendGateLeg("ui", FrontendPackageRole.Library, Tests: 0, Avp: 0, FeatureE2e: 0, E2eShape: null, E2e: null)])));
        Assert.False(GateReport.Green(clean, new GateLegs(0, 0,
            [new FrontendGateLeg("core", FrontendPackageRole.Core, Tests: 0, Avp: 0, FeatureE2e: 1, E2eShape: null, E2e: null)])));
    }

    [Fact]
    public void Frontend_numbers_are_labeled_as_exit_codes_in_the_human_report()
    {
        var markdown = GateReport.Markdown(
            SampleMatrix(passing: true),
            new GateLegs(0, 0,
                [new FrontendGateLeg("web", FrontendPackageRole.Surface, 0, 0, 0, 0, 0)]),
            DateTimeOffset.UnixEpoch);

        Assert.Contains("exit codes (`0` = pass)", markdown);
        Assert.Contains("| Tests exit | AVP exit | Rendered exit | Feature E2E exit |", markdown);
        Assert.Contains("| web | surface · full |", markdown);
    }

    [Fact]
    public void Reusable_packages_are_reported_as_libraries_without_owing_e2e()
    {
        var markdown = GateReport.Markdown(
            SampleMatrix(passing: true),
            new GateLegs(0, 0,
                [new FrontendGateLeg("ui", FrontendPackageRole.Library, 0, 0, 0, null, null)]),
            DateTimeOffset.UnixEpoch);

        Assert.Contains("| ui | library · full |", markdown);
    }

    [Fact]
    public void A_leg_that_could_not_run_is_reported_as_incomplete_verification_not_as_a_failed_proof()
    {
        var clean = CleanMatrix();
        var blocked = new GateLegs(0, 0,
            [new FrontendGateLeg("web", FrontendPackageRole.Surface, Tests: 0, Avp: 2, FeatureE2e: 0, E2eShape: 0, E2e: 0)]);
        var failed = new GateLegs(0, 0,
            [new FrontendGateLeg("web", FrontendPackageRole.Surface, Tests: 0, Avp: 1, FeatureE2e: 0, E2eShape: 0, E2e: 0)]);

        Assert.Equal("GREEN", GateReport.Verdict(clean, new GateLegs(0, 0)));
        Assert.Equal("RED", GateReport.Verdict(clean, failed));
        Assert.Equal("INCOMPLETE", GateReport.Verdict(clean, blocked));
        Assert.False(GateReport.Green(clean, blocked));          // incomplete still blocks; it just is not a finding
        Assert.False(GateReport.Incomplete(failed));

        var console = new StringWriter();
        GateReport.WriteConsole(clean, blocked, console);
        Assert.Contains("verdict: INCOMPLETE", console.ToString());
        Assert.Contains("**Gate verdict: INCOMPLETE**", GateReport.Markdown(clean, blocked, DateTimeOffset.UnixEpoch));
        Assert.Contains("\"verdict\": \"incomplete\"", GateReport.Json(clean, blocked, DateTimeOffset.UnixEpoch));
    }

    // A matrix whose single declared criterion is proven, so a verdict under test reflects the legs alone.
    private static GateMatrix CleanMatrix() => GateMatrix.Build(
        [new ManifestFile("Wallets.spec.toml",
            SpecManifest.Parse("module = \"Wallets\"\n[slices.Withdraw]\ncriteria = [\"idempotency-key-honored\"]"), null)],
        [new AvpProof("Wallets", "Withdraw", "idempotency-key-honored", "W.cs", "WithdrawAvpProof", "Honors_the_key")],
        [new SliceSite("Wallets", "Withdraw", "W.cs")],
        [new TestVerdict("Sample.WithdrawAvpProof", "Honors_the_key", "Passed")]);

    // A one-module matrix carrying one declared criterion and one stray (undeclared) proof, so both the row
    // table and the findings section have content to assert on.
    private static GateMatrix SampleMatrix(bool passing) => GateMatrix.Build(
        [new ManifestFile("Wallets.spec.toml",
            SpecManifest.Parse("module = \"Wallets\"\n[slices.Withdraw]\ncriteria = [\"idempotency-key-honored\"]"), null)],
        [
            new AvpProof("Wallets", "Withdraw", "idempotency-key-honored", "W.cs", "WithdrawAvpProof", "Honors_the_key"),
            new AvpProof("Wallets", "Withdraw", "stray-criterion", "S.cs", "StrayProof", "Proves_the_undeclared"),
        ],
        [new SliceSite("Wallets", "Withdraw", "W.cs")],
        [new TestVerdict("Sample.WithdrawAvpProof", "Honors_the_key", passing ? "Passed" : "Failed")]);
}
