using System;
using System.Collections.Generic;
using System.Collections.Immutable;
using System.Linq;
using System.Text.RegularExpressions;
using System.Threading;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.Text;

namespace Skies.Framework.Doctor;

/// <summary>
/// The spec half of <c>SKY0005</c>. A ctx design note that states an invariant cites the spec proving it, as the
/// backticked spec folder name with an optional failure mode: <c>`0002-withdraw`</c> or <c>`0002-withdraw#FM-n`</c>.
/// The citation ties the prose to evidence, so it goes stale the moment the spec is renamed, removed, or loses that
/// failure mode, exactly like a code citation goes stale when the type is renamed.
///
/// The specs arrive as <c>AdditionalFiles</c> (<c>.specs/&lt;id&gt;-&lt;slug&gt;/spec.md</c>); a spec is known by its
/// folder name and its failure modes are the <c>- FM-n</c> bullets under <c>## Failure modes</c>, the same lines
/// <c>skies proof</c> reads. A citation starts with digits, so it can never be mistaken for a PascalCase code
/// citation, and a code citation can never be mistaken for it.
/// </summary>
internal static class SpecCitations
{
    // `<digits>-<kebab slug>`, optionally `#FM-<n>`: the whole backtick span, nothing else inside it. The slug holds at
    // least one letter: an all-digit span (`2026-31`, an ISO week; `2024-01-15`, a date; `1-5`, a range) is a number
    // in prose, not a spec folder, and calibrating on a real app's ctx.md found exactly that.
    private static readonly Regex CitationPattern = new(
        @"`(?<spec>[0-9]+-(?=[a-z0-9-]*[a-z])[a-z0-9]+(?:-[a-z0-9]+)*)(?:#FM-(?<fm>[0-9]+))?`", RegexOptions.Compiled);

    // A failure-mode bullet: `- FM-n text` or `* FM-n: text`; the id must lead the bullet.
    private static readonly Regex FailureModeLine = new(@"^\s*[-*]\s+FM-(?<n>[0-9]+)(?::|\s|$)", RegexOptions.Compiled);

    /// <summary>One spec citation in a ctx: the cited folder, the failure mode (if any), and its span.</summary>
    internal readonly struct Citation
    {
        public Citation(string spec, int? failureMode, TextSpan span)
        {
            Spec = spec;
            FailureMode = failureMode;
            Span = span;
        }

        /// <summary>The cited spec folder name, e.g. <c>0002-withdraw</c>.</summary>
        public string Spec { get; }

        /// <summary>The cited failure mode number, or <see langword="null"/> for a whole-spec citation.</summary>
        public int? FailureMode { get; }

        /// <summary>The span of the citation text inside the backticks.</summary>
        public TextSpan Span { get; }

        /// <summary>The citation as written, without the backticks.</summary>
        public string Text => FailureMode is { } fm ? $"{Spec}#FM-{fm}" : Spec;
    }

    /// <summary>Every spec citation in a ctx, in document order.</summary>
    public static List<Citation> In(SourceText text)
    {
        var found = new List<Citation>();
        foreach (Match match in CitationPattern.Matches(text.ToString()))
        {
            var fm = match.Groups["fm"];
            int? mode = fm.Success && int.TryParse(fm.Value, out var number) ? number : null;
            found.Add(new Citation(match.Groups["spec"].Value, mode,
                new TextSpan(match.Index + 1, match.Length - 2)));
        }

        return found;
    }

    /// <summary>
    /// The specs the project fed the doctor: folder name → the failure mode numbers its spec.md lists. Only a
    /// <c>spec.md</c> directly inside <c>.specs/&lt;folder&gt;/</c> counts, so a stray spec.md elsewhere is ignored.
    /// </summary>
    public static Dictionary<string, HashSet<int>> Index(ImmutableArray<AdditionalText> files, CancellationToken ct)
    {
        var specs = new Dictionary<string, HashSet<int>>(StringComparer.Ordinal);
        foreach (var file in files)
        {
            var folder = SpecFolder(file.Path);
            if (folder is null)
                continue;
            var text = file.GetText(ct);
            if (text is not null)
                specs[folder] = FailureModes(text);
        }

        return specs;
    }

    /// <summary>Why a citation does not resolve against the index, or <see langword="null"/> when it does.</summary>
    public static string? Problem(Citation citation, Dictionary<string, HashSet<int>> specs)
    {
        if (specs.Count == 0)
            return "but no spec.md reached the doctor; feed .specs/*/spec.md to it as AdditionalFiles";
        if (!specs.TryGetValue(citation.Spec, out var modes))
            return $"but there is no .specs/{citation.Spec}/spec.md";
        if (citation.FailureMode is { } fm && !modes.Contains(fm))
            return $"but .specs/{citation.Spec}/spec.md lists no FM-{fm}";
        return null;
    }

    // `…/.specs/<folder>/spec.md` → `<folder>`; any other path → null. Splits on both separators so an MSBuild
    // item written with backslashes resolves on every OS.
    private static string? SpecFolder(string path)
    {
        var parts = path.Split('/', '\\');
        var n = parts.Length;
        if (n < 3 || !parts[n - 1].Equals("spec.md", StringComparison.OrdinalIgnoreCase))
            return null;
        return parts[n - 3] == ".specs" ? parts[n - 2] : null;
    }

    // The failure mode numbers under `## Failure modes`, where `skies proof` reads them.
    private static HashSet<int> FailureModes(SourceText text)
    {
        var modes = new HashSet<int>();
        var inSection = false;
        foreach (var line in text.Lines.Select(l => l.ToString()))
        {
            if (line.StartsWith("## ", StringComparison.Ordinal))
            {
                inSection = line.Substring(3).Trim().Equals("Failure modes", StringComparison.OrdinalIgnoreCase);
                continue;
            }

            if (!inSection)
                continue;
            var match = FailureModeLine.Match(line);
            if (match.Success && int.TryParse(match.Groups["n"].Value, out var number))
                modes.Add(number);
        }

        return modes;
    }
}
