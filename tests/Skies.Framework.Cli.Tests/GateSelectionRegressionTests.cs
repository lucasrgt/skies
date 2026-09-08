using Skies.Framework.Cli;

namespace Skies.Framework.Cli.Tests;

public sealed class GateSelectionRegressionTests : IDisposable
{
    private readonly string _root = Directory.CreateTempSubdirectory("skies-gate-selection-").FullName;

    public GateSelectionRegressionTests()
    {
        Write("src/App/Placeholder.cs", "class Placeholder {}");
        Write("clients/web/package.json", "{}");
        Write("Skies.toml", "[workspace]\nname='test'\n[products.app]\nbackend='src/App'\nfrontend='clients/web'\n");
    }

    [Fact]
    public void A_module_manifest_selects_that_modules_proofs_without_widening_other_modules()
    {
        Write("src/App/Modules/Wallets/Wallets.spec.toml", "module='Wallets'\n[slices.Withdraw]\ncriteria=['valid']\n");
        var plan = GateImpact.Build(_root, ["src/App/Modules/Wallets/Wallets.spec.toml"],
            [new("Wallets", "Withdraw", "src/App/Modules/Wallets/Withdraw.cs"),
             new("Account", "Login", "src/App/Modules/Account/Login.cs")],
            [new("Wallets", "Withdraw", "valid", "Withdraw.Tests.cs", "WithdrawTests", "Valid"),
             new("Account", "Login", "valid", "Login.Tests.cs", "LoginTests", "Valid")], [], [], []);

        Assert.False(plan.Backend.Full);
        Assert.Equal(["Wallets/Withdraw"], plan.Backend.AffectedSlices);
        Assert.Contains("WithdrawTests", plan.Backend.Filters);
        Assert.DoesNotContain("LoginTests", plan.Backend.Filters);
    }

    [Fact]
    public void A_transitive_proof_selects_every_proof_of_its_subject_without_widening_other_subjects()
    {
        Write("src/App/Money.cs", "namespace Shared; internal class Money {}");
        Write("src/App/Modules/Reports/Mixed.Tests.cs", "namespace Reports; class MixedTests { Shared.Money value; }");
        Write("src/App/Modules/Reports/Export.cs", "namespace Reports; class Export {}");
        Write("src/App/Modules/Reports/Export.Tests.cs", "namespace Reports; class ExportTests {}");
        var plan = GateImpact.Build(_root, ["src/App/Money.cs"],
            [new("Reports", "Export", "src/App/Modules/Reports/Export.cs"),
             new("Account", "Login", "src/App/Modules/Account/Login.cs")],
            [new("Reports", "Export", "header", "src/App/Modules/Reports/Mixed.Tests.cs", "MixedTests", "Header"),
             new("Reports", "Export", "totals", "src/App/Modules/Reports/Export.Tests.cs", "ExportTests", "Totals"),
             new("Account", "Login", "valid", "src/App/Modules/Account/Login.Tests.cs", "LoginTests", "Valid")],
            [new("Export", "src/App/Modules/Reports/ExportJourney.Tests.cs", "ExportJourneyTests", "Downloads")], [], []);

        Assert.False(plan.Backend.Full);
        Assert.Equal(["Reports/Export"], plan.Backend.AffectedSlices);
        Assert.Contains("MixedTests", plan.Backend.Filters);
        Assert.Contains("ExportTests", plan.Backend.Filters);
        Assert.Contains("ExportJourneyTests", plan.Backend.Filters);
        Assert.DoesNotContain("LoginTests", plan.Backend.Filters);
        Assert.Empty(plan.Backend.RuntimeSlices);
    }

    [Fact]
    public void Feature_copy_selects_descendant_feature_proofs_and_global_copy_validation()
    {
        Write("clients/web/src/features/events/events.i18n.ts", "export default { title: 'Events' };");
        Write("clients/web/src/features/events/detail/EventDetail.viewModel.ts", "export {};");
        Write("clients/web/src/features/events/detail/EventDetail.assay.test.ts", "export {};");
        Write("clients/web/src/features/events/list/EventList.viewModel.ts", "export {};");
        Write("clients/web/src/features/events/list/EventList.assay.test.ts", "export {};");
        Write("clients/web/src/features/login/Login.assay.test.ts", "export {};");
        Write("clients/web/src/i18n/keysResolve.test.ts", "export {};");

        var plan = GateImpact.Build(_root, ["clients/web/src/features/events/events.i18n.ts"],
            [], [], [], [], [new(Path.Combine(_root, "clients/web"), FrontendPackageRole.Surface)]);

        var frontend = Assert.Single(plan.Frontends);
        Assert.False(frontend.ExhaustiveFallback);
        Assert.Equal(2, frontend.Assays.Count);
        Assert.Contains("src/i18n/keysResolve.test.ts", frontend.Tests);
        Assert.DoesNotContain("src/features/login/Login.assay.test.ts", frontend.Assays);
    }

    [Fact]
    public void Sharing_a_flow_does_not_make_its_other_features_new_change_roots()
    {
        Write("clients/web/src/features/checkout/Checkout.viewModel.ts", "export {};");
        Write("clients/web/src/features/checkout/Checkout.assay.test.ts", "export {};");
        Write("clients/web/src/features/profile/Profile.viewModel.ts", "export {};");
        Write("clients/web/src/features/profile/Profile.assay.test.ts", "export {};");
        Write("clients/web/e2e/flows.json", """
            [
              {"id":"checkout","target":"web","spec":"e2e/checkout.spec.ts","features":["Checkout", "Login"],"backendSlices":[]},
              {"id":"profile","target":"web","spec":"e2e/profile.spec.ts","features":["Profile", "Login"],"backendSlices":[]}
            ]
            """);

        var plan = GateImpact.Build(_root, ["clients/web/src/features/checkout/Checkout.viewModel.ts"],
            [], [], [], [], [new(Path.Combine(_root, "clients/web"), FrontendPackageRole.Surface)]);

        var frontend = Assert.Single(plan.Frontends);
        Assert.Equal("checkout", Assert.Single(frontend.Flows).Id);
        Assert.DoesNotContain("src/features/profile/Profile.assay.test.ts", frontend.Assays);
    }

    private void Write(string path, string contents)
    {
        var fullPath = Path.Combine(_root, path);
        Directory.CreateDirectory(Path.GetDirectoryName(fullPath)!);
        File.WriteAllText(fullPath, contents);
    }

    public void Dispose() => Directory.Delete(_root, recursive: true);
}
