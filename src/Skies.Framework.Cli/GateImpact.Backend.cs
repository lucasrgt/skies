namespace Skies.Framework.Cli;

internal static partial class GateImpact
{
    private static BackendImpact SelectBackend(
        string root,
        IReadOnlyList<string> changes,
        IReadOnlyList<SliceSite> slices,
        IReadOnlyList<AvpProof> proofs,
        IReadOnlyList<JourneyProof> journeys,
        IReadOnlyList<CSharpTestSite> testClasses,
        bool global,
        List<string> reasons)
    {
        if (global)
        {
            reasons.Add("backend: runtime-wide build infrastructure changed; selecting the full backend");
            return FullBackend();
        }

        var backendRoots = SkiesManifest.BackendPaths(root)
            .Select(path => Normalize(Path.GetRelativePath(root, path))).ToList();
        var filters = new HashSet<string>(StringComparer.Ordinal);
        var affected = new HashSet<string>(StringComparer.Ordinal);
        var directFilters = new HashSet<string>(StringComparer.Ordinal);
        var directAffected = new HashSet<string>(StringComparer.Ordinal);
        var runtimeAffected = new HashSet<string>(StringComparer.Ordinal);
        var full = false;
        CSharpImpactGraph? csharpGraph = null;

        foreach (var change in changes)
        {
            if (IsDocumentation(change) || IsFrontendPath(change, root))
                continue;

            var backendPath = backendRoots.Any(path => IsWithin(change, path));
            var backendContract = change.EndsWith(".spec.toml", StringComparison.OrdinalIgnoreCase)
                || change.EndsWith(".csproj", StringComparison.OrdinalIgnoreCase)
                || change.EndsWith(".props", StringComparison.OrdinalIgnoreCase)
                || change.EndsWith(".targets", StringComparison.OrdinalIgnoreCase)
                || change.EndsWith(".sln", StringComparison.OrdinalIgnoreCase)
                || change.EndsWith(".slnx", StringComparison.OrdinalIgnoreCase);
            if (backendContract)
            {
                if (change.EndsWith(".spec.toml", StringComparison.OrdinalIgnoreCase))
                {
                    var module = Path.GetFileName(change)[..^".spec.toml".Length];
                    foreach (var slice in slices.Where(slice => slice.Module == module))
                    {
                        SelectSlice(slice, directFilters, directAffected, proofs, journeys);
                        SelectSlice(slice, filters, affected, proofs, journeys);
                        runtimeAffected.Add(slice.Module + "/" + slice.Name);
                    }
                    reasons.Add($"backend: {change} selects the {module} module's declared proofs");
                    continue;
                }
                full = true;
                reasons.Add($"backend: {change} changes the proof/build contract; selecting all tests");
                continue;
            }

            if (!change.EndsWith(".cs", StringComparison.OrdinalIgnoreCase))
                continue;

            foreach (var slice in slices.Where(slice => Normalize(slice.File) == change))
                SelectSlice(slice, directFilters, directAffected, proofs, journeys);
            foreach (var proof in proofs.Where(proof => Normalize(proof.File) == change))
            {
                directFilters.Add(proof.ClassName);
                foreach (var slice in slices.Where(slice =>
                    slice.Module == proof.Module && slice.Name == proof.Subject))
                {
                    SelectSlice(slice, directFilters, directAffected, proofs, journeys);
                }
            }
            foreach (var journey in journeys.Where(journey => Normalize(journey.File) == change))
            {
                directFilters.Add(journey.ClassName);
                foreach (var slice in slices.Where(slice => slice.Name == journey.Subject))
                    SelectSlice(slice, directFilters, directAffected, proofs, journeys);
            }
            foreach (var site in testClasses.Where(site => Normalize(site.File) == change))
                directFilters.Add(site.ClassName);

            csharpGraph ??= CSharpImpactGraph.Build(root);
            var impactedFiles = csharpGraph.Expand(change);
            var matched = false;
            foreach (var slice in slices.Where(slice => impactedFiles.Contains(Normalize(slice.File))))
            {
                SelectSlice(slice, filters, affected, proofs, journeys);
                runtimeAffected.Add(slice.Module + "/" + slice.Name);
                matched = true;
            }

            foreach (var proof in proofs.Where(proof => impactedFiles.Contains(Normalize(proof.File))))
            {
                filters.Add(proof.ClassName);
                affected.Add(proof.Module + "/" + proof.Subject);
                // Selecting one proof selects its subject's whole matrix obligation, including other proof files.
                foreach (var slice in slices.Where(slice => slice.Module == proof.Module && slice.Name == proof.Subject))
                    SelectSlice(slice, filters, affected, proofs, journeys);
                matched = true;
            }

            foreach (var journey in journeys.Where(journey => impactedFiles.Contains(Normalize(journey.File))))
            {
                filters.Add(journey.ClassName);
                matched = true;
            }

            foreach (var site in testClasses.Where(site => impactedFiles.Contains(Normalize(site.File))))
            {
                filters.Add(site.ClassName);
                matched = true;
            }

            if (matched && impactedFiles.Count > 1)
                reasons.Add($"backend: {change} reaches {impactedFiles.Count - 1} transitive C# consumer(s)");

            if (!matched && backendPath)
            {
                full = true;
                reasons.Add($"backend: {change} has no unambiguous slice binding; selecting all tests");
            }
            else if (!matched)
            {
                full = true;
                reasons.Add($"backend: C# infrastructure {change} changed outside a declared backend; selecting all tests");
            }
        }

        if (full)
            return new BackendImpact(true, filters, affected, directFilters, directAffected) { RuntimeSlices = runtimeAffected };
        if (filters.Count > 0)
            reasons.Add($"backend: selected {affected.Count} slice(s) through {filters.Count} test filter term(s)");
        return new BackendImpact(false, filters, affected, directFilters, directAffected) { RuntimeSlices = runtimeAffected };
    }

}
