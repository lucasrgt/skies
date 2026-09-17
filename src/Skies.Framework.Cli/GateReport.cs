using System.Globalization;
using System.Text;
using System.Text.Json;

namespace Skies.Framework.Cli;

/// <summary>The deterministic legs' exit codes, carried into the artifacts so a report is self-contained.</summary>
/// <param name="Doctor">The doctor's exit code (manifest + sync + build/SKY* + frontend SKYFE* legs).</param>
/// <param name="Tests">The proof run's exit code (<c>dotnet test</c> over the workspace).</param>
/// <param name="Frontends">Every frontend client's unit, AVP, feature-E2E, E2E-shape and real-E2E outcomes.</param>
/// <param name="SkippedTests">Tests reported as not executed by the .NET runner.</param>
internal sealed record GateLegs(
    int Doctor,
    int Tests,
    IReadOnlyList<FrontendGateLeg>? Frontends = null,
    int SkippedTests = 0,
    string Scope = "full")
{
    /// <summary>Frontend results, normalized to an empty list for backend-only workspaces.</summary>
    public IReadOnlyList<FrontendGateLeg> FrontendRuns => Frontends ?? [];
}

/// <summary>
/// Renders the <see cref="GateMatrix"/> three ways: a console table for the terminal, a
/// <c>VERIFICATION.md</c> that travels with the repo (the human-curated reading surface), and a
/// <c>VERIFICATION.json</c> for machines (CI, the harness fan-out). Same matrix, three faces — the
/// renderers never recompute a verdict.
/// </summary>
internal static class GateReport
{
    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        WriteIndented = true,
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
    };

    /// <summary>Whether the whole gate is green: every leg exit 0 and the matrix total.</summary>
    public static bool Green(GateMatrix matrix, GateLegs legs) =>
        legs.Doctor == 0 && legs.Tests == 0 && legs.SkippedTests == 0
        && legs.FrontendRuns.All(leg => leg.Green) && !matrix.Blocking;

    /// <summary>
    /// Whether a selected leg could not run. Exit 2 or greater is the toolchain's "incomplete validation" code,
    /// and the distinction is the whole point of reporting it: a run that never executed a proof has disproven
    /// nothing, and calling it RED sends the reader looking for a failure that was never observed.
    /// </summary>
    public static bool Incomplete(GateLegs legs) =>
        legs.Doctor >= 2 || legs.Tests >= 2 || legs.FrontendRuns.Any(leg => leg.Incomplete);

    /// <summary>The single verdict every renderer prints, so the console, the markdown and the JSON never disagree.</summary>
    public static string Verdict(GateMatrix matrix, GateLegs legs) =>
        Green(matrix, legs) ? "GREEN" : Incomplete(legs) ? "INCOMPLETE" : "RED";

    /// <summary>Write the per-module traceability table and the findings to <paramref name="writer"/>.</summary>
    public static void WriteConsole(GateMatrix matrix, GateLegs legs, TextWriter writer)
    {
        writer.WriteLine();
        writer.WriteLine($"gate matrix ({legs.Scope}) — {matrix.Rows.Count} declared criteria across "
                       + $"{matrix.Rows.Select(r => r.Module).Distinct().Count()} modules");
        var visible = legs.Scope == "full" ? matrix.Rows : matrix.Rows.Where(row => row.Verdict != MatrixVerdict.NotAffected);
        var unaffected = matrix.Rows.Count(row => row.Verdict == MatrixVerdict.NotAffected);
        if (unaffected > 0)
            writer.WriteLine($"  {unaffected} criteria not affected; omitted from this execution report (not counted as passes)");
        foreach (var module in visible.GroupBy(r => r.Module))
        {
            var proven = module.Count(r => r.Verdict == MatrixVerdict.Pass);
            var declared = matrix.Rows.Count(row => row.Module == module.Key);
            var omitted = matrix.Rows.Count(row => row.Module == module.Key && row.Verdict == MatrixVerdict.NotAffected);
            writer.WriteLine($"  {module.Key} — {proven} executed/proven, {omitted} not affected, {declared} declared");
            foreach (var row in module.Where(row => legs.Scope == "full" || row.Verdict != MatrixVerdict.Pass))
                writer.WriteLine($"    {row.Slice,-28} {row.CriterionId,-34} {Label(row.Verdict),-9} {FirstProof(row)}");
        }

        foreach (var orphan in matrix.OrphanProofs)
            writer.WriteLine($"  creep: AVP {orphan.Subject ?? "(unbound)"}/{orphan.CriterionId} at {orphan.File} "
                           + $"({orphan.ClassName}.{orphan.Method}) has no exact manifest obligation");
        foreach (var slice in matrix.UndeclaredSlices)
            writer.WriteLine($"  undeclared: [Slice] {slice.Module}/{slice.Name} ({slice.File}) declares no criterion");
        foreach (var missing in matrix.DeclaredWithoutClass)
            writer.WriteLine($"  note: {missing.Module}/{missing.Slice} is declared in {missing.ManifestPath} but has no [Slice] class");
        foreach (var broken in matrix.MalformedManifests)
            writer.WriteLine($"  malformed: {broken.Path} — {broken.Error}");
        foreach (var frontend in legs.FrontendRuns)
            writer.WriteLine($"  frontend {frontend.Client} ({Role(frontend)}, {frontend.Scope}) exits: tests={Exit(frontend.Tests)} "
                           + $"avp={Exit(frontend.Avp)} rendered={Exit(frontend.RenderedDesign)} "
                           + $"feature-e2e={frontend.FeatureE2e} e2e-shape={Exit(frontend.E2eShape)} "
                           + $"e2e={Exit(frontend.E2e)} (0 = pass)");
        if (legs.SkippedTests > 0)
            writer.WriteLine($"  skipped: {legs.SkippedTests} .NET test(s) did not execute");
        writer.WriteLine($"  verdict: {Verdict(matrix, legs)}");
        writer.WriteLine();
    }

    /// <summary>The repo-traveling artifact: the matrix as reviewable markdown.</summary>
    public static string Markdown(GateMatrix matrix, GateLegs legs, DateTimeOffset generatedAt)
    {
        var md = new StringBuilder();
        md.AppendLine("# Verification matrix");
        md.AppendLine();
        md.AppendLine(CultureInfo.InvariantCulture,
            $"> Generated by `skies gate` ({legs.Scope}) on {generatedAt:yyyy-MM-dd HH:mm zzz}. Do not edit — re-run the gate to refresh.");
        md.AppendLine();
        md.AppendLine(CultureInfo.InvariantCulture,
            $"**Gate verdict: {Verdict(matrix, legs)}** — doctor exit {legs.Doctor} · tests exit {legs.Tests} "
          + $"({legs.SkippedTests} skipped) · "
          + $"matrix {(matrix.Blocking ? "has findings" : "total")}.");

        if (legs.FrontendRuns.Count > 0)
        {
            md.AppendLine();
            md.AppendLine("Frontend leg exit codes (`0` = pass):");
            md.AppendLine();
            md.AppendLine("| Frontend | Role | Tests exit | AVP exit | Rendered exit | Feature E2E exit | E2E shape exit | E2E run exit |");
            md.AppendLine("|---|---|---:|---:|---:|---:|---:|---:|");
            foreach (var frontend in legs.FrontendRuns)
                md.AppendLine(CultureInfo.InvariantCulture,
                    $"| {frontend.Client} | {Role(frontend)} · {frontend.Scope} | {Exit(frontend.Tests)} | {Exit(frontend.Avp)} | "
                  + $"{Exit(frontend.RenderedDesign)} | {frontend.FeatureE2e} | "
                  + $"{Exit(frontend.E2eShape)} | {Exit(frontend.E2e)} |");
        }

        foreach (var module in matrix.Rows.GroupBy(r => r.Module))
        {
            var proven = module.Count(r => r.Verdict == MatrixVerdict.Pass);
            var omitted = module.Count(r => r.Verdict == MatrixVerdict.NotAffected);
            md.AppendLine();
            md.AppendLine(CultureInfo.InvariantCulture,
                $"## {module.Key} — {proven} executed/proven · {omitted} not affected · {module.Count()} declared");
            md.AppendLine();
            md.AppendLine("| Slice | Criterion | Proof | Verdict |");
            md.AppendLine("|---|---|---|---|");
            foreach (var row in module)
                md.AppendLine(CultureInfo.InvariantCulture,
                    $"| {row.Slice} | `{row.CriterionId}` | {ProofCell(row)} | {Label(row.Verdict)} |");
        }

        AppendFindings(md, matrix);
        return md.ToString();
    }

    /// <summary>The machine-readable artifact, camelCased and indented for diff-friendly commits.</summary>
    public static string Json(GateMatrix matrix, GateLegs legs, DateTimeOffset generatedAt)
    {
        var payload = new
        {
            Tool = "skies gate",
            GeneratedAt = generatedAt,
            Verdict = Verdict(matrix, legs).ToLowerInvariant(),
            Legs = new { legs.Scope, legs.Doctor, legs.Tests, legs.SkippedTests, Frontends = legs.FrontendRuns },
            Modules = matrix.Rows.GroupBy(r => r.Module).Select(module => new
            {
                Module = module.Key,
                Declared = module.Count(),
                Proven = module.Count(r => r.Verdict == MatrixVerdict.Pass),
                NotAffected = module.Count(r => r.Verdict == MatrixVerdict.NotAffected),
                Rows = module.Select(r => new
                {
                    r.Slice,
                    Criterion = r.CriterionId,
                    Verdict = Label(r.Verdict),
                    Proofs = r.Proofs.Select(p => new { p.File, Class = p.ClassName, p.Method }),
                }),
            }),
            Findings = new
            {
                OrphanProofs = matrix.OrphanProofs.Select(p => new
                {
                    p.Module,
                    p.Subject,
                    Criterion = p.CriterionId,
                    p.File,
                    Class = p.ClassName,
                    p.Method,
                }),
                UndeclaredSlices = matrix.UndeclaredSlices.Select(s => new { s.Module, Slice = s.Name, s.File }),
                DeclaredWithoutClass = matrix.DeclaredWithoutClass.Select(d => new { d.Module, d.Slice, Manifest = d.ManifestPath }),
                MalformedManifests = matrix.MalformedManifests.Select(m => new { m.Path, m.Error }),
            },
        };
        return JsonSerializer.Serialize(payload, JsonOptions);
    }

    private static void AppendFindings(StringBuilder md, GateMatrix matrix)
    {
        if (!matrix.Blocking && matrix.DeclaredWithoutClass.Count == 0)
            return;

        md.AppendLine();
        md.AppendLine("## Findings");
        foreach (var orphan in matrix.OrphanProofs)
            md.AppendLine(CultureInfo.InvariantCulture,
                $"- **Creep** — AVP `{orphan.Subject ?? "(unbound)"}/{orphan.CriterionId}` at `{orphan.File}` "
              + $"({orphan.ClassName}.{orphan.Method}) has no exact manifest obligation.");
        foreach (var slice in matrix.UndeclaredSlices)
            md.AppendLine(CultureInfo.InvariantCulture,
                $"- **Undeclared slice** — `{slice.Module}/{slice.Name}` (`{slice.File}`) declares no acceptance criterion.");
        foreach (var broken in matrix.MalformedManifests)
            md.AppendLine(CultureInfo.InvariantCulture, $"- **Malformed manifest** — `{broken.Path}`: {broken.Error}");
        foreach (var missing in matrix.DeclaredWithoutClass)
            md.AppendLine(CultureInfo.InvariantCulture,
                $"- *Note* — `{missing.Module}/{missing.Slice}` is declared in `{missing.ManifestPath}` but has no `[Slice]` class yet.");
    }

    private static string Label(MatrixVerdict verdict) => verdict switch
    {
        MatrixVerdict.Pass => "pass",
        MatrixVerdict.Fail => "FAIL",
        MatrixVerdict.NotRun => "NOT-RUN",
        MatrixVerdict.NotAffected => "not-affected",
        _ => "NO-PROOF",
    };

    private static string Role(FrontendGateLeg frontend) => frontend.Role switch
    {
        FrontendPackageRole.Core => "core",
        FrontendPackageRole.Library => "library",
        _ => "surface",
    };

    private static string Exit(int? exitCode) => exitCode?.ToString(CultureInfo.InvariantCulture) ?? "n/a";

    private static string ProofCell(MatrixRow row) => row.Proofs.Count switch
    {
        0 => "—",
        1 => $"`{row.Proofs[0].ClassName}.{row.Proofs[0].Method}` — {row.Proofs[0].File}",
        _ => $"`{row.Proofs[0].ClassName}.{row.Proofs[0].Method}` — {row.Proofs[0].File} (+{row.Proofs.Count - 1} more)",
    };

    private static string FirstProof(MatrixRow row) =>
        row.Proofs.Count == 0 ? "—" : $"{row.Proofs[0].ClassName}.{row.Proofs[0].Method}";
}
