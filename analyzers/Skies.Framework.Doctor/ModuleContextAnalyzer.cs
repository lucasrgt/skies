using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Collections.Immutable;
using System.IO;
using System.Linq;
using System.Threading;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp;
using Microsoft.CodeAnalysis.CSharp.Syntax;
using Microsoft.CodeAnalysis.Diagnostics;

namespace Skies.Framework.Doctor;

/// <summary>
/// SKY0004 — every module that owns a slice carries a <c>&lt;Module&gt;.ctx.md</c> with its spine: a
/// non-empty <c>## Boundaries</c> and a non-empty <c>## Design notes</c>. The ctx is the home for the
/// business "why" the code cannot show, so a module never ships as undocumented code.
///
/// The module is the last segment of a slice's namespace (<c>App.Api.Modules.Account</c> → Account),
/// matching the convention that the namespace <em>is</em> the module. The ctx file is read from
/// <c>AdditionalFiles</c> — the app opts in with <c>&lt;AdditionalFiles Include="**\*.ctx.md" /&gt;</c>
/// — and matched by name (<c>Account.ctx.md</c>). This
/// proves the spine sections exist and carry content, not that the content is correct; freshness (a ctx
/// that names code which no longer exists) is <c>SKY0005</c>'s job.
/// </summary>
[DiagnosticAnalyzer(LanguageNames.CSharp)]
public sealed class ModuleContextAnalyzer : DiagnosticAnalyzer
{
    /// <summary>The identifier reported for a module missing its ctx, or its spine sections.</summary>
    public const string DiagnosticId = "SKY0004";

    private static readonly DiagnosticDescriptor Rule = new(
        id: DiagnosticId,
        title: "Module must have a ctx.md with its spine",
        messageFormat: "Module '{0}' {1}",
        category: "Skies.Framework.Convention",
        defaultSeverity: DiagnosticSeverity.Error,
        isEnabledByDefault: true,
        description: "Every module that owns a slice must carry a <Module>.ctx.md with a non-empty "
                   + "## Boundaries and ## Design notes section. The ctx is the home for the business "
                   + "why the code cannot show.",
        customTags: WellKnownDiagnosticTags.CompilationEnd);

    /// <summary>The spine sections every module ctx must carry, non-empty.</summary>
    private static readonly string[] SpineSections = ["Boundaries", "Design notes"];

    /// <inheritdoc />
    public override ImmutableArray<DiagnosticDescriptor> SupportedDiagnostics => ImmutableArray.Create(Rule);

    /// <inheritdoc />
    public override void Initialize(AnalysisContext context)
    {
        context.ConfigureGeneratedCodeAnalysis(GeneratedCodeAnalysisFlags.None);
        context.EnableConcurrentExecution();
        context.RegisterCompilationStartAction(OnStart);
    }

    private static void OnStart(CompilationStartAnalysisContext context)
    {
        // One representative location per module (the first slice seen), to report on.
        var modules = new ConcurrentDictionary<string, Location>();

        context.RegisterSyntaxNodeAction(syntax =>
        {
            var cls = (ClassDeclarationSyntax)syntax.Node;
            if (!IsSlice(cls))
                return;
            var module = ModuleNaming.ModuleOf(cls);
            if (module is not null)
                modules.TryAdd(module, cls.Identifier.GetLocation());
        }, SyntaxKind.ClassDeclaration);

        context.RegisterCompilationEndAction(end =>
        {
            if (modules.IsEmpty)
                return;

            var ctxByName = ReadContextFiles(end.Options.AdditionalFiles, end.CancellationToken);

            foreach (var entry in modules)
            {
                var module = entry.Key;
                var location = entry.Value;
                var expected = module + ".ctx.md";
                if (!ctxByName.TryGetValue(expected, out var content))
                {
                    end.ReportDiagnostic(Diagnostic.Create(Rule, location, module,
                        $"has no {expected} beside it; add one with a '## Boundaries' and a '## Design notes' section"));
                    continue;
                }

                foreach (var section in SpineSections)
                    if (!HasNonEmptySection(content, section))
                        end.ReportDiagnostic(Diagnostic.Create(Rule, location, module,
                            $"needs a non-empty '## {section}' section in {expected}"));
            }
        });
    }

    private static Dictionary<string, string> ReadContextFiles(ImmutableArray<AdditionalText> files, CancellationToken ct)
    {
        var byName = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
        foreach (var file in files)
        {
            if (!file.Path.EndsWith(".ctx.md", StringComparison.OrdinalIgnoreCase))
                continue;
            var text = file.GetText(ct)?.ToString();
            if (text is not null)
                byName[Path.GetFileName(file.Path)] = text;   // last one wins; module names are unique
        }

        return byName;
    }

    // A section is present and non-empty when its "## <heading>" line is followed by some content
    // before the next level-2 heading. A "### subsection" counts as content (it does not start "## ").
    private static bool HasNonEmptySection(string content, string heading)
    {
        var lines = content.Replace("\r\n", "\n").Split('\n');

        var i = 0;
        while (i < lines.Length && !lines[i].TrimStart().StartsWith("## " + heading, StringComparison.OrdinalIgnoreCase))
            i++;
        if (i >= lines.Length)
            return false;   // heading absent

        for (i++; i < lines.Length; i++)
        {
            if (lines[i].TrimStart().StartsWith("## ", StringComparison.Ordinal))
                return false;   // hit the next section with no content in between
            if (lines[i].Trim().Length > 0)
                return true;
        }

        return false;
    }

    // Matches [Slice] by simple name (no reference to Skies.Framework.Abstractions needed), like SKY0001.
    private static bool IsSlice(ClassDeclarationSyntax cls) =>
        cls.AttributeLists
            .SelectMany(list => list.Attributes)
            .Select(attr => attr.Name.ToString())
            .Any(n => n is "Slice" or "SliceAttribute"
                   || n.EndsWith(".Slice")
                   || n.EndsWith(".SliceAttribute"));
}
