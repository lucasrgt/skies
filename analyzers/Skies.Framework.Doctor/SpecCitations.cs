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
/// folder name and its failure modes are the lines under <c>## Failure modes</c> that start <c>- FM-&lt;n&gt; </c>, the
/// one grammar <c>skies proof</c> reads. A citation starts with digits, so it can never be mistaken for a PascalCase
/// code citation, and a code citation can never be mistaken for it.
///
/// The grammar is exact on both sides, and a look-alike is reported rather than silently skipped: a citation whose
/// fragment is not <c>FM-&lt;n&gt;</c> (a lowercase <c>fm-n</c>, a missing dash, a zero-padded number) is malformed,
/// and a cited mode that the spec lists only on a look-alike line (a <c>*</c> bullet, a missing dash, emphasis, a
/// colon after the id) is named with the line to rewrite.
/// </summary>
internal static class SpecCitations
{
    // `<digits>-<kebab slug>`, optionally `#<fragment>`: the whole backtick span, nothing else inside it. The slug holds
    // at least one letter: an all-digit span (`2026-31`, an ISO week; `2024-01-15`, a date; `1-5`, a range) is a number
    // in prose, not a spec folder, and calibrating on a real app's ctx.md found exactly that. The fragment is validated
    // apart (see FailureModeFragment), so a look-alike is reported instead of read as prose.
    private static readonly Regex CitationPattern = new(
        @"`(?<spec>[0-9]+-(?=[a-z0-9-]*[a-z])[a-z0-9]+(?:-[a-z0-9]+)*)(?:#(?<fragment>[^`\s]*))?`", RegexOptions.Compiled);

    // The one fragment a citation may carry: `FM-<n>`, n a positive number without a leading zero.
    private static readonly Regex FailureModeFragment = new(@"^FM-(?<n>[1-9][0-9]*)$", RegexOptions.Compiled);

    // The one failure-mode line: `- FM-<n> text`, at the start of the line, the id followed by a space or the end.
    private static readonly Regex FailureModeLine = new(@"^- FM-(?<n>[1-9][0-9]*)(?:[ \t]|$)", RegexOptions.Compiled);

    // A line that means a failure mode but is not in the grammar: another bullet, no dash in the id, emphasis, a
    // colon, indentation. It names the number it was meant to declare.
    private static readonly Regex LookAlikeLine = new(
        @"^\s*[-*+]\s*[*_]{0,2}FM[- ]?0*(?<n>[1-9][0-9]*)", RegexOptions.Compiled | RegexOptions.IgnoreCase);

    /// <summary>One spec citation in a ctx: the cited folder, the failure mode (if any), and its span.</summary>
    internal readonly struct Citation
    {
        public Citation(string spec, int? failureMode, TextSpan span, string? malformed = null)
        {
            Spec = spec;
            FailureMode = failureMode;
            Span = span;
            Malformed = malformed;
        }

        /// <summary>The cited spec folder name, e.g. <c>0002-withdraw</c>.</summary>
        public string Spec { get; }

        /// <summary>The cited failure mode number, or <see langword="null"/> for a whole-spec citation.</summary>
        public int? FailureMode { get; }

        /// <summary>The span of the citation text inside the backticks.</summary>
        public TextSpan Span { get; }

        /// <summary>The fragment as written when it is not <c>FM-&lt;n&gt;</c>, else <see langword="null"/>.</summary>
        public string? Malformed { get; }

        /// <summary>The citation as written, without the backticks.</summary>
        public string Text => Malformed is { } fragment ? $"{Spec}#{fragment}"
            : FailureMode is { } fm ? $"{Spec}#FM-{fm}" : Spec;
    }

    /// <summary>A spec's failure modes, and the numbers its look-alike lines meant to declare, with those lines.</summary>
    internal sealed class Spec
    {
        /// <summary>The failure modes the spec declares in the grammar.</summary>
        public HashSet<int> Modes { get; } = new();

        /// <summary>A mode number → the first look-alike line that meant it.</summary>
        public Dictionary<int, string> LookAlikes { get; } = new();
    }

    /// <summary>Every spec citation in a ctx, in document order.</summary>
    public static List<Citation> In(SourceText text)
    {
        var found = new List<Citation>();
        foreach (Match match in CitationPattern.Matches(text.ToString()))
        {
            var span = new TextSpan(match.Index + 1, match.Length - 2);
            var spec = match.Groups["spec"].Value;
            var fragment = match.Groups["fragment"];
            if (!fragment.Success)
            {
                found.Add(new Citation(spec, null, span));
                continue;
            }

            var fm = FailureModeFragment.Match(fragment.Value);
            found.Add(fm.Success && int.TryParse(fm.Groups["n"].Value, out var number)
                ? new Citation(spec, number, span)
                : new Citation(spec, null, span, fragment.Value));
        }

        return found;
    }

    /// <summary>
    /// The specs the project fed the doctor: folder name → the failure mode numbers its spec.md lists. Only a
    /// <c>spec.md</c> directly inside <c>.specs/&lt;folder&gt;/</c> counts, so a stray spec.md elsewhere is ignored.
    /// </summary>
    public static Dictionary<string, Spec> Index(ImmutableArray<AdditionalText> files, CancellationToken ct)
    {
        var specs = new Dictionary<string, Spec>(StringComparer.Ordinal);
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
    public static string? Problem(Citation citation, Dictionary<string, Spec> specs)
    {
        if (citation.Malformed is { } fragment)
            return $"but '#{fragment}' is not a failure mode; cite one as `{citation.Spec}#FM-<n>` (FM, a dash, and "
                   + "the number, as the spec's `- FM-<n>` line declares it)";
        if (specs.Count == 0)
            return "but no spec.md reached the doctor; feed .specs/*/spec.md to it as AdditionalFiles";
        if (!specs.TryGetValue(citation.Spec, out var spec))
            return $"but there is no .specs/{citation.Spec}/spec.md";
        if (citation.FailureMode is { } fm && !spec.Modes.Contains(fm))
            return spec.LookAlikes.TryGetValue(fm, out var line)
                ? $"but .specs/{citation.Spec}/spec.md declares FM-{fm} only as '{line}', which is not a failure "
                  + $"mode; write it as '- FM-{fm} …'"
                : $"but .specs/{citation.Spec}/spec.md lists no FM-{fm}";
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

    // The failure mode numbers under `## Failure modes`, where `skies proof` reads them, and the look-alikes.
    private static Spec FailureModes(SourceText text)
    {
        var spec = new Spec();
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
                spec.Modes.Add(number);
            else if (LookAlikeLine.Match(line) is { Success: true } lookAlike
                     && int.TryParse(lookAlike.Groups["n"].Value, out var meant)
                     && !spec.LookAlikes.ContainsKey(meant))
                spec.LookAlikes[meant] = line.Trim();
        }

        return spec;
    }
}
