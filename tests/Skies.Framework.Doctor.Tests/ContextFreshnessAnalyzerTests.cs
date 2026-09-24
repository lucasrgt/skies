using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp.Testing;
using Microsoft.CodeAnalysis.Testing;

namespace Skies.Framework.Doctor.Tests;

public class ContextFreshnessAnalyzerTests
{
    [Fact]
    public Task Citations_that_resolve_report_nothing() =>
        // `Login` is in source; `Guid` is in a referenced assembly — both resolve, so the ctx is fresh.
        Make("""
            # account

            ## Boundaries

            - in/out

            ## Design notes

            `Login` issues a token; ids are `Guid`.
            """).RunAsync();

    [Fact]
    public Task A_dangling_citation_is_flagged()
    {
        var test = Make("""
            # account

            ## Boundaries

            - x

            ## Design notes

            The `AttachCtx` hook is gone.
            """);
        test.TestState.ExpectedDiagnostics.Add(
            new DiagnosticResult(ContextFreshnessAnalyzer.DiagnosticId, DiagnosticSeverity.Error)
                .WithSpan("Account.ctx.md", 9, 6, 9, 15)
                .WithArguments("Account.ctx.md", "AttachCtx"));
        return test.RunAsync();
    }

    [Fact]
    public Task A_test_class_is_not_production_code_and_goes_stale()
    {
        // A ctx documents the module; a class that exists only in a test file is not part of it, even when the
        // project happens to feed that file to the doctor.
        var test = Make("""
            # account

            ## Boundaries

            - x

            ## Design notes

            Sign-up is covered by `PromotionFlow`.
            """);
        test.TestState.AdditionalFiles.Add(("PromotionFlow.Tests.cs", """
            namespace Demo.Tests;

            public class PromotionFlow { }
            """));
        test.TestState.ExpectedDiagnostics.Add(
            new DiagnosticResult(ContextFreshnessAnalyzer.DiagnosticId, DiagnosticSeverity.Error)
                .WithSpan("Account.ctx.md", 9, 24, 9, 37)
                .WithArguments("Account.ctx.md", "PromotionFlow"));
        return test.RunAsync();
    }

    [Fact]
    public Task A_cited_spec_and_failure_mode_that_exist_report_nothing()
    {
        // A spec citation starts with digits, so it is never read as a PascalCase code citation, and `Login` beside
        // it is still resolved as code.
        var test = Make(CtxCiting("`Login` refuses overdraw (`0002-withdraw#FM-2`), proven by `0002-withdraw`."));
        test.TestState.AdditionalFiles.Add((".specs/0002-withdraw/spec.md", WithdrawSpec));
        return test.RunAsync();
    }

    [Fact]
    public Task A_cited_spec_that_does_not_exist_is_flagged()
    {
        var test = Make(CtxCiting("Overdraw is refused (`0009-withdraw`)."));
        test.TestState.AdditionalFiles.Add((".specs/0002-withdraw/spec.md", WithdrawSpec));
        test.TestState.ExpectedDiagnostics.Add(SpecDiagnostic(9, 23, 9, 36, "0009-withdraw",
            "but there is no .specs/0009-withdraw/spec.md"));
        return test.RunAsync();
    }

    [Fact]
    public Task A_cited_failure_mode_the_spec_does_not_list_is_flagged()
    {
        // FM-9 appears in the spec's prose, but only a bullet under ## Failure modes declares a mode.
        var test = Make(CtxCiting("Overdraw is refused (`0002-withdraw#FM-9`)."));
        test.TestState.AdditionalFiles.Add((".specs/0002-withdraw/spec.md", WithdrawSpec));
        test.TestState.ExpectedDiagnostics.Add(SpecDiagnostic(9, 23, 9, 41, "0002-withdraw#FM-9",
            "but .specs/0002-withdraw/spec.md lists no FM-9"));
        return test.RunAsync();
    }

    [Fact]
    public Task Spec_citations_are_flagged_when_no_spec_is_fed_to_the_doctor()
    {
        var test = Make(CtxCiting("Overdraw is refused (`0002-withdraw#FM-2`)."));
        test.TestState.ExpectedDiagnostics.Add(SpecDiagnostic(9, 23, 9, 41, "0002-withdraw#FM-2",
            "but no spec.md reached the doctor; feed .specs/*/spec.md to it as AdditionalFiles"));
        return test.RunAsync();
    }

    [Fact]
    public Task Backticked_numbers_are_not_spec_citations()
    {
        // Calibration on a real ctx.md: an ISO week, a date, and a range are numbers in prose, not spec folders, so
        // they are never checked against the specs, even when no spec.md is fed at all.
        var test = Make(CtxCiting("Weeks are `YYYY-WW` (e.g. `2026-31`), dated `2024-01-15`, ranged `1-5`; `2xx` is success."));
        return test.RunAsync();
    }

    [Fact]
    public Task A_slug_that_starts_with_digits_is_still_a_spec_citation()
    {
        var test = Make(CtxCiting("Sign-in asks for a code (`0009-2fa-login`)."));
        test.TestState.ExpectedDiagnostics.Add(SpecDiagnostic(9, 27, 9, 41, "0009-2fa-login",
            "but no spec.md reached the doctor; feed .specs/*/spec.md to it as AdditionalFiles"));
        return test.RunAsync();
    }

    [Fact]
    public Task A_dangling_code_citation_beside_a_valid_spec_citation_is_still_flagged()
    {
        var test = Make(CtxCiting("`AttachCtx` is proven by `0002-withdraw#FM-1`."));
        test.TestState.AdditionalFiles.Add((".specs/0002-withdraw/spec.md", WithdrawSpec));
        test.TestState.ExpectedDiagnostics.Add(
            new DiagnosticResult(ContextFreshnessAnalyzer.DiagnosticId, DiagnosticSeverity.Error)
                .WithSpan("Account.ctx.md", 9, 2, 9, 11)
                .WithArguments("Account.ctx.md", "AttachCtx"));
        return test.RunAsync();
    }

    private static string CtxCiting(string note) => $"""
        # account

        ## Boundaries

        - x

        ## Design notes

        {note}
        """;

    private static DiagnosticResult SpecDiagnostic(int line, int column, int endLine, int endColumn, string cited, string problem) =>
        new DiagnosticResult(ContextFreshnessAnalyzer.DiagnosticId, DiagnosticSeverity.Error)
            .WithSpan("Account.ctx.md", line, column, endLine, endColumn)
            .WithArguments("Account.ctx.md", cited, problem);

    private const string WithdrawSpec = """
        ---
        id: "0002"
        runner: api
        ---
        # Withdraw

        Debits a wallet. FM-9 is mentioned here only in prose.

        ## Failure modes

        - FM-1 A valid withdrawal does not move the balance.
        - FM-2 Overdrawing is accepted.

        ## Out of scope

        - FM-9 is not a mode here either.
        """;

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
