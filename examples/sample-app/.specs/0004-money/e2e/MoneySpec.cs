using Sample.Api.BuildingBlocks;

namespace Specs.S0004;

// Isolated cases: Money is a leaf type with no I/O, so each case calls the smart constructor directly.
public class MoneySpec
{
    [Fact(DisplayName = "FM-1: zero is valid money")]
    public void Zero_is_valid_money()
    {
        var result = Money.From(0m);

        Assert.True(result.IsSuccess);
        Assert.Equal(0m, result.Value.Amount);
    }

    [Fact(DisplayName = "FM-2: a positive amount is valid and keeps its value")]
    public void A_positive_amount_is_valid()
    {
        var result = Money.From(10m);

        Assert.True(result.IsSuccess);
        Assert.Equal(10m, result.Value.Amount);
    }

    [Fact(DisplayName = "FM-3: a negative amount is rejected as a validation error")]
    public void A_negative_amount_is_rejected()
    {
        var result = Money.From(-1m);

        Assert.True(result.IsFailure);
        Assert.Equal(ErrorKind.Validation, result.Error.Kind);
        Assert.Equal(MoneyErrorCodes.Negative, result.Error.Code);
    }

    [Fact(DisplayName = "FM-4: adding two amounts produces their sum")]
    public void Add_sums_two_amounts()
    {
        var sum = Money.Zero.Add(Money.From(10m).Value).Add(Money.From(5m).Value);

        Assert.Equal(15m, sum.Amount);
    }
}
