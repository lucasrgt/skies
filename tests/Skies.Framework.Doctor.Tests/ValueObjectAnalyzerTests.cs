namespace Skies.Framework.Doctor.Tests;

public class ValueObjectAnalyzerTests
{
    [Fact]
    public Task Encapsulated_value_object_reports_nothing() =>
        Harness<ValueObjectAnalyzer>.Verify(Valid);

    [Fact]
    public Task Value_object_with_a_public_setter_is_flagged() =>
        Harness<ValueObjectAnalyzer>.Verify(PublicSetter);

    [Fact]
    public Task Value_object_with_a_public_constructor_is_flagged() =>
        Harness<ValueObjectAnalyzer>.Verify(PublicConstructor);

    [Fact]
    public Task Value_object_without_a_smart_constructor_is_flagged() =>
        Harness<ValueObjectAnalyzer>.Verify(NoSmartConstructor);

    [Fact]
    public Task Positional_value_object_is_flagged() =>
        Harness<ValueObjectAnalyzer>.Verify(Positional);

    [Fact]
    public Task Public_init_allows_copying_a_record_with_invalid_state() =>
        Harness<ValueObjectAnalyzer>.Verify(PublicSetter
            .Replace("sealed class Email", "sealed record Email")
            .Replace("get; set;", "get; init;"));

    [Fact]
    public Task Private_init_keeps_record_copies_encapsulated() =>
        Harness<ValueObjectAnalyzer>.Verify(PublicSetter
            .Replace("{|SKY0013:Value|}", "Value")
            .Replace("get; set;", "get; private init;"));

    [Fact]
    public Task Implicit_public_construction_cannot_skip_the_factory() =>
        Harness<ValueObjectAnalyzer>.Verify("""
            using System;
            [ValueObject]
            public class {|SKY0013:Email|}
            {
                public static Result<Email> From(string input) => new Result<Email>();
            }
            public sealed class ValueObjectAttribute : Attribute { }
            public struct Result<T> { }
            """);

    [Theory]
    [InlineData("internal")]
    [InlineData("protected")]
    [InlineData("protected internal")]
    public Task Setters_accessible_outside_the_type_are_flagged(string accessibility) =>
        Harness<ValueObjectAnalyzer>.Verify(PublicSetter
            .Replace("sealed class", "class")
            .Replace("get; set;", $"get; {accessibility} set;"));

    // The Money shape: immutable, private ctor, a static From returning Result<Money> — nothing to flag.
    private const string Valid = """
        using System;

        [ValueObject]
        public readonly struct Money
        {
            public decimal Amount { get; }
            private Money(decimal amount) => Amount = amount;
            public static Result<Money> From(decimal amount) => new Result<Money>();
        }

        public sealed class ValueObjectAttribute : Attribute { }
        public struct Result<T> { }
        """;

    private const string PublicSetter = """
        using System;

        [ValueObject]
        public sealed class Email
        {
            public string {|SKY0013:Value|} { get; set; }
            private Email(string value) => Value = value;
            public static Result<Email> From(string value) => new Result<Email>();
        }

        public sealed class ValueObjectAttribute : Attribute { }
        public struct Result<T> { }
        """;

    private const string PublicConstructor = """
        using System;

        [ValueObject]
        public sealed class Email
        {
            public string Value { get; }
            public {|SKY0013:Email|}(string value) => Value = value;
            public static Result<Email> From(string value) => new Result<Email>();
        }

        public sealed class ValueObjectAttribute : Attribute { }
        public struct Result<T> { }
        """;

    private const string NoSmartConstructor = """
        using System;

        [ValueObject]
        public readonly struct {|SKY0013:Money|}
        {
            public decimal Amount { get; }
            private Money(decimal amount) => Amount = amount;
        }

        public sealed class ValueObjectAttribute : Attribute { }
        """;

    // A positional record carries a public primary constructor — a way in that skips the smart constructor.
    private const string Positional = """
        using System;

        [ValueObject]
        public readonly record struct {|SKY0013:Money|}(decimal Amount)
        {
            public static Result<Money> From(decimal amount) => new Result<Money>();
        }

        public sealed class ValueObjectAttribute : Attribute { }
        public struct Result<T> { }
        """;
}
