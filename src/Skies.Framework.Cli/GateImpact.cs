using System.Text.Json;

namespace Skies.Framework.Cli;

/// <summary>The backend proof scope selected from one Git delta.</summary>
/// <param name="Full">Whether every backend test must execute.</param>
/// <param name="Filters">Class/subject fragments used by the framework-owned <c>dotnet test</c> filter.</param>
/// <param name="AffectedSlices">Module/subject keys whose AVP rows must receive runtime verdicts.</param>
internal sealed record BackendImpact(
    bool Full,
    IReadOnlySet<string> Filters,
    IReadOnlySet<string> AffectedSlices,
    IReadOnlySet<string> DirectFilters,
    IReadOnlySet<string> DirectAffectedSlices)
{
    internal BackendImpact(
        bool full,
        IReadOnlySet<string> filters,
        IReadOnlySet<string> affectedSlices)
        : this(full, filters, affectedSlices, filters, affectedSlices)
    {
    }

    /// <summary>Whether this change requires any backend test process.</summary>
    public bool RunsTests => Full || Filters.Count > 0;

    /// <summary>Production slices whose behavior can affect a browser flow; test-only consumers are not new roots.</summary>
    public IReadOnlySet<string> RuntimeSlices { get; init; } = AffectedSlices;
}

/// <summary>The runtime proof subset for one frontend package.</summary>
internal sealed class FrontendImpact(FrontendPackage package)
{
    /// <summary>The package being selected.</summary>
    public FrontendPackage Package { get; } = package;

    /// <summary>Whether every runtime proof in this package must run.</summary>
    public bool Full { get; set; }

    /// <summary>Whether ambiguous impact requires an exhaustive fallback at an authoritative boundary.</summary>
    public bool ExhaustiveFallback { get; set; }

    /// <summary>Vitest files selected outside the Assay partition.</summary>
    public HashSet<string> Tests { get; } = new(StringComparer.OrdinalIgnoreCase);

    /// <summary>Assay files selected for affected ViewModels.</summary>
    public HashSet<string> Assays { get; } = new(StringComparer.OrdinalIgnoreCase);

    /// <summary>E2E flows selected by a changed ViewModel, spec, or backend slice.</summary>
    public List<FrontendFlow> Flows { get; } = [];

    /// <summary>Whether the package's rendered design proof was selected by Storybook source.</summary>
    public bool RenderedDesign { get; set; }

    /// <summary>Whether any runtime proof was selected.</summary>
    public bool Selected => Full || ExhaustiveFallback || Tests.Count > 0 || Assays.Count > 0 || Flows.Count > 0
        || RenderedDesign;
}

/// <summary>A normalized executable frontend flow from <c>e2e/flows.json</c>.</summary>
/// <param name="Id">Stable flow identifier.</param>
/// <param name="Target">Canonical execution target (<c>web</c> or <c>native</c>).</param>
/// <param name="Spec">Package-relative Playwright, Maestro, or Flutter integration-test spec.</param>
/// <param name="Features">ViewModel subjects proven by the flow.</param>
/// <param name="BackendSlices">Backend subjects observed by the flow.</param>
internal sealed record FrontendFlow(
    string Id,
    string Target,
    string Spec,
    IReadOnlyList<string> Features,
    IReadOnlyList<string> BackendSlices,
    string Case = "");

/// <summary>The complete, explainable execution closure for one gate run.</summary>
/// <param name="Backend">Backend runtime selection.</param>
/// <param name="Frontends">Frontend runtime selection, one entry per declared package.</param>
/// <param name="Reasons">Human-readable selection and fallback decisions.</param>
internal sealed record GateImpactPlan(
    BackendImpact Backend,
    IReadOnlyList<FrontendImpact> Frontends,
    IReadOnlyList<string> Reasons);

/// <summary>
/// Expands changed files into the backend AVP/Journey and frontend Assay/E2E closure. Selection is convention-
/// derived; ambiguous production or runtime-wide infrastructure changes widen instead of trusting a skip.
/// Control-plane changes are validated by the doctor without impersonating application impact.
/// </summary>
internal static partial class GateImpact
{
    /// <summary>Build a fail-closed impact plan from the current workspace inventory.</summary>
    public static GateImpactPlan Build(
        string root,
        IReadOnlyList<string> changes,
        IReadOnlyList<SliceSite> slices,
        IReadOnlyList<AvpProof> proofs,
        IReadOnlyList<JourneyProof> journeys,
        IReadOnlyList<CSharpTestSite> testClasses,
        IReadOnlyList<FrontendPackage> packages,
        GitComparison? comparison = null)
    {
        var normalized = changes.Select(Normalize).Distinct(StringComparer.OrdinalIgnoreCase).ToList();
        var reasons = new List<string>();
        var gateControl = normalized.Count(IsGateControlInfrastructure);
        if (gateControl > 0)
        {
            reasons.Add(
                $"gate: {gateControl} control-only path(s) changed; the doctor validates them without widening application proofs");
        }

        var global = normalized.Any(IsRuntimeWideInfrastructure);
        var backend = SelectBackend(root, normalized, slices, proofs, journeys, testClasses, global, reasons);
        var frontend = packages.Select(package => new FrontendImpact(package)).ToList();
        SelectFrontends(root, normalized, frontend, backend, global, reasons, comparison);
        return new GateImpactPlan(backend, frontend, reasons);
    }

    private static void SelectFrontends(
        string root,
        IReadOnlyList<string> changes,
        IReadOnlyList<FrontendImpact> impacts,
        BackendImpact backend,
        bool global,
        List<string> reasons,
        GitComparison? comparison)
    {
        if (global || changes.Any(IsRootFrontendContract))
        {
            foreach (var impact in impacts)
                impact.ExhaustiveFallback = true;
            if (impacts.Count > 0)
                reasons.Add("frontend: shared workspace/gate dependency changed; selecting every frontend proof");
        }

        var packageFeatures = impacts.ToDictionary(impact => impact,
            _ => new HashSet<string>(StringComparer.Ordinal));
        var runtimeChanges = new List<string>(changes);
        foreach (var impact in impacts)
        {
            var packageRelative = Normalize(Path.GetRelativePath(root, impact.Package.Path));
            var packageChanges = changes.Where(change => IsWithin(change, packageRelative)).ToList();
            if (packageChanges.Any(GeneratedClientImpact.IsGenerated))
            {
                var consumers = GeneratedClientImpact.Select(root, impact.Package, packageChanges, comparison, reasons,
                    impacts.Select(item => item.Package).ToList());
                if (consumers is null)
                    impact.ExhaustiveFallback = true;
                else
                    runtimeChanges.AddRange(consumers.Select(file => Normalize(Path.GetRelativePath(root,
                        Path.GetFullPath(Path.Combine(impact.Package.Path, file))))));
            }
        }
        foreach (var impact in impacts)
        {
            var packageRelative = Normalize(Path.GetRelativePath(root, impact.Package.Path));
            foreach (var change in runtimeChanges.Where(change => IsWithin(change, packageRelative))
                         .Distinct(StringComparer.OrdinalIgnoreCase))
                SelectPackageChange(root, impact, packageRelative, change, packageFeatures[impact], reasons);
        }

        var widenedLibraries = impacts.Where(impact => impact.Package.Role != FrontendPackageRole.Surface
                                  && impact.ExhaustiveFallback).ToList();
        if (widenedLibraries.Count > 0)
        {
            foreach (var impact in FrontendConsumers.Expand(impacts, widenedLibraries, reasons))
                impact.ExhaustiveFallback = true;
        }

        var backendSubjects = backend.RuntimeSlices
            .Select(key => key[(key.IndexOf('/') + 1)..])
            .ToHashSet(StringComparer.Ordinal);
        if (backend.Full)
            backendSubjects.Clear();

        var sharedFeatures = impacts.Where(impact => impact.Package.Role != FrontendPackageRole.Surface)
            .SelectMany(impact => packageFeatures[impact]).ToHashSet(StringComparer.Ordinal);
        foreach (var impact in impacts)
        {
            var changedFeatures = packageFeatures[impact].Concat(sharedFeatures).ToHashSet(StringComparer.Ordinal);
            var flows = ReadFrontendFlows(impact.Package.Path);
            if (flows is null)
            {
                impact.ExhaustiveFallback = true;
                continue;
            }

            foreach (var flow in flows.Where(flow =>
                         backend.Full
                         || flow.Features.Any(changedFeatures.Contains)
                         || flow.BackendSlices.Any(backendSubjects.Contains)))
            {
                AddFlow(impact, flow);
                foreach (var feature in flow.Features)
                    packageFeatures[impact].Add(feature);
            }
        }

        foreach (var impact in impacts)
            foreach (var feature in packageFeatures[impact])
                SelectFeatureProofs(impact, feature);

        foreach (var impact in impacts.Where(impact => impact.Selected))
        {
            reasons.Add(impact.ExhaustiveFallback
                ? $"frontend: {Path.GetFileName(impact.Package.Path)} widened to its full proof surface"
                : $"frontend: {Path.GetFileName(impact.Package.Path)} selected tests={impact.Tests.Count}, "
                  + $"assays={impact.Assays.Count}, rendered={(impact.RenderedDesign ? 1 : 0)}, "
                  + $"flows={impact.Flows.Count}");
        }
    }

    /// <summary>Read the flow inventory used by both impact selection and canonical runner invocation.</summary>
    internal static IReadOnlyList<FrontendFlow>? ReadFrontendFlows(string package)
    {
        var path = Path.Combine(package, "e2e", "flows.json");
        if (!File.Exists(path))
            return [];
        try
        {
            using var document = JsonDocument.Parse(File.ReadAllText(path));
            if (document.RootElement.ValueKind != JsonValueKind.Array)
                return null;
            return document.RootElement.EnumerateArray().Select(flow => new FrontendFlow(
                Text(flow, "id"),
                Text(flow, "target"),
                Normalize(Text(flow, "spec")),
                Strings(flow, "features"),
                Strings(flow, "backendSlices"),
                Text(flow, "case"))).ToList();
        }
        catch (JsonException)
        {
            return null;
        }
    }

    private static string Text(JsonElement element, string property) =>
        element.TryGetProperty(property, out var value) && value.ValueKind == JsonValueKind.String
            ? value.GetString() ?? ""
            : "";

    private static IReadOnlyList<string> Strings(JsonElement element, string property) =>
        element.TryGetProperty(property, out var value) && value.ValueKind == JsonValueKind.Array
            ? value.EnumerateArray().Where(item => item.ValueKind == JsonValueKind.String)
                .Select(item => item.GetString()!).ToList()
            : [];

    private static void AddFlow(FrontendImpact impact, FrontendFlow flow)
    {
        if (!impact.Flows.Any(existing => existing.Id == flow.Id))
            impact.Flows.Add(flow);
    }

    private static void SelectSlice(
        SliceSite slice,
        HashSet<string> filters,
        HashSet<string> affected,
        IReadOnlyList<AvpProof> proofs,
        IReadOnlyList<JourneyProof> journeys)
    {
        filters.Add(slice.Name);
        affected.Add(slice.Module + "/" + slice.Name);
        foreach (var proof in proofs.Where(proof => proof.Module == slice.Module && proof.Subject == slice.Name))
            filters.Add(proof.ClassName);
        foreach (var journey in journeys.Where(journey => journey.Subject == slice.Name))
            filters.Add(journey.ClassName);
    }

    private static BackendImpact FullBackend() =>
        new(true, new HashSet<string>(StringComparer.Ordinal), new HashSet<string>(StringComparer.Ordinal));

    private static bool TryViewModelName(string file, out string feature)
    {
        const string marker = ".viewModel.";
        var index = file.IndexOf(marker, StringComparison.OrdinalIgnoreCase);
        feature = index > 0 ? file[..index] : "";
        return feature.Length > 0;
    }

    private static bool TryFlutterViewModelName(string file, out string feature)
    {
        const string marker = "_view_model.dart";
        feature = file.EndsWith(marker, StringComparison.OrdinalIgnoreCase)
            ? PascalCase(file[..^marker.Length])
            : "";
        return feature.Length > 0;
    }

    private static string PascalCase(string value) => string.Concat(
        value.Split('_', StringSplitOptions.RemoveEmptyEntries)
            .Select(part => char.ToUpperInvariant(part[0]) + part[1..]));

    private static bool IsRuntimeWideInfrastructure(string path) =>
        path.Equals(SkiesManifest.FileName, StringComparison.OrdinalIgnoreCase)
        || path.Equals("global.json", StringComparison.OrdinalIgnoreCase)
        || Path.GetFileName(path).StartsWith("Directory.Build.", StringComparison.OrdinalIgnoreCase);

    private static bool IsGateControlInfrastructure(string path) =>
        path.Equals("lefthook.yml", StringComparison.OrdinalIgnoreCase)
        || path.StartsWith(".github/workflows/", StringComparison.OrdinalIgnoreCase)
        || path.StartsWith(".config/dotnet-tools", StringComparison.OrdinalIgnoreCase);

    private static bool IsRootFrontendContract(string path) =>
        !path.Contains('/') && path is "package.json" or "package-lock.json" or "npm-shrinkwrap.json";

    private static bool IsDocumentation(string path) =>
        path.EndsWith(".md", StringComparison.OrdinalIgnoreCase)
        || path.StartsWith("docs/", StringComparison.OrdinalIgnoreCase)
        || path.StartsWith(".skies/", StringComparison.OrdinalIgnoreCase)
        || Path.GetFileName(path).StartsWith("VERIFICATION", StringComparison.OrdinalIgnoreCase);

    private static bool IsFrontendPath(string path, string root) =>
        SkiesManifest.FrontendPackages(root)
            .Select(package => Normalize(Path.GetRelativePath(root, package.Path)))
            .Any(package => IsWithin(path, package));

    private static bool IsWithin(string path, string directory) =>
        path.Equals(directory, StringComparison.OrdinalIgnoreCase)
        || path.StartsWith(directory.TrimEnd('/') + "/", StringComparison.OrdinalIgnoreCase);

    private static bool SamePath(string left, string right) =>
        Normalize(left).Equals(Normalize(right), StringComparison.OrdinalIgnoreCase);

    private static string Normalize(string path)
    {
        var normalized = path.Replace('\\', '/');
        while (normalized.StartsWith("./", StringComparison.Ordinal))
            normalized = normalized[2..];
        return normalized;
    }
}
