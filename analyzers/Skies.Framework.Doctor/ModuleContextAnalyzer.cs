using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Collections.Immutable;
using System.IO;
using System.Linq;
using System.Text;
using System.Threading;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp;
using Microsoft.CodeAnalysis.CSharp.Syntax;
using Microsoft.CodeAnalysis.Diagnostics;
using Microsoft.CodeAnalysis.Text;

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
/// proves the spine sections exist and carry the author's own words (not blank lines, HTML comments, or the
/// hints <c>skies g module</c> writes), not that the content is correct; freshness (a ctx that names code
/// which no longer exists) is <c>SKY0005</c>'s job. The generator writes the skeleton with its hints
/// commented out, so the build asks for the module's why as soon as the module owns a slice.
///
/// The finding sits where the fix goes: a missing or empty section is reported in the <c>.ctx.md</c> itself (at the
/// section's heading, or the first line when the heading is absent), and a missing ctx on the module's
/// <c>[Module]</c> class. Only a module with no <c>[Module]</c> class falls back to a slice, the first one by file
/// path, so the location is stable from build to build.
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
        description: "Every module that owns a slice must carry a <Module>.ctx.md whose ## Boundaries and "
                   + "## Design notes sections are written: a section that is empty, holds only HTML comments, or "
                   + "holds the scaffold's hints does not count. The ctx is the home for the business why the "
                   + "code cannot show.",
        customTags: WellKnownDiagnosticTags.CompilationEnd);

    /// <summary>The spine sections every module ctx must carry, non-empty.</summary>
    private static readonly string[] SpineSections = ["Boundaries", "Design notes"];

    /// <summary>
    /// The hints <c>skies g module</c> writes, as HTML comments, into a fresh ctx
    /// (<c>cli/templates/dotnet/scaffold/Module.ctx.md.cstmpl</c>). They say what to write, not what the module
    /// is, so a section that holds one, commented out or not, has not been written yet.
    /// </summary>
    public static readonly ImmutableArray<string> ScaffoldHints = ImmutableArray.Create(
        "What this module is for, in the business's words",
        "Inside: the data and rules this module owns",
        "Outside: what this module leaves to other modules",
        "Non-goals: what this module deliberately does not do",
        "The non-obvious invariants of this module and why they hold");

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
        // Every slice location per module (the module owns a slice), and the module's [Module] class when it has one.
        var slices = new ConcurrentDictionary<string, ConcurrentBag<Location>>();
        var roots = new ConcurrentDictionary<string, Location>();

        context.RegisterSyntaxNodeAction(syntax =>
        {
            var cls = (ClassDeclarationSyntax)syntax.Node;
            var module = ModuleNaming.ModuleOf(cls);
            if (module is null)
                return;
            if (HasAttribute(cls, "Slice"))
                slices.GetOrAdd(module, _ => new ConcurrentBag<Location>()).Add(cls.Identifier.GetLocation());
            else if (HasAttribute(cls, "Module"))
                roots.TryAdd(module, cls.Identifier.GetLocation());
        }, SyntaxKind.ClassDeclaration);

        context.RegisterCompilationEndAction(end =>
        {
            if (slices.IsEmpty)
                return;

            var ctxByName = ReadContextFiles(end.Options.AdditionalFiles, end.CancellationToken);

            foreach (var entry in slices)
            {
                var module = entry.Key;
                var expected = module + ".ctx.md";
                if (!ctxByName.TryGetValue(expected, out var ctx))
                {
                    var location = roots.TryGetValue(module, out var root) ? root : First(entry.Value);
                    end.ReportDiagnostic(Diagnostic.Create(Rule, location, module,
                        $"has no {expected} beside it; add one with a '## Boundaries' and a '## Design notes' section"));
                    continue;
                }

                var content = ctx.Text.ToString();
                foreach (var section in SpineSections)
                {
                    var ask = $"write the module's {section.ToLowerInvariant()}";
                    switch (Section(content, section))
                    {
                        case SectionState.Absent:
                            end.ReportDiagnostic(Diagnostic.Create(Rule, InCtx(ctx, 0), module,
                                $"needs a '## {section}' section in {expected}: {ask}"));
                            break;
                        case SectionState.Empty:
                            end.ReportDiagnostic(Diagnostic.Create(Rule, InCtx(ctx, HeadingLine(content, section)),
                                module, $"has an empty '## {section}' section in {expected} (HTML comments and the "
                                + $"scaffold's hints do not count): {ask}"));
                            break;
                    }
                }
            }
        });
    }

    private static Dictionary<string, (string Path, SourceText Text)> ReadContextFiles(
        ImmutableArray<AdditionalText> files, CancellationToken ct)
    {
        var byName = new Dictionary<string, (string, SourceText)>(StringComparer.OrdinalIgnoreCase);
        foreach (var file in files)
        {
            if (!file.Path.EndsWith(".ctx.md", StringComparison.OrdinalIgnoreCase))
                continue;
            var text = file.GetText(ct);
            if (text is not null)
                byName[Path.GetFileName(file.Path)] = (file.Path, text);   // last one wins; module names are unique
        }

        return byName;
    }

    // The whole line `line` (0-based) of the ctx, as a location in that file.
    private static Location InCtx((string Path, SourceText Text) ctx, int line)
    {
        var span = ctx.Text.Lines[Math.Min(line, ctx.Text.Lines.Count - 1)].Span;
        return Location.Create(ctx.Path, span, ctx.Text.Lines.GetLinePositionSpan(span));
    }

    // The 0-based line of a section's heading (0 when it cannot be found).
    private static int HeadingLine(string content, string heading)
    {
        var lines = content.Replace("\r\n", "\n").Split('\n');
        for (var i = 0; i < lines.Length; i++)
            if (lines[i].TrimStart().StartsWith("## " + heading, StringComparison.OrdinalIgnoreCase))
                return i;
        return 0;
    }

    // The first slice by file path, then position: a stable place to report a missing ctx when no [Module] exists.
    private static Location First(IEnumerable<Location> locations) =>
        locations
            .OrderBy(l => l.SourceTree?.FilePath, StringComparer.Ordinal)
            .ThenBy(l => l.SourceSpan.Start)
            .First();

    private enum SectionState
    {
        Absent,
        Empty,
        Written,
    }

    // A section is written when its "## <heading>" line is followed, before the next level-2 heading, by a line
    // of the author's own: not blank, not inside an HTML comment, and not one of the scaffold's hints. A "###
    // subsection" counts as content (it does not start "## "). A section holding a hint the author uncommented is
    // still empty: the hint describes what to write, not the module.
    private static SectionState Section(string content, string heading)
    {
        var lines = StripComments(content.Replace("\r\n", "\n")).Split('\n');

        var i = 0;
        while (i < lines.Length && !lines[i].TrimStart().StartsWith("## " + heading, StringComparison.OrdinalIgnoreCase))
            i++;
        if (i >= lines.Length)
            return SectionState.Absent;

        var written = false;
        for (i++; i < lines.Length; i++)
        {
            var line = lines[i].Trim();
            if (line.StartsWith("## ", StringComparison.Ordinal))
                break;
            if (IsHint(line))
                return SectionState.Empty;
            if (line.Length > 0)
                written = true;
        }

        return written ? SectionState.Written : SectionState.Empty;
    }

    // Removes every <!-- ... --> span, keeping its line breaks so the lines around it stay where they were. An
    // unclosed comment runs to the end, as it renders.
    private static string StripComments(string content)
    {
        var text = new StringBuilder(content.Length);
        var at = 0;
        while (at < content.Length)
        {
            var open = content.IndexOf("<!--", at, StringComparison.Ordinal);
            if (open < 0)
            {
                text.Append(content, at, content.Length - at);
                break;
            }

            text.Append(content, at, open - at);
            var close = content.IndexOf("-->", open + 4, StringComparison.Ordinal);
            var end = close < 0 ? content.Length : close + 3;
            text.Append('\n', content.Substring(open, end - open).Count(c => c == '\n'));
            at = end;
        }

        return text.ToString();
    }

    private static bool IsHint(string line) =>
        ScaffoldHints.Any(hint => line.IndexOf(hint, StringComparison.OrdinalIgnoreCase) >= 0);

    // Matches [Slice] / [Module] by simple name (no reference to Skies.Framework.Abstractions needed), like SKY0001.
    private static bool HasAttribute(ClassDeclarationSyntax cls, string name) =>
        cls.AttributeLists
            .SelectMany(list => list.Attributes)
            .Select(attr => attr.Name.ToString())
            .Any(n => n == name || n == name + "Attribute"
                   || n.EndsWith("." + name, StringComparison.Ordinal)
                   || n.EndsWith("." + name + "Attribute", StringComparison.Ordinal));
}
