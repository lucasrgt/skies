using Sample.Api.BuildingBlocks;

namespace Sample.Tests.BuildingBlocks;

public class MoneyTests
{
    // The boundary the mutation run exposed: 0 is valid money. Without this, flipping `>= 0` to
    // `> 0` survived — no test exercised the zero case.
    [Fact]
    public void Zero_is_valid_money()
    {
        var result = Money.From(0m);

        Assert.True(result.IsSuccess);
        Assert.Equal(0m, result.Value.Amount);
    }

    [Fact]
    public void A_positive_amount_is_valid()
    {
        var result = Money.From(10m);

        Assert.True(result.IsSuccess);
        Assert.Equal(10m, result.Value.Amount);
    }

    [Fact]
    public void A_negative_amount_is_rejected()
    {
        var result = Money.From(-1m);

        Assert.True(result.IsFailure);
        Assert.Equal(ErrorKind.Validation, result.Error.Kind);
    }

    [Fact]
    public void Add_sums_two_amounts()
    {
        var sum = Money.From(10m).Value.Add(Money.From(5m).Value);

        Assert.Equal(15m, sum.Amount);
    }
}
