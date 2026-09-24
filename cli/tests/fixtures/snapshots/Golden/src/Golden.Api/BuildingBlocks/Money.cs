namespace Golden.Api.BuildingBlocks;

public static class MoneyErrorCodes
{
    public const string Required = "money.required";
}

/// <summary>A non-empty value. Replace the predicate with the domain's validation.</summary>
[ValueObject]
public sealed record Money
{
    public string Value { get; }

    private Money(string value) => Value = value;

    public static Result<Money> From(string value) =>
        !string.IsNullOrWhiteSpace(value)
            ? Result<Money>.Ok(new Money(value))
            : Error.Validation(MoneyErrorCodes.Required, "money is required");

    public override string ToString() => Value;
}
