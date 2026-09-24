using System.Net;
using System.Net.Http.Json;
using Microsoft.Extensions.DependencyInjection;
using Sample.Api;
using Sample.Api.BuildingBlocks;
using Sample.Api.Modules.Wallets;
using Sample.Tests;

namespace Specs.S0003;

public class WalletReadsSpec
{
    [Fact(DisplayName = "FM-1: the balance read returns the stored balance")]
    public async Task Balance_reflects_the_store()
    {
        await using var app = new TestApp();
        var id = (await Seed(app, 1, balance: 42m))[0];

        var balance = await app.CreateClient().GetFromJsonAsync<GetBalance.Output>($"/wallets/{id}/balance");

        Assert.Equal(42m, balance!.Balance);
    }

    [Fact(DisplayName = "FM-2: a balance read for a missing wallet answers 404")]
    public async Task Missing_wallet_is_not_found()
    {
        await using var app = new TestApp();

        var response = await app.CreateClient().GetAsync($"/wallets/{Guid.NewGuid()}/balance");

        Assert.Equal(HttpStatusCode.NotFound, response.StatusCode);
    }

    [Fact(DisplayName = "FM-3: a page carries its items and the count of the whole set")]
    public async Task Page_carries_items_and_total()
    {
        await using var app = new TestApp();
        await Seed(app, 4);

        var page = await List(app, "?page=2&pageSize=2");

        Assert.Equal(2, page.Items.Count);
        Assert.Equal(5, page.TotalCount);
        Assert.Equal(2, page.PageNumber);
        Assert.Equal(2, page.PageSize);
    }

    [Fact(DisplayName = "FM-4: hostile paging input is clamped server-side")]
    public async Task Hostile_paging_is_clamped()
    {
        await using var app = new TestApp();
        await Seed(app, 2);

        var page = await List(app, "?page=-2&pageSize=10000");

        Assert.Equal(1, page.PageNumber);
        Assert.Equal(100, page.PageSize);
        Assert.Equal(3, page.Items.Count);
    }

    [Fact(DisplayName = "FM-5: consecutive pages neither overlap nor skip")]
    public async Task Pages_partition_the_set()
    {
        await using var app = new TestApp();
        await Seed(app, 6);

        var ids = new List<Guid>();
        for (var number = 1; number <= 4; number++)
            ids.AddRange((await List(app, $"?page={number}&pageSize=2")).Items.Select(w => w.WalletId));

        Assert.Equal(7, ids.Count);
        Assert.Equal(7, ids.Distinct().Count());
    }

    private static async Task<List<Guid>> Seed(TestApp app, int count, decimal balance = 0m)
    {
        using var scope = app.Services.CreateScope();
        var db = scope.ServiceProvider.GetRequiredService<AppDb>();
        var ids = new List<Guid>();
        for (var i = 0; i < count; i++)
        {
            var wallet = Wallet.Open(Guid.NewGuid()).Value;
            if (balance > 0)
                wallet.Deposit(Money.From(balance).Value);
            db.Wallets.Add(wallet);
            ids.Add(wallet.Id);
        }
        await db.SaveChangesAsync();
        return ids;
    }

    private static async Task<PageBody> List(TestApp app, string query) =>
        (await app.CreateClient().GetFromJsonAsync<ListBody>($"/wallets{query}"))!.Wallets;

    private sealed record ListBody(PageBody Wallets);

    private sealed record PageBody(List<WalletBody> Items, int TotalCount, int PageNumber, int PageSize);

    private sealed record WalletBody(Guid WalletId, decimal Balance);
}
