using Microsoft.CodeAnalysis.CSharp.Testing;
using Microsoft.CodeAnalysis.Testing;

namespace Skies.Framework.Doctor.Tests;

public class TenantFilterBypassAnalyzerTests
{
    // The shared harness, minus its blanket-pragma pass: that pass wraps each source in a reasonless
    // `#pragma warning disable`, which this rule reports on purpose (a hatch must say why).
    private static Task Verify(string markup) => new CSharpAnalyzerTest<TenantFilterBypassAnalyzer, DefaultVerifier>
    {
        TestCode = markup,
        ReferenceAssemblies = ReferenceAssemblies.Net.Net80,
        CompilerDiagnostics = CompilerDiagnostics.Errors,
        TestBehaviors = TestBehaviors.SkipSuppressionCheck,
    }.RunAsync();

    // The EF surface the rule matches by name, stubbed so the sources compile without the package.
    private const string Ef = """
        static class Ef
        {
            public static Q<T> IgnoreQueryFilters<T>(this Q<T> q) => q;
            public static Q<T> IgnoreQueryFilters<T>(this Q<T> q, System.Collections.Generic.IReadOnlyCollection<string> names) => q;
        }
        class Q<T> { }
        class SliceAttribute : System.Attribute { }
        static class Filters { public const string Tenant = "tenant"; }
        """;

    [Fact]
    public Task A_slice_lifting_every_filter_is_flagged() =>
        Verify(Ef + """
            namespace App.Api.Modules.Billing
            {
                static class ListInvoices
                {
                    static Q<int> Handle(Q<int> invoices) => invoices.{|SKY0030:IgnoreQueryFilters|}();
                }
            }
            """);

    [Fact]
    public Task Lifting_the_tenant_filter_by_name_is_flagged_in_a_marked_slice() =>
        Verify(Ef + """
            [Slice]
            static class ListInvoices
            {
                static Q<int> ByLiteral(Q<int> q) => q.{|SKY0030:IgnoreQueryFilters|}(["tenant"]);
                static Q<int> ByConstant(Q<int> q) => q.{|SKY0030:IgnoreQueryFilters|}([Filters.Tenant]);
            }
            """);

    [Fact]
    public Task Lifting_only_an_apps_own_named_filter_reports_nothing() =>
        Verify(Ef + """
            namespace App.Api.Modules.Billing
            {
                static class ListArchived
                {
                    static Q<int> Handle(Q<int> q) => q.IgnoreQueryFilters(["soft-delete"]);
                }
            }
            """);

    [Fact]
    public Task Code_outside_modules_reports_nothing() =>
        Verify(Ef + """
            namespace Specs.S0001
            {
                static class Seed
                {
                    static Q<int> All(Q<int> q) => q.IgnoreQueryFilters();
                }
            }
            """);

    [Fact]
    public Task A_co_located_test_file_reports_nothing() =>
        new CSharpAnalyzerTest<TenantFilterBypassAnalyzer, DefaultVerifier>
        {
            ReferenceAssemblies = ReferenceAssemblies.Net.Net80,
            CompilerDiagnostics = CompilerDiagnostics.Errors,
            TestBehaviors = TestBehaviors.SkipSuppressionCheck,
            TestState =
            {
                Sources =
                {
                    ("Modules/Billing/Slices/ListInvoices.Tests.cs", Ef + """
                        namespace App.Api.Modules.Billing
                        {
                            static class ListInvoicesTests
                            {
                                static Q<int> Everything(Q<int> q) => q.IgnoreQueryFilters();
                            }
                        }
                        """),
                },
            },
        }.RunAsync();

    [Fact]
    public Task A_reasoned_suppression_silences_the_crossing() =>
        Verify(Ef + """
            namespace App.Api.Modules.Account
            {
                static class Login
                {
            #pragma warning disable SKY0030 // sign-in looks the user up by email before any org is known
                    static Q<int> Handle(Q<int> users) => users.IgnoreQueryFilters();
            #pragma warning restore SKY0030
                }
            }
            """);

    [Fact]
    public Task A_suppression_without_a_reason_is_flagged() =>
        Verify(Ef + """
            namespace App.Api.Modules.Account
            {
                static class Login
                {
            {|SKY0030:#pragma warning disable SKY0030|}
                    static Q<int> Handle(Q<int> users) => users.IgnoreQueryFilters();
            #pragma warning restore SKY0030
                }
            }
            """);
}
