using Microsoft.CodeAnalysis.CSharp.Testing;
using Microsoft.CodeAnalysis.Testing;

namespace Skies.Framework.Doctor.Tests;

public class TestPlacementAnalyzerTests
{
    // Minimal stand-ins for the three test frameworks, so the attributes resolve the way they do in a real tests
    // project (the analyzer matches the resolved type and its bases, not only the written name).
    private const string Frameworks = """
        namespace Xunit
        {
            public class FactAttribute : System.Attribute { }
            public class TheoryAttribute : FactAttribute { }
            public class InlineDataAttribute : System.Attribute { public InlineDataAttribute(params object[] data) { } }
        }
        namespace NUnit.Framework
        {
            public class TestAttribute : System.Attribute { }
            public class TestCaseAttribute : System.Attribute { public TestCaseAttribute(params object[] args) { } }
        }
        namespace Microsoft.VisualStudio.TestTools.UnitTesting
        {
            public class TestMethodAttribute : System.Attribute { }
            public class DataTestMethodAttribute : TestMethodAttribute { }
        }
        namespace Custom
        {
            public class SkippableFactAttribute : Xunit.FactAttribute { }
        }
        """;

    private static Task Verify(params (string Path, string Source)[] files)
    {
        var test = new CSharpAnalyzerTest<TestPlacementAnalyzer, DefaultVerifier>
        {
            ReferenceAssemblies = ReferenceAssemblies.Net.Net80,
            CompilerDiagnostics = CompilerDiagnostics.Errors,
        };
        test.TestState.Sources.Add(("/repo/tests/App.Tests/Frameworks.cs", Frameworks));
        foreach (var file in files)
            test.TestState.Sources.Add(file);
        return test.RunAsync();
    }

    [Fact]
    public Task A_case_inside_a_spec_e2e_folder_reports_nothing() =>
        Verify(("/repo/.specs/0004-money/e2e/MoneySpec.cs", """
            namespace Specs.S0004;
            public class MoneySpec
            {
                [Xunit.Fact] public void Negative_amounts_are_rejected() { }
                [Xunit.Theory, Xunit.InlineData(1)] public void Amounts_round(int x) { _ = x; }
            }
            """));

    [Fact]
    public Task A_windows_spec_path_reports_nothing() =>
        Verify((@"C:\repo\.specs\0004-money\e2e\MoneySpec.cs", """
            public class MoneySpec { [Xunit.Fact] public void Case() { } }
            """));

    [Fact]
    public Task A_fact_in_the_tests_project_is_flagged() =>
        Verify(("/repo/tests/App.Tests/MoneyTests.cs", """
            using Xunit;
            public class MoneyTests
            {
                [Fact] public void {|SKY0029:Rejects_negative|}() { }
                [Theory, InlineData(2)] public void {|SKY0029:Rounds|}(int x) { _ = x; }
                public void Helper() { }
            }
            """));

    [Fact]
    public Task A_co_located_tests_file_is_flagged() =>
        Verify(("/repo/src/App.Api/BuildingBlocks/Money.Tests.cs", """
            public class MoneyTests { [Xunit.Fact] public void {|SKY0029:From_rejects_negative|}() { } }
            """));

    [Fact]
    public Task Nunit_and_mstest_tests_are_flagged() =>
        Verify(("/repo/tests/App.Tests/OtherFrameworks.cs", """
            using Microsoft.VisualStudio.TestTools.UnitTesting;
            public class NUnitStyle
            {
                [NUnit.Framework.Test] public void {|SKY0029:A|}() { }
                [NUnit.Framework.TestCase(1)] public void {|SKY0029:B|}(int x) { _ = x; }
            }
            public class MsTestStyle
            {
                [TestMethod] public void {|SKY0029:C|}() { }
                [DataTestMethod] public void {|SKY0029:D|}() { }
            }
            """));

    [Fact]
    public Task A_derived_custom_attribute_is_a_test() =>
        Verify(("/repo/tests/App.Tests/Skippable.cs", """
            public class Skippable { [Custom.SkippableFact] public void {|SKY0029:Maybe|}() { } }
            """));

    [Fact]
    public Task An_unrelated_attribute_that_resolves_is_not_a_test() =>
        Verify(("/repo/src/App.Api/Reports.cs", """
            namespace App;
            public class FactAttribute : System.Attribute { }
            public class Reports { [Fact] public void Summarize() { } [System.Obsolete] public void Old() { } }
            """));

    // A stray test compiled where xUnit is not referenced: the attribute does not resolve, so its written name decides,
    // and the doctor names the misplaced test beside the compiler's missing-type error.
    [Fact]
    public Task An_unresolved_test_attribute_is_judged_by_its_name()
    {
        var test = new CSharpAnalyzerTest<TestPlacementAnalyzer, DefaultVerifier>
        {
            ReferenceAssemblies = ReferenceAssemblies.Net.Net80,
            CompilerDiagnostics = CompilerDiagnostics.None,
        };
        test.TestState.Sources.Add(("/repo/src/App.Api/Money.Tests.cs", """
            public class MoneyTests
            {
                [Fact] public void {|SKY0029:A|}() { }
                [SkippableTheory] public void {|SKY0029:B|}() { }
                [Unrelated] public void C() { }
            }
            """));
        return test.RunAsync();
    }
}
