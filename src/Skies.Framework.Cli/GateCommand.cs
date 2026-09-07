namespace Skies.Framework.Cli;

/// <summary>
/// <c>skies gate</c> — the Clockwork done-gate as one deterministic command. It composes the existing legs
/// instead of duplicating them: the doctor (manifest + framework sync + build with the SKY* analyzers,
/// including the SKY0030/SKY0031 bridge + the frontend SKYFE* legs), then the proof run (<c>dotnet test</c>,
/// which executes every <c>[AVP]</c> verification), and finally joins declarations × proofs × verdicts into
/// the traceability matrix. A full audit persists the canonical <c>VERIFICATION.md</c> (for humans, committed
/// with the repo) and <c>VERIFICATION.json</c> (for machines). Change-scoped gates report the same verdict to
/// the console without dirtying the application checkout; their exit code remains the machine contract.
/// </summary>
internal static class GateCommand
{
    private const int MaxInlineFilterLength = 6000;

    // Marks the reasons ApplyFastFeedback contributes, so the closing notice can restate exactly what --fast held
    // back without re-deriving it or matching on prose.
    internal const string FastDeferralMarker = "deferred by --fast";

    /// <summary>The human-facing matrix artifact, written at the workspace root.</summary>
    public const string MarkdownArtifact = "VERIFICATION.md";

    /// <summary>The machine-facing matrix artifact, written at the workspace root.</summary>
    public const string JsonArtifact = "VERIFICATION.json";

    /// <summary>Run the Git-affected gate by default, or the explicitly requested staged/full variant.</summary>
    /// <param name="rest">Gate options and arguments forwarded to the build/test legs.</param>
    public static int Run(string[] rest)
    {
        if (TryWriteHelp(rest, Console.Out))
            return 0;

        var root = Directory.GetCurrentDirectory();
        if (!GateOptions.TryParse(rest, out var options, out var error))
        {
            Console.Error.WriteLine($"skies gate: {error}.");
            return 2;
        }

        var changes = GitChanges.Read(root, options);
        if (!changes.Reliable || options.Mode != GateMode.Full && changes.Comparison is null)
        {
            Console.Error.WriteLine($"skies gate: Git scope is unavailable ({changes.Message}); no proof run started. "
                + "Fetch the comparison revision and retry the same scoped check, or request --full explicitly.");
            return 2;
        }
        return GateAttempt.Run(root, options, () => Execute(root, options, changes), Console.Error);
    }

    private static int Execute(string root, GateOptions options, GitChangeSet changes)
    {
        var effectiveFull = options.Mode == GateMode.Full;

        var manifests = GateScan.DiscoverManifests(root);
        var proofs = GateScan.ScanProofs(root);
        var slices = GateScan.ScanSlices(root);
        var journeys = GateScan.ScanJourneys(root);
        var targets = DoctorCommand.FrontendTargets(root);
        GateImpactPlan impact;
        if (effectiveFull)
        {
            impact = new GateImpactPlan(
                new BackendImpact(true, new HashSet<string>(), new HashSet<string>()),
                targets.Select(target => new FrontendImpact(target) { Full = true }).ToList(),
                ["explicit or fail-closed full gate"]);
        }
        else
        {
            impact = GateImpact.Build(
                root,
                changes.Files,
                slices,
                proofs,
                journeys,
                GateScan.ScanTestClasses(root),
                targets,
                changes.Comparison);
        }
        impact = ApplyFastFeedback(impact, options.Fast);

        var scope = effectiveFull ? "full" : options.Mode == GateMode.Staged ? "staged" : "affected";
        if (options.Fast)
            scope += "-fast";
        Console.WriteLine($"skies gate ({scope}) — form ∧ Git-derived proof closure ∧ traceability.");
        if (!effectiveFull)
            Console.WriteLine($"skies gate — {changes.Files.Count} changed path(s) root the impact graph.");
        foreach (var reason in impact.Reasons)
            Console.WriteLine($"  select: {reason}");

        var forwarded = options.ToolArguments.ToArray();
        var suppressions = SuppressionGate.Run(root);
        var doctor = DoctorCommand.Run(forwarded, strictWarnings: true);

        var results = Path.Combine(Path.GetTempPath(), "skies-gate-" + Guid.NewGuid().ToString("N"));
        var tests = 0;
        IReadOnlyList<TestVerdict> verdicts = [];
        if (impact.Backend.RunsTests)
        {
            Console.WriteLine("skies gate — backend proofs (dotnet test)...");
            tests = Tooling.Dotnet("test", ProofArguments(forwarded, doctor, results, impact.Backend));
            try
            {
                verdicts = GateScan.ParseTrxDirectory(results);
            }
            catch (InvalidDataException exception)
            {
                Console.Error.WriteLine($"skies gate: incomplete test evidence: {exception.Message}");
                tests = Math.Max(tests, 2);
            }
            if (verdicts.Count == 0)
            {
                Console.Error.WriteLine("skies gate: the selected backend run produced no executable test evidence.");
                tests = Math.Max(tests, 2);
            }
        }
        else
        {
            Console.WriteLine("skies gate — backend proofs: not affected.");
        }
        var skippedTests = verdicts.Count(verdict => verdict.Outcome == "NotExecuted");
        TryDelete(results);

        var frontend = FrontendGate.Run(root, targets, impact.Frontends, options.Fast);

        var matrix = GateMatrix.Build(
            manifests,
            proofs,
            slices,
            verdicts,
            impact.Backend.Full ? null : impact.Backend.AffectedSlices);
        var legs = new GateLegs(doctor, tests, frontend, skippedTests, scope);

        GateReport.WriteConsole(matrix, legs, Console.Out);
        if (PersistsArtifacts(options.Mode))
        {
            File.WriteAllText(Path.Combine(root, MarkdownArtifact), GateReport.Markdown(matrix, legs, DateTimeOffset.Now));
            File.WriteAllText(Path.Combine(root, JsonArtifact), GateReport.Json(matrix, legs, DateTimeOffset.Now));
            Console.WriteLine($"gate: wrote {MarkdownArtifact} + {JsonArtifact}.");
        }
        else
        {
            Console.WriteLine("gate: change-scoped verdict emitted without replacing the canonical full-audit artifacts.");
        }

        var code = Math.Max(Math.Max(Math.Max(suppressions, doctor), tests), matrix.Blocking ? 1 : 0);
        if (frontend.Any(leg => !leg.Green))
            code = Math.Max(code, 1);
        if (skippedTests > 0)
            code = Math.Max(code, 1);
        Console.WriteLine(code == 0
            ? "gate: GREEN — form, proofs and the matrix all hold."
            : "gate: RED — a leg failed or the matrix has findings (see above).");

        var deferred = DeferredCoverage(options, effectiveFull, impact, frontend);
        if (deferred.Count > 0)
        {
            Console.WriteLine($"gate: this run did not cover ({scope}) —");
            foreach (var notice in deferred)
                Console.WriteLine($"  not covered: {notice}");
        }
        return code;
    }

    /// <summary>
    /// What this run did not prove, restated after the verdict. The selection reasons print before a long doctor and
    /// proof run and have scrolled away by the time the verdict lands, and a green change-scoped line reads like a
    /// full pass — which is how a branch accumulates commits behind green pre-commit and pre-push hooks while its
    /// E2E closure has been red since its first slice. The gate's scope is correct; only its silence was.
    /// </summary>
    internal static IReadOnlyList<string> DeferredCoverage(
        GateOptions options,
        bool effectiveFull,
        GateImpactPlan impact,
        IReadOnlyList<FrontendGateLeg> frontend)
    {
        if (effectiveFull)
            return [];

        var notices = new List<string>();
        // --fast runs the E2E contract check but never the runner, so a surface package reports an E2E leg that
        // never executed a flow. Naming it is the difference between a caveat and a false green.
        if (options.Fast && frontend.Any(leg => leg.Role == FrontendPackageRole.Surface))
            notices.Add(
                "browser/device E2E execution — --fast runs the E2E contract check only; no flow was driven. "
                + "Run the repository's authoritative affected check before pushing; reserve --full for release audits.");

        notices.AddRange(impact.Reasons
            .Where(reason => reason.Contains(FastDeferralMarker, StringComparison.Ordinal))
            .Select(reason => reason.Replace($" {FastDeferralMarker}", string.Empty, StringComparison.Ordinal)));

        notices.Add(
            $"proofs outside the {(options.Mode == GateMode.Staged ? "staged" : "affected")} closure — a "
            + "change-scoped gate proves the impact of these changes, not the suite.");
        return notices;
    }

    /// <summary>Print gate-specific help before any workspace discovery or proof execution begins.</summary>
    internal static bool TryWriteHelp(IReadOnlyCollection<string> arguments, TextWriter output)
    {
        if (!arguments.Contains("--help") && !arguments.Contains("-h") && !arguments.Contains("-?"))
            return false;

        output.WriteLine(
            """
            skies gate — execute the mandatory proof closure

            usage:
              skies gate [--affected] [--base <rev>] [--fast] [dotnet arguments...]
              skies gate --staged [--fast] [dotnet arguments...]
              skies gate --full [dotnet arguments...]
              Add --retry-review <json-file> after a failed or interrupted expensive attempt.

            modes:
              --affected   select proofs from Git changes; this is the default
              --staged     select proofs from paths currently staged in the Git index
              --full       execute every backend, Assay, and browser/device proof

            options:
              --base <rev> compare <rev>...HEAD in affected mode; local uncommitted paths are excluded
              --fast       keep local feedback bounded: no browser/device E2E flow is driven and an
                           exhaustive fallback is held back; authoritative affected CI executes both.
                           Every run closes by listing what it did not cover.
              -h, --help   print this help without executing the gate

            The gate always runs the universal inventory and rejects caller-authored test filters.
            """);
        return true;
    }

    /// <summary>
    /// Keep local feedback bounded: directly mapped proof filters still execute, while an exhaustive or oversized
    /// transitive closure waits for authoritative affected CI or an explicit full audit. The reason remains visible.
    /// </summary>
    internal static GateImpactPlan ApplyFastFeedback(GateImpactPlan impact, bool fast)
    {
        var exhaustiveFrontend = impact.Frontends.Any(frontend => frontend.Full || frontend.ExhaustiveFallback);
        if (!fast)
        {
            if (!impact.Frontends.Any(frontend => frontend.ExhaustiveFallback))
                return impact;
            return impact with
            {
                Frontends = impact.Frontends
                    .Select(frontend => frontend.ExhaustiveFallback ? FullFrontend(frontend) : frontend)
                    .ToList(),
            };
        }

        if (!impact.Backend.Full && !exhaustiveFrontend)
            return impact;

        var reasons = new List<string>(impact.Reasons);
        if (impact.Backend.Full)
            reasons.Add($"backend: exhaustive fallback {FastDeferralMarker}; the authoritative affected boundary or an explicit --full audit executes it");
        if (exhaustiveFrontend)
            reasons.Add($"frontend: exhaustive runtime closure {FastDeferralMarker}; the authoritative affected boundary executes every test and Assay");

        return impact with
        {
            Backend = new BackendImpact(
                false,
                impact.Backend.Filters,
                impact.Backend.AffectedSlices),
            Frontends = impact.Frontends
                .Select(frontend => frontend.Full || frontend.ExhaustiveFallback
                    ? CopyFrontend(frontend, full: false)
                    : frontend)
                .ToList(),
            Reasons = reasons,
        };
    }

    private static FrontendImpact FullFrontend(FrontendImpact source) => CopyFrontend(source, full: true);

    private static FrontendImpact CopyFrontend(FrontendImpact source, bool full)
    {
        var copy = new FrontendImpact(source.Package) { Full = full };
        copy.Tests.UnionWith(source.Tests);
        copy.Assays.UnionWith(source.Assays);
        copy.Flows.AddRange(source.Flows);
        copy.RenderedDesign = source.RenderedDesign;
        return copy;
    }

    /// <summary>Build the proof-run arguments, reusing a doctor build only when it actually passed.</summary>
    internal static string[] ProofArguments(
        string[] rest,
        int doctorExit,
        string resultsDirectory,
        BackendImpact? impact = null)
    {
        var arguments = new List<string>(rest);
        if (doctorExit == 0 && !arguments.Contains("--no-build", StringComparer.OrdinalIgnoreCase))
            arguments.Add("--no-build");
        arguments.Add("--logger");
        arguments.Add("trx");
        arguments.Add("--results-directory");
        arguments.Add(resultsDirectory);
        if (impact is { Full: false } && impact.Filters.Count > 0)
        {
            var filter = ProofFilter(impact);
            if (filter.Length <= MaxInlineFilterLength)
            {
                arguments.Add("--filter");
                arguments.Add(filter);
            }
            else
            {
                // A long selection belongs in the runner's standard settings file, never an unfiltered run or
                // several overlapping batches that execute the same shared proof repeatedly.
                Directory.CreateDirectory(resultsDirectory);
                var settingsPath = Path.Combine(resultsDirectory, "selected.runsettings");
                var settings = new System.Xml.Linq.XDocument(
                    new System.Xml.Linq.XElement("RunSettings",
                        new System.Xml.Linq.XElement("RunConfiguration",
                            new System.Xml.Linq.XElement("TestCaseFilter", filter))));
                settings.Save(settingsPath);
                arguments.Add("--settings");
                arguments.Add(settingsPath);
            }
        }
        return [.. arguments];
    }

    private static string ProofFilter(BackendImpact impact) =>
        string.Join('|', impact.Filters.Order().Select(filter => $"FullyQualifiedName~{filter}"));

    /// <summary>Keep the committed attestation stable until an explicitly requested exhaustive audit replaces it.</summary>
    internal static bool PersistsArtifacts(GateMode mode) => mode == GateMode.Full;

    // The TRX scratch dir is disposable; a locked file on Windows must never fail the gate itself.
    private static void TryDelete(string directory)
    {
        try
        {
            if (Directory.Exists(directory))
                Directory.Delete(directory, recursive: true);
        }
        catch (IOException)
        {
        }
        catch (UnauthorizedAccessException)
        {
        }
    }
}
