using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp.Testing;
using Microsoft.CodeAnalysis.Testing;

namespace Skies.Framework.Doctor.Tests;

// The rule audit's SKY0005 grammar: which backtick spans are code citations (an identifier with a lowercase letter),
// and the one failure-mode grammar (`<spec>#FM-<n>` citations, `- FM-<n> …` spec lines) with look-alikes reported.
public class ContextCitationGrammarTests
{
    [Fact]
    public Task Acronyms_constants_and_quoted_literals_are_prose_not_citations() =>
        Make(CtxCiting("Calls `POST` with a `JWT` in `UTC`; `SKY0005` guards it, the scheme is `\"Bearer\"`, see `Q`."))
            .RunAsync();

    // A single capitalized word stays a citation: the real apps cite slices and enum members that way.
    [Fact]
    public Task A_single_word_that_no_longer_exists_is_still_stale()
    {
        var test = Make(CtxCiting("`Withdraw` refuses overdraw."));
        test.TestState.ExpectedDiagnostics.Add(
            new DiagnosticResult(ContextFreshnessAnalyzer.DiagnosticId, DiagnosticSeverity.Error)
                .WithSpan("Account.ctx.md", 9, 2, 9, 10)
                .WithArguments("Account.ctx.md", "Withdraw"));
        return test.RunAsync();
    }

    [Theory]
    [InlineData("fm-2")]
    [InlineData("FM2")]
    [InlineData("FM_2")]
    [InlineData("FM-[x]")]
    [InlineData("FM-")]
    [InlineData("2")]
    public Task A_look_alike_fragment_is_reported_with_the_grammar(string fragment)
    {
        var cited = $"0002-withdraw#{fragment}";
        var test = Make(CtxCiting($"Overdraw (`{cited}`)."));
        test.TestState.AdditionalFiles.Add((".specs/0002-withdraw/spec.md", Spec));
        test.TestState.ExpectedDiagnostics.Add(SpecDiagnostic(12, 12 + cited.Length, cited,
            $"but '#{fragment}' is not a failure mode; cite one as `0002-withdraw#FM-<n>` (FM, a dash, and the number, "
            + "as the spec's `- FM-<n>` line declares it)"));
        return test.RunAsync();
    }

    [Theory]
    [InlineData(3, "- FM 3 A second session reuses the token.")]
    [InlineData(4, "- fm_4 The refresh races the restore.")]
    [InlineData(5, "- FM5: A replay burns the family.")]
    [InlineData(6, "FM-6 The cookie outlives the session.")]
    public Task A_mode_declared_only_on_a_look_alike_line_names_the_line(int mode, string line)
    {
        var cited = $"0002-withdraw#FM-{mode}";
        var test = Make(CtxCiting($"Overdraw (`{cited}`)."));
        test.TestState.AdditionalFiles.Add((".specs/0002-withdraw/spec.md", Spec));
        test.TestState.ExpectedDiagnostics.Add(SpecDiagnostic(12, 12 + cited.Length, cited,
            $"but .specs/0002-withdraw/spec.md declares FM-{mode} only as '{line}', which is not a failure mode; "
            + $"write it as '- FM-{mode} <what goes wrong>' (a dash, FM, a hyphen, the number)"));
        return test.RunAsync();
    }

    // The engine's grammar (cli/src/proof/grammar.rs): a `-` or `*` bullet, an optional colon after the id.
    [Fact]
    public Task The_engine_grammar_declares_a_mode() =>
        WithSpec("`0002-withdraw#FM-1`, `0002-withdraw#FM-2`, `0002-withdraw#FM-7`, `0002-withdraw#FM-8` hold.")
            .RunAsync();

    private static CSharpAnalyzerTest<ContextFreshnessAnalyzer, DefaultVerifier> WithSpec(string note)
    {
        var test = Make(CtxCiting(note));
        test.TestState.AdditionalFiles.Add((".specs/0002-withdraw/spec.md", Spec));
        return test;
    }

    private const string Spec = """
        # Withdraw

        ## Failure modes

        - FM-1 A valid withdrawal does not move the balance.
        - FM-2 Overdrawing is accepted.
        - FM 3 A second session reuses the token.
        - fm_4 The refresh races the restore.
        - FM5: A replay burns the family.
        FM-6 The cookie outlives the session.
        * FM-7 A star bullet declares a mode.
          - FM-8: So does an indented one with a colon.
        """;

    private static string CtxCiting(string note) => $"""
        # account

        ## Boundaries

        - x

        ## Design notes

        {note}
        """;

    // The citation sits on line 9 after "Overdraw (`" (11 characters), so its text starts at column 12.
    private static DiagnosticResult SpecDiagnostic(int column, int endColumn, string cited, string problem) =>
        new DiagnosticResult(ContextFreshnessAnalyzer.DiagnosticId, DiagnosticSeverity.Error)
            .WithSpan("Account.ctx.md", 9, column, 9, endColumn)
            .WithArguments("Account.ctx.md", cited, problem);

    private const string Source = """
        namespace Demo.Modules.Account;

        [Slice]
        class Login { }

        sealed class SliceAttribute : System.Attribute { }
        """;

    private static CSharpAnalyzerTest<ContextFreshnessAnalyzer, DefaultVerifier> Make(string ctx) =>
        new()
        {
            ReferenceAssemblies = ReferenceAssemblies.Net.Net80,
            CompilerDiagnostics = CompilerDiagnostics.Errors,
            TestState =
            {
                Sources = { ("Login.cs", Source) },
                AdditionalFiles = { ("Account.ctx.md", ctx) },
            },
        };
}
