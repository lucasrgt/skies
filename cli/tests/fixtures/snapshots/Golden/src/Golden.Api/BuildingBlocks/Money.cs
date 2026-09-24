namespace Golden.Api.BuildingBlocks;

/// <summary>Error codes for the Money value object — stable, namespaced i18n keys the frontend localizes
/// from, declared as constants so the set stays enumerable into the OpenAPI contract.</summary>
public static class MoneyErrorCodes
{
    /// <summary>The value is missing or fails the Money invariant.</summary>
    public const string Required = "money.required";
}

/// <summary>
/// Money — an always-valid value object: the type <em>is</em> the rule. There is no public way to
/// construct an invalid Money, so any Money in the system is already valid. Fill in the
/// invariant in <see cref="From"/> (it wraps a string by default — change the wrapped type to fit).
/// </summary>
[ValueObject]
public readonly record struct Money
{
    /// <summary>The wrapped value.</summary>
    public string Value { get; }

    private Money(string value) => Value = value;

    /// <summary>Build a Money, rejecting an invalid value as a domain error.</summary>
    public static Result<Money> From(string value) =>
        !string.IsNullOrWhiteSpace(value)
            ? Result<Money>.Ok(new Money(value))
            : Error.Validation(MoneyErrorCodes.Required, "money is required");

    /// <inheritdoc />
    public override string ToString() => Value;
}
