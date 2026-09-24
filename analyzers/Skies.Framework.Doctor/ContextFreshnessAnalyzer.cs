using System;
using System.Collections.Generic;
using System.Collections.Immutable;
using System.IO;
using System.Linq;
using System.Text.RegularExpressions;
using System.Threading;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.Diagnostics;
using Microsoft.CodeAnalysis.Text;

namespace Skies.Framework.Doctor;

/// <summary>
/// SKY0005 — a module's <c>.ctx.md</c> stays fresh by <em>citation resolution</em>: a backticked code
/// identifier it names must still exist in this module's source or a referenced assembly. A ctx that names <c>`AttachCtx`</c> after that construct was renamed or removed is rot, and the doctor
/// catches it; this is the .NET analog of the <c>attach_ctx</c> documentation drift that the Rust codebase's
/// docs-hygiene gate was built to prevent.
///
/// A ctx documents the module, so it cites production code. Test classes are not part of the api compilation and
/// do not count: the module's behavior is proven by its specs under <c>.specs/</c>, which the ctx does not index.
///
/// Freshness is deliberately <b>not</b> mtime. Because the ctx does not duplicate the code (see the
/// schema in CONVENTIONS), adding a field must not force a ctx edit — only a *dangling* reference is
/// stale. A citation is a single PascalCase identifier inside backticks (<c>`Refresh`</c>); prose,
/// lowercase tokens (claim names, paths), and qualified or punctuated spans are ignored. The expensive
/// walk of referenced assemblies runs only when a suspect appears — an identifier absent from this
/// source — so a fresh ctx costs nothing.
///
/// A ctx also cites the specs that prove its invariants, as <c>`0002-withdraw`</c> or
/// <c>`0002-withdraw#FM-n`</c> (see <see cref="SpecCitations"/>): the spec must exist and, when a failure mode is
/// named, its spec.md must list it. The specs are read as AdditionalFiles (<c>.specs/*/spec.md</c>); a spec
/// citation starts with digits, so the two kinds of citation never overlap.
/// </summary>
[DiagnosticAnalyzer(LanguageNames.CSharp)]
public sealed class ContextFreshnessAnalyzer : DiagnosticAnalyzer
{
    /// <summary>The identifier reported for a ctx that names code which no longer exists.</summary>
    public const string DiagnosticId = "SKY0005";

    private static readonly DiagnosticDescriptor Rule = new(
        id: DiagnosticId,
        title: "ctx.md must not cite code that no longer exists",
        messageFormat: "{0} cites `{1}`, which no longer exists in the code; update or remove the reference",
        category: "Skies.Framework.Convention",
        defaultSeverity: DiagnosticSeverity.Error,
        isEnabledByDefault: true,
        description: "A module .ctx.md stays fresh by citation resolution: a backticked identifier it "
                   + "names must still exist in this source or a referenced assembly. A dangling "
                   + "reference is documentation rot.",
        customTags: WellKnownDiagnosticTags.CompilationEnd);

    private static readonly DiagnosticDescriptor SpecRule = new(
        id: DiagnosticId,
        title: "ctx.md must not cite a spec or failure mode that does not exist",
        messageFormat: "{0} cites `{1}`, {2}; update or remove the citation",
        category: "Skies.Framework.Convention",
        defaultSeverity: DiagnosticSeverity.Error,
        isEnabledByDefault: true,
        description: "A module .ctx.md cites the spec that proves an invariant as `<id>-<slug>` or "
                   + "`<id>-<slug>#FM-n`. The spec folder must exist under .specs/ and, when a failure mode is "
                   + "cited, its spec.md must list it. A dangling spec citation is documentation rot.",
        customTags: WellKnownDiagnosticTags.CompilationEnd);

    // A citation: a single PascalCase identifier whose entire backtick span is that identifier. Prose,
    // lowercase (claim/header names), and punctuated spans (`a.b`, `f(x)`, `X-Client`) never match.
    private static readonly Regex CitationPattern = new(@"`([A-Z][A-Za-z0-9_]*)`", RegexOptions.Compiled);

    /// <inheritdoc />
    public override ImmutableArray<DiagnosticDescriptor> SupportedDiagnostics => ImmutableArray.Create(Rule, SpecRule);

    /// <inheritdoc />
    public override void Initialize(AnalysisContext context)
    {
        context.ConfigureGeneratedCodeAnalysis(GeneratedCodeAnalysisFlags.None);
        context.EnableConcurrentExecution();
        context.RegisterCompilationAction(OnCompilation);
    }

    private static void OnCompilation(CompilationAnalysisContext context)
    {
        var ctxFiles = context.Options.AdditionalFiles
            .Where(f => f.Path.EndsWith(".ctx.md", StringComparison.OrdinalIgnoreCase))
            .ToArray();
        if (ctxFiles.Length == 0)
            return;

        // Per ctx: every citation and where it sits, so we can report into the .ctx.md itself.
        var perFile = ctxFiles
            .Select(f => (file: f, text: f.GetText(context.CancellationToken)))
            .Where(x => x.text is not null)
            .Select(x => (x.file, x.text!, citations: Citations(x.text!)))
            .ToArray();

        ReportSpecCitations(context, perFile.Select(x => (x.file, x.Item2)));

        var allNames = perFile.SelectMany(x => x.citations.Select(c => c.Name)).ToImmutableHashSet();
        if (allNames.IsEmpty)
            return;

        var source = SourceNames(context.Compilation, context.CancellationToken);
        var suspects = new HashSet<string>(allNames.Where(n => !source.Contains(n)), StringComparer.Ordinal);
        if (suspects.Count > 0)
            suspects.ExceptWith(ReferencedNames(context.Compilation, suspects, context.CancellationToken));
        if (suspects.Count == 0)
            return;   // everything resolves — fresh

        foreach (var (file, text, citations) in perFile)
            foreach (var citation in citations)
                if (suspects.Contains(citation.Name))
                {
                    var location = Location.Create(file.Path, citation.Span, text.Lines.GetLinePositionSpan(citation.Span));
                    context.ReportDiagnostic(Diagnostic.Create(Rule, location, Path.GetFileName(file.Path), citation.Name));
                }
    }

    // A spec citation that names a missing spec folder, or a failure mode its spec.md does not list, is stale.
    private static void ReportSpecCitations(CompilationAnalysisContext context, IEnumerable<(AdditionalText File, SourceText Text)> ctxs)
    {
        Dictionary<string, HashSet<int>>? specs = null;
        foreach (var (file, text) in ctxs)
            foreach (var citation in SpecCitations.In(text))
            {
                specs ??= SpecCitations.Index(context.Options.AdditionalFiles, context.CancellationToken);
                if (SpecCitations.Problem(citation, specs) is not { } problem)
                    continue;
                var location = Location.Create(file.Path, citation.Span, text.Lines.GetLinePositionSpan(citation.Span));
                context.ReportDiagnostic(Diagnostic.Create(SpecRule, location, Path.GetFileName(file.Path), citation.Text, problem));
            }
    }

    private static List<(string Name, TextSpan Span)> Citations(SourceText text)
    {
        var content = text.ToString();
        var found = new List<(string, TextSpan)>();
        foreach (Match match in CitationPattern.Matches(content))
        {
            var group = match.Groups[1];
            found.Add((group.Value, new TextSpan(group.Index, group.Length)));
        }

        return found;
    }

    // Every simple name declared in this compilation's own source — types and members alike.
    private static HashSet<string> SourceNames(Compilation compilation, CancellationToken ct)
    {
        var names = new HashSet<string>(StringComparer.Ordinal);
        foreach (var symbol in compilation.GetSymbolsWithName(_ => true, SymbolFilter.TypeAndMember, ct))
            names.Add(symbol.Name);
        return names;
    }

    // Which of the suspects exist in a referenced assembly (type or member). Walks references only
    // because a suspect appeared, and short-circuits once all suspects are accounted for.
    private static HashSet<string> ReferencedNames(Compilation compilation, HashSet<string> suspects, CancellationToken ct)
    {
        var found = new HashSet<string>(StringComparer.Ordinal);

        foreach (var reference in compilation.References)
        {
            if (found.Count == suspects.Count)
                break;
            if (compilation.GetAssemblyOrModuleSymbol(reference) is IAssemblySymbol assembly)
                Collect(assembly.GlobalNamespace, suspects, found, ct);
        }

        return found;

        static void Collect(INamespaceSymbol ns, HashSet<string> wanted, HashSet<string> found, CancellationToken ct)
        {
            ct.ThrowIfCancellationRequested();
            foreach (var type in ns.GetTypeMembers())
                CollectType(type, wanted, found);
            foreach (var child in ns.GetNamespaceMembers())
                Collect(child, wanted, found, ct);
        }

        static void CollectType(INamedTypeSymbol type, HashSet<string> wanted, HashSet<string> found)
        {
            if (wanted.Contains(type.Name))
                found.Add(type.Name);
            foreach (var member in type.GetMembers())
                if (wanted.Contains(member.Name))
                    found.Add(member.Name);
            foreach (var nested in type.GetTypeMembers())
                CollectType(nested, wanted, found);
        }
    }
}
