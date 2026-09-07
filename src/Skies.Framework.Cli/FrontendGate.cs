namespace Skies.Framework.Cli;

/// <summary>The executable verification legs for one frontend package.</summary>
/// <param name="Client">The client directory name.</param>
/// <param name="Role">The manifest role that decides whether E2E applies.</param>
/// <param name="Tests">The unit/integration test script exit code.</param>
/// <param name="Avp">The Assay/AVP verification script exit code.</param>
/// <param name="FeatureE2e">The workspace feature-to-executable-flow coverage exit code.</param>
/// <param name="E2eShape">The E2E contract exit code, or null for a non-executable package.</param>
/// <param name="E2e">The real E2E runner exit code, or null for a non-executable package.</param>
/// <param name="RenderedDesign">The rendered design/Storybook proof exit code.</param>
internal sealed record FrontendGateLeg(
    string Client,
    FrontendPackageRole Role,
    int? Tests,
    int? Avp,
    int FeatureE2e,
    int? E2eShape,
    int? E2e,
    string Scope = "full",
    int? RenderedDesign = null)
{
    /// <summary>Whether every selected frontend verification leg ran successfully.</summary>
    public bool Green => Tests is null or 0 && Avp is null or 0 && RenderedDesign is null or 0 && FeatureE2e == 0
        && (Role != FrontendPackageRole.Surface || E2eShape == 0)
        && E2e is null or 0;
}

/// <summary>Runs every frontend proof suite. Missing scripts/tools fail naturally; nothing is optional.</summary>
internal static class FrontendGate
{
    private const string ReactAssaySuiteGlob = "**/*.assay.test.*";

    /// <summary>
    /// Run the globally structural frontend checks and the runtime subset selected by <paramref name="impacts"/>.
    /// Null means the explicit full audit.
    /// </summary>
    public static IReadOnlyList<FrontendGateLeg> Run(
        string workspace,
        IEnumerable<FrontendPackage> clients,
        IReadOnlyList<FrontendImpact>? impacts = null,
        bool fast = false)
    {
        var targets = clients.ToList();
        var selected = impacts ?? targets.Select(target => new FrontendImpact(target) { Full = true }).ToList();
        var featureCoverage = FeatureCoverage(workspace, targets);
        var legs = new List<FrontendGateLeg>();
        foreach (var target in targets)
        {
            var client = target.Path;
            var name = Path.GetFileName(client);
            var impact = selected.FirstOrDefault(item => SamePackage(item.Package, target))
                ?? new FrontendImpact(target);
            int? tests = null;
            int? avp = null;
            if (impact.Full || impact.Tests.Count > 0)
            {
                if (target.Platform == FrontendPlatform.Flutter)
                {
                    tests = RunFlutterTests(client, impact);
                }
                else
                {
                    Console.WriteLine($"skies gate — frontend tests ({name}, "
                        + (impact.Full ? "full non-Assay partition" : $"{impact.Tests.Count} affected file(s)") + ")...");
                    var filters = impact.Full ? [] : impact.Tests.Order().ToArray();
                    var script = FrontendScriptContract.ResolveUnitTestScript(client);
                    if (FrontendScriptContract.UsesNativeNodeTests(client, script))
                    {
                        var helper = Path.Combine(AppContext.BaseDirectory, "Tools", "node-test-affected.mjs");
                        var paths = impact.Full ? "--full" : System.Text.Json.JsonSerializer.Serialize(filters);
                        tests = Tooling.Run("node", [helper, script, paths], client);
                    }
                    else
                    {
                        var arguments = new List<string> { "--" };
                        arguments.AddRange(filters);
                        arguments.Add($"--exclude={ReactAssaySuiteGlob}");
                        arguments.Add("--maxWorkers=2");
                        tests = FrontendScriptContract.Run(client, script, [.. arguments]);
                    }
                }
            }

            if (impact.Full || impact.Assays.Count > 0)
                avp = RunAssay(target, name, impact.Full ? null : impact.Assays.Order().ToArray());

            int? renderedDesign = null;
            if (!fast && (impact.RenderedDesign
                          || impact.Full && FrontendScriptContract.HasScript(client, "design:rendered")))
            {
                Console.WriteLine($"skies gate — rendered design ({name}, "
                    + (impact.Full ? "full" : "affected Storybook surface") + ")...");
                renderedDesign = FrontendScriptContract.Run(client, "design:rendered");
            }

            int? e2eShape = null;
            int? e2e = null;
            if (target.Role == FrontendPackageRole.Surface)
            {
                Console.WriteLine($"skies gate — frontend E2E contract ({name})...");
                var doctor = target.Platform == FrontendPlatform.Flutter
                    ? "skies-flutter-e2e-doctor"
                    : "skyfe-e2e-doctor";
                var manifestShape = Tooling.Run("npx", ["--no-install", doctor, "."], client);
                e2eShape = manifestShape;

                if (!fast && (impact.Full || impact.Flows.Count > 0))
                {
                    var flows = impact.Full ? GateImpact.ReadFrontendFlows(client) : impact.Flows;
                    if (flows is null)
                    {
                        e2e = 1;
                    }
                    else
                    {
                        Console.WriteLine($"skies gate — frontend E2E execution ({name}, "
                            + (impact.Full ? "full" : $"{flows.Count} affected flow(s)") + ")...");
                        e2e = RunE2e(target, flows, impact.Full);
                    }
                }
            }

            var scope = impact.Full ? "full" : impact.Selected ? (fast ? "affected-fast" : "affected") : "not-affected";
            legs.Add(new FrontendGateLeg(
                name, target.Role, tests, avp, featureCoverage, e2eShape, e2e, scope, renderedDesign));
        }

        return legs;
    }

    /// <summary>Run Assay whenever the package owns a ViewModel or an explicit Assay suite.</summary>
    internal static int RunAssay(FrontendPackage package, string name, IReadOnlyList<string>? paths = null)
    {
        var client = package.Path;
        if (!RequiresAssay(client))
        {
            Console.WriteLine($"skies gate — frontend AVP ({name}): not applicable (no ViewModel or Assay suite).");
            return 0;
        }

        Console.WriteLine($"skies gate — frontend AVP ({name})...");
        if (package.Platform == FrontendPlatform.Flutter)
        {
            var testRoot = Path.Combine(client, "test");
            var assays = paths ?? (Directory.Exists(testRoot)
                ? Directory.EnumerateFiles(testRoot, "*.assay_test.dart", SearchOption.AllDirectories)
                    .Select(path => Path.GetRelativePath(client, path))
                    .Order(StringComparer.OrdinalIgnoreCase)
                    .ToArray()
                : []);
            if (assays.Count == 0)
            {
                Console.Error.WriteLine($"skies gate — frontend AVP ({name}): a Flutter ViewModel has no executable *.assay_test.dart proof.");
                return 1;
            }
            return Tooling.Run("flutter", ["test", .. assays], client);
        }
        // Invoke Assay directly: a package script is allowed to compose work, but must not be able to replace
        // the acceptance verifier with a placeholder that exits zero.
        var arguments = new List<string> { "--no-install", "assay", "verify" };
        if (paths is not null)
            arguments.AddRange(paths);
        arguments.AddRange(["--", "--maxWorkers=2"]);
        return Tooling.Run("npx", [.. arguments], client);
    }

    /// <summary>Decide whether the universal ViewModel-to-Assay obligation applies to this package.</summary>
    internal static bool RequiresAssay(string client)
    {
        var flutterSource = Path.Combine(client, "lib");
        var flutterTests = Path.Combine(client, "test");
        if (File.Exists(Path.Combine(client, "pubspec.yaml")))
        {
            return Directory.Exists(flutterSource)
                   && Directory.EnumerateFiles(flutterSource, "*_view_model.dart", SearchOption.AllDirectories).Any()
                   || Directory.Exists(flutterTests)
                   && Directory.EnumerateFiles(flutterTests, "*.assay_test.dart", SearchOption.AllDirectories).Any();
        }
        var source = Path.Combine(client, "src");
        if (!Directory.Exists(source))
            return false;

        return Directory.EnumerateFiles(source, "*", SearchOption.AllDirectories)
            .Select(Path.GetFileName)
            .Any(file => file is not null &&
                (file.EndsWith(".viewModel.ts", StringComparison.OrdinalIgnoreCase)
                 || file.EndsWith(".viewModel.tsx", StringComparison.OrdinalIgnoreCase)
                 || file.Contains(".assay.test.", StringComparison.OrdinalIgnoreCase)));
    }

    private static int FeatureCoverage(string workspace, IReadOnlyList<FrontendPackage> targets)
    {
        if (targets.Count == 0)
            return 0;

        Console.WriteLine("skies gate — frontend feature → E2E coverage...");
        var reactTargets = targets.Where(target => target.Platform == FrontendPlatform.React).ToList();
        var code = 0;
        if (reactTargets.Count > 0)
        {
            var arguments = new List<string> { "--no-install", "skyfe-feature-e2e", workspace };
            arguments.AddRange(reactTargets.Select(target =>
            $"{(target.Role == FrontendPackageRole.Surface ? "surface" : "core")}={target.Path}"));
            var toolRoot = reactTargets.FirstOrDefault(target => target.Role == FrontendPackageRole.Surface)?.Path
                ?? reactTargets[0].Path;
            code = Math.Max(code, Tooling.Run("npx", [.. arguments], toolRoot));
        }
        foreach (var target in targets.Where(target => target.Platform == FrontendPlatform.Flutter))
            code = Math.Max(code, Tooling.Run(
                "npx",
                ["--no-install", "skies-flutter-feature-e2e", "."],
                target.Path));
        return code;
    }

    private static int RunE2e(FrontendPackage package, IReadOnlyList<FrontendFlow> flows, bool full)
    {
        var client = package.Path;
        if (package.Platform == FrontendPlatform.Flutter)
        {
            if (flows.Any(flow => flow.Target != "native"))
                return 1;
            var specs = flows.Select(flow => flow.Spec).Where(spec => spec.Length > 0)
                .Distinct(StringComparer.OrdinalIgnoreCase).Order().ToArray();
            return FlutterIntegrationSuite.Run(client, specs);
        }
        var code = 0;
        // A release gate must never attach to an arbitrary dev server that merely answers the same health URL.
        // Playwright's standard `reuseExistingServer: !process.env.CI` convention therefore receives CI=true
        // for the execution leg even when `skies gate` runs locally. SKY_GATE additionally gives custom configs an
        // explicit, tool-owned signal. Manual `playwright test` remains free to reuse servers during development.
        IReadOnlyDictionary<string, string?> gateEnvironment = new Dictionary<string, string?>
        {
            ["CI"] = "true",
            ["SKY_GATE"] = "1",
        };
        var web = flows.Where(flow => flow.Target == "web").Select(flow => flow.Spec)
            .Where(spec => spec.Length > 0).Distinct(StringComparer.OrdinalIgnoreCase).Order().ToList();
        if (web.Count > 0)
        {
            if (string.Equals(Environment.GetEnvironmentVariable("CI"), "true", StringComparison.OrdinalIgnoreCase))
            {
                Console.WriteLine("skies gate — installing the project-pinned Playwright browsers for the selected web closure...");
                var install = Tooling.Run("npx", ["--no-install", "playwright", "install", "--with-deps"], client);
                if (install != 0)
                    return install;
            }
            if (full)
                code = Math.Max(code, Tooling.Run(
                    "npx", ["--no-install", "playwright", "test", .. web], client, gateEnvironment));
            else
                code = Math.Max(code, RunAffectedWeb(client, flows.Where(flow => flow.Target == "web").ToList(), gateEnvironment));
        }

        var native = flows.Where(flow => flow.Target == "native").Select(flow => flow.Spec)
            .Where(spec => spec.Length > 0).Distinct(StringComparer.OrdinalIgnoreCase).Order().ToList();
        if (native.Count > 0)
            code = Math.Max(code, Tooling.Run("maestro", ["test", .. native], client, gateEnvironment));
        return code;
    }

    private static int RunAffectedWeb(string client, IReadOnlyList<FrontendFlow> flows, IReadOnlyDictionary<string, string?> environment)
    {
        var selection = Path.GetTempFileName();
        try
        {
            File.WriteAllText(selection, System.Text.Json.JsonSerializer.Serialize(flows,
                new System.Text.Json.JsonSerializerOptions { PropertyNamingPolicy = System.Text.Json.JsonNamingPolicy.CamelCase }));
            var script = Path.Combine(AppContext.BaseDirectory, "Tools", "playwright-affected.mjs");
            return Tooling.Run("node", [script, selection], client, environment);
        }
        finally
        {
            File.Delete(selection);
        }
    }

    internal static int RunFlutterTests(string client, FrontendImpact impact)
    {
        var testRoot = Path.Combine(client, "test");
        var tests = impact.Full && Directory.Exists(testRoot)
            ? Directory.EnumerateFiles(testRoot, "*_test.dart", SearchOption.AllDirectories)
                .Where(path => !path.EndsWith(".assay_test.dart", StringComparison.OrdinalIgnoreCase))
                .Select(path => Path.GetRelativePath(client, path)).Order(StringComparer.OrdinalIgnoreCase).ToArray()
            : impact.Full ? [] : impact.Tests.Order().ToArray();
        if (tests.Length == 0)
        {
            if (!impact.Full || RequiresAssay(client))
                return 0; // The Assay leg owns the package's remaining executable partition.
            Console.Error.WriteLine($"skies gate — Flutter tests ({Path.GetFileName(client)}): no executable test suite.");
            return 1;
        }
        Console.WriteLine($"skies gate — Flutter tests ({Path.GetFileName(client)}, "
            + (impact.Full ? "full non-Assay partition" : $"{tests.Length} affected file(s)") + ")...");
        return Tooling.Run("flutter", ["test", .. tests], client);
    }

    private static bool SamePackage(FrontendPackage left, FrontendPackage right) =>
        string.Equals(Path.GetFullPath(left.Path), Path.GetFullPath(right.Path), StringComparison.OrdinalIgnoreCase);
}
