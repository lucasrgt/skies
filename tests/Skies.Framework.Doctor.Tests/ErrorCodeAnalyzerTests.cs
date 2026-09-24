using Microsoft.CodeAnalysis;

namespace Skies.Framework.Doctor.Tests;

public class ErrorCodeAnalyzerTests
{
    [Fact]
    public Task Registry_constant_and_codeless_calls_report_nothing() =>
        Harness<ErrorCodeAnalyzer>.Verify(Code("""
            Error.NotFound(WalletsErrorCodes.NotFound, "not found");
            new Validation().Check(true, "amount", WalletsErrorCodes.NotFound, "bad");
            new Validation().Collect("amount", 0);
            Error.Validation(new List<int>());
            """));

    [Fact]
    public Task Inline_literal_in_a_factory_is_flagged() =>
        Harness<ErrorCodeAnalyzer>.Verify(Code("""
            Error.NotFound({|SKY0018:"wallets.not_found"|}, "not found");
            """));

    [Fact]
    public Task Inline_literal_in_a_check_is_flagged() =>
        Harness<ErrorCodeAnalyzer>.Verify(Code("""
            new Validation().Check(true, "amount", {|SKY0018:"amount.required"|}, "is required");
            """));

    [Fact]
    public Task Inline_literal_in_a_field_error_is_flagged() =>
        Harness<ErrorCodeAnalyzer>.Verify(Code("""
            var _ = new FieldError("amount", {|SKY0018:"amount.required"|}, "is required");
            """));

    [Fact]
    public Task A_constant_not_on_an_ErrorCodes_registry_is_flagged() =>
        Harness<ErrorCodeAnalyzer>.Verify(Code("""
            Error.NotFound({|SKY0018:Other.NotFound|}, "not found");
            """));

    [Fact]
    public Task A_code_parameter_on_an_unrelated_type_is_left_alone() =>
        Harness<ErrorCodeAnalyzer>.Verify(Code("""
            Unrelated.Log("anything");
            """));

    [Fact]
    public Task An_unused_error_code_constant_is_flagged() =>
        Harness<ErrorCodeAnalyzer>.Verify("""
            public static class WidgetsErrorCodes
            {
                public const string {|SKY0019:NotFound|} = "widgets.not_found";
            }
            """);

    [Fact]
    public void A_dead_code_is_a_warning_while_a_literal_code_stays_an_error()
    {
        var tiers = new ErrorCodeAnalyzer().SupportedDiagnostics.ToDictionary(d => d.Id, d => d.DefaultSeverity);
        Assert.Equal(DiagnosticSeverity.Error, tiers[ErrorCodeAnalyzer.DiagnosticId]);
        Assert.Equal(DiagnosticSeverity.Warning, tiers[ErrorCodeAnalyzer.DeadCodeDiagnosticId]);
    }

    [Fact]
    public Task A_referenced_error_code_constant_is_not_flagged() =>
        Harness<ErrorCodeAnalyzer>.Verify("""
            public static class WidgetsErrorCodes
            {
                public const string NotFound = "widgets.not_found";
            }

            public static class Use
            {
                public static readonly string Ref = WidgetsErrorCodes.NotFound;
            }
            """);

    // Stubs mirror the real shapes: Error factories + Validation.Check/Add + the FieldError record each declare a
    // 'code' parameter; Collect and the aggregate Error.Validation(fields) do not, so they are left alone. A code
    // is conformant only when it references a const on a class whose name ends with ErrorCodes.
    private static string Code(string body) => $$"""
        using System.Collections.Generic;

        public static class Subject
        {
            // Keeps WalletsErrorCodes.NotFound referenced so SKY0019 (dead-code) never fires in the SKY0018 cases.
            public static readonly string KeepAlive = WalletsErrorCodes.NotFound;

            public static void M()
            {
                {{body}}
            }
        }

        public static class WalletsErrorCodes { public const string NotFound = "wallets.not_found"; }

        // A const that is NOT on an *ErrorCodes class — referencing it must still be flagged.
        public static class Other { public const string NotFound = "other.not_found"; }

        // An unrelated type with a 'code' parameter — the rule is scoped to Error/Validation/FieldError, so this is left alone.
        public static class Unrelated { public static void Log(string code) { } }

        public sealed class Error
        {
            public static Error NotFound(string code, string message) => new();
            public static Error Validation(IReadOnlyList<int> fields) => new();
        }

        public sealed class Validation
        {
            public Validation Check(bool ok, string field, string code, string message) => this;
            public Validation Add(string field, string code, string message) => this;
            public Validation Collect(string field, int result) => this;
        }

        public readonly record struct FieldError(string Field, string Code, string Message);
        """;
}
