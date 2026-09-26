using System.Linq;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp.Testing;
using Microsoft.CodeAnalysis.Testing;

namespace Skies.Framework.Doctor.Tests;

public class FileSizeAnalyzerTests
{
    [Fact]
    public Task File_within_the_ceiling_reports_nothing() =>
        Harness<FileSizeAnalyzer>.Verify(Lines(100));

    [Fact]
    public Task File_over_the_ceiling_is_flagged()
    {
        var test = new CSharpAnalyzerTest<FileSizeAnalyzer, DefaultVerifier>
        {
            TestCode = Lines(600),
            ReferenceAssemblies = ReferenceAssemblies.Net.Net80,
            CompilerDiagnostics = CompilerDiagnostics.Errors,
            // A whole-file diagnostic is reported at the file's start, not at a token a
            // `#pragma warning disable` can wrap — so skip the harness's pragma-suppression pass.
            TestBehaviors = TestBehaviors.SkipSuppressionCheck,
        };
        test.TestState.ExpectedDiagnostics.Add(
            new DiagnosticResult(FileSizeAnalyzer.DiagnosticId, DiagnosticSeverity.Warning)
                .WithSpan(1, 1, 1, 1));
        return test.RunAsync();
    }

    [Fact]
    public Task An_ef_migration_is_exempt_however_large()
    {
        // Tool-emitted, append-only — its size is the schema's, not a packing smell.
        var test = new CSharpAnalyzerTest<FileSizeAnalyzer, DefaultVerifier>
        {
            ReferenceAssemblies = ReferenceAssemblies.Net.Net80,
            CompilerDiagnostics = CompilerDiagnostics.Errors,
            TestState = { Sources = { ("/src/App/Migrations/20260101000000_InitialCreate.cs", Lines(600)) } },
        };
        return test.RunAsync();
    }

    // A file of N comment-only lines: enough to cross the ceiling without declaring anything the
    // compiler would warn about.
    private static string Lines(int n) => string.Join("\n", Enumerable.Repeat("//", n));
}
