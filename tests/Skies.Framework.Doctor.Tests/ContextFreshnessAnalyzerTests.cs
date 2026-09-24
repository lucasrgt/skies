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
