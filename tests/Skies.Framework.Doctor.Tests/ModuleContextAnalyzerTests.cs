using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp.Testing;
using Microsoft.CodeAnalysis.Testing;

namespace Skies.Framework.Doctor.Tests;

public class ModuleContextAnalyzerTests
{
    [Fact]
    public Task Module_with_a_conformant_ctx_reports_nothing() =>
        Make("Account.ctx.md", GoodCtx).RunAsync();

    [Fact]
    public Task Module_with_no_ctx_is_flagged()
    {
        var test = Make("README.md", "not a ctx file");
        test.TestState.ExpectedDiagnostics.Add(
            new DiagnosticResult(ModuleContextAnalyzer.DiagnosticId, DiagnosticSeverity.Error)
                .WithSpan("Login.cs", 4, 7, 4, 12)
                .WithArguments("Account",
                    "has no Account.ctx.md beside it; add one with a '## Boundaries' and a '## Design notes' section"));
        return test.RunAsync();
    }

    [Fact]
    public Task Ctx_missing_boundaries_is_flagged()
    {
        var test = Make("Account.ctx.md", DesignOnlyCtx);
        test.TestState.ExpectedDiagnostics.Add(Absent(DesignOnlyCtx, "Boundaries"));
        return test.RunAsync();
    }

    [Fact]
    public Task Ctx_with_an_empty_design_notes_is_flagged()
    {
        var test = Make("Account.ctx.md", EmptyDesignCtx);
        test.TestState.ExpectedDiagnostics.Add(Empty(EmptyDesignCtx, "Design notes"));
        return test.RunAsync();
    }

    // The audit's misplaced finding: a missing ctx was reported on whichever slice the analyzer met first (an
    // unrelated `DeleteProduct.cs:10`). It belongs on the module's [Module] class, where the ctx sits beside.
    [Fact]
    public Task A_missing_ctx_is_reported_on_the_module_class_not_a_slice()
    {
        var test = Make("README.md", "not a ctx file");
        test.TestState.Sources.Add(("DeleteProduct.cs", """
            namespace Demo.Modules.Account;

            [Slice]
            class DeleteProduct { }
            """));
        test.TestState.Sources.Add(("AccountModule.cs", """
            namespace Demo.Modules.Account;

            [Module]
            static class AccountModule { }

            sealed class ModuleAttribute : System.Attribute { }
            """));
        test.TestState.ExpectedDiagnostics.Add(
            new DiagnosticResult(ModuleContextAnalyzer.DiagnosticId, DiagnosticSeverity.Error)
                .WithSpan("AccountModule.cs", 4, 14, 4, 27)
                .WithArguments("Account",
                    "has no Account.ctx.md beside it; add one with a '## Boundaries' and a '## Design notes' section"));
        return test.RunAsync();
    }

    [Fact]
    public Task Sections_holding_only_html_comments_are_empty()
    {
        const string ctx = """
            # account

            ## Boundaries

            <!-- Inside: identity. -->
            <!--
              Outside: payments.
            -->

            ## Design notes
            <!-- A rule. -->
            """;
        var test = Make("Account.ctx.md", ctx);
        test.TestState.ExpectedDiagnostics.Add(Empty(ctx, "Boundaries"));
        test.TestState.ExpectedDiagnostics.Add(Empty(ctx, "Design notes"));
        return test.RunAsync();
    }

    [Fact]
    public Task An_uncommented_scaffold_hint_does_not_count_as_written()
    {
        const string ctx = """
            # account

            ## Boundaries

            - Inside: the data and rules this module owns and is the only one allowed to write.

            ## Design notes

            ### Sessions rotate
            A refresh burns the family on reuse, so a stolen token dies with its first replay.
            """;
        var test = Make("Account.ctx.md", ctx);
        test.TestState.ExpectedDiagnostics.Add(Empty(ctx, "Boundaries"));
        return test.RunAsync();
    }

    [Fact]
    public Task Written_sections_pass_beside_leftover_comments() =>
        Make("Account.ctx.md", """
            # account

            ## Boundaries

            <!-- Outside: what this module leaves to other modules, referenced by id. -->
            - Inside: identity and sessions.

            ## Design notes

            A refresh burns the family on reuse. <!-- cite the spec -->
            """).RunAsync();

    [Fact]
    public Task The_skeleton_g_module_writes_fails_until_the_author_writes_it()
    {
        var template = File.ReadAllText(Path.Combine(RepositoryRoot(), "cli", "templates", "dotnet", "scaffold",
            "Module.ctx.md.cstmpl"));
        var ctx = template.Replace("__NAME_LOWER__", "account").Replace("__NAME__", "Account");
        var test = Make("Account.ctx.md", ctx);
        test.TestState.ExpectedDiagnostics.Add(Empty(ctx, "Boundaries"));
        test.TestState.ExpectedDiagnostics.Add(Empty(ctx, "Design notes"));
        return test.RunAsync();
    }

    [Fact]
    public void Every_hint_the_skeleton_writes_is_known_to_the_rule()
    {
        var template = File.ReadAllText(Path.Combine(RepositoryRoot(), "cli", "templates", "dotnet", "scaffold",
            "Module.ctx.md.cstmpl"));
        var comments = template.Split('\n').Where(line => line.TrimStart().StartsWith("<!--")).ToList();
        Assert.NotEmpty(comments);
        Assert.All(comments, line =>
            Assert.Contains(ModuleContextAnalyzer.ScaffoldHints, hint => line.Contains(hint, StringComparison.Ordinal)));
    }

    private static string RepositoryRoot()
    {
        var dir = new DirectoryInfo(AppContext.BaseDirectory);
        while (dir is not null && !File.Exists(Path.Combine(dir.FullName, "Skies.Framework.slnx")))
            dir = dir.Parent;
        return dir?.FullName ?? throw new InvalidOperationException("the repository root is not above the tests");
    }

    // A section finding sits in the ctx: on the section's heading when it is there, else on the first line.
    private static DiagnosticResult Absent(string ctx, string section) =>
        InCtx(ctx, 0, $"needs a '## {section}' section in Account.ctx.md: write the module's "
                      + section.ToLowerInvariant());

    private static DiagnosticResult Empty(string ctx, string section) =>
        InCtx(ctx, Array.FindIndex(Lines(ctx), line => line.TrimStart().StartsWith("## " + section, StringComparison.Ordinal)),
            $"has an empty '## {section}' section in Account.ctx.md (HTML comments and the scaffold's "
            + $"hints do not count): write the module's {section.ToLowerInvariant()}");

    private static DiagnosticResult InCtx(string ctx, int line, string message) =>
        new DiagnosticResult(ModuleContextAnalyzer.DiagnosticId, DiagnosticSeverity.Error)
            .WithSpan("Account.ctx.md", line + 1, 1, line + 1, Lines(ctx)[line].Length + 1)
            .WithArguments("Account", message);

    private static string[] Lines(string text) => text.Replace("\r\n", "\n").Split('\n');

    private const string Slice = """
        namespace Demo.Modules.Account;

        [Slice]
        class Login { }

        sealed class SliceAttribute : System.Attribute { }
        """;

    private const string GoodCtx = """
        # account

        ## Boundaries

        - Inside: identity
        - Outside: payments live elsewhere

        ## Design notes

        ### A rule
        Why it holds.
        """;

    private const string DesignOnlyCtx = """
        # account

        ## Design notes

        ### A rule
        Why it holds.
        """;

    private const string EmptyDesignCtx = """
        # account

        ## Boundaries

        - Inside: identity

        ## Design notes
        """;

    private static CSharpAnalyzerTest<ModuleContextAnalyzer, DefaultVerifier> Make(string ctxName, string ctx) =>
        new()
        {
            ReferenceAssemblies = ReferenceAssemblies.Net.Net80,
            CompilerDiagnostics = CompilerDiagnostics.Errors,
            TestState =
            {
                Sources = { ("Login.cs", Slice) },
                AdditionalFiles = { (ctxName, ctx) },
            },
        };
}
