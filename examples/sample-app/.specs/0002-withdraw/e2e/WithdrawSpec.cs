using System.Net;
using System.Net.Http.Json;
using Sample.Api.Modules.Wallets;
using Sample.Tests;

namespace Specs.S0002;

public class WithdrawSpec
{
    private static readonly Guid Seeded = Guid.Parse("11111111-1111-1111-1111-111111111111");

    [Fact(DisplayName = "FM-1: a withdrawal is reflected by the next balance read")]
    public async Task Withdrawal_is_reflected_by_the_balance()
    {
        await using var app = new TestApp();
        var client = app.CreateClient();
        await client.PostAsJsonAsync("/wallets/deposit", new { walletId = Seeded, amount = 50m });

        var response = await client.PostAsJsonAsync("/wallets/withdraw", new { walletId = Seeded, amount = 20m });

        response.EnsureSuccessStatusCode();
        Assert.Equal(30m, (await response.Content.ReadFromJsonAsync<Withdraw.Output>())!.Balance);
        Assert.Equal(30m, await BalanceOf(client));
    }

    [Fact(DisplayName = "FM-2: overdrawing answers 422 and leaves the balance untouched")]
    public async Task Overdraw_is_refused()
    {
        await using var app = new TestApp();
        var client = app.CreateClient();
        await client.PostAsJsonAsync("/wallets/deposit", new { walletId = Seeded, amount = 10m });

        var response = await client.PostAsJsonAsync("/wallets/withdraw", new { walletId = Seeded, amount = 999m });

        Assert.Equal(HttpStatusCode.UnprocessableEntity, response.StatusCode);
        Assert.Equal(10m, await BalanceOf(client));
    }

    [Fact(DisplayName = "FM-3: a negative amount answers 400 with an amount field error")]
    public async Task Negative_amount_is_rejected()
    {
        await using var app = new TestApp();
        var client = app.CreateClient();

        var response = await client.PostAsJsonAsync("/wallets/withdraw", new { walletId = Seeded, amount = -1m });

        Assert.Equal(HttpStatusCode.BadRequest, response.StatusCode);
        Assert.Contains("amount", await response.Content.ReadAsStringAsync());
    }

    [Fact(DisplayName = "FM-4: a missing wallet answers 404")]
    public async Task Missing_wallet_is_not_found()
    {
        await using var app = new TestApp();
        var client = app.CreateClient();

        var response = await client.PostAsJsonAsync("/wallets/withdraw", new { walletId = Guid.NewGuid(), amount = 5m });

        Assert.Equal(HttpStatusCode.NotFound, response.StatusCode);
    }

    [Fact(DisplayName = "FM-5: a retry with the same Idempotency-Key debits once")]
    public async Task Same_key_debits_once()
    {
        await using var app = new TestApp();
        var client = app.CreateClient();
        await client.PostAsJsonAsync("/wallets/deposit", new { walletId = Seeded, amount = 50m });

        await Post(client, "retry-1", 20m);
        var retry = await Post(client, "retry-1", 20m);

        Assert.Equal(HttpStatusCode.OK, retry.StatusCode);
        Assert.Equal(30m, await BalanceOf(client));
    }

    private static Task<HttpResponseMessage> Post(HttpClient client, string key, decimal amount)
    {
        var request = new HttpRequestMessage(HttpMethod.Post, "/wallets/withdraw")
        {
            Content = JsonContent.Create(new { walletId = Seeded, amount }),
        };
        request.Headers.Add("Idempotency-Key", key);
        return client.SendAsync(request);
    }

    private static async Task<decimal> BalanceOf(HttpClient client) =>
        (await client.GetFromJsonAsync<GetBalance.Output>($"/wallets/{Seeded}/balance"))!.Balance;
}
