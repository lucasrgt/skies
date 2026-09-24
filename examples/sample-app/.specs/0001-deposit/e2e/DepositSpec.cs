using System.Net;
using System.Net.Http.Json;
using Sample.Api.Modules.Wallets;
using Sample.Tests;

namespace Specs.S0001;

public class DepositSpec
{
    private static readonly Guid Seeded = Guid.Parse("11111111-1111-1111-1111-111111111111");

    [Fact(DisplayName = "FM-1: a deposit is reflected by the next balance read")]
    public async Task Deposit_is_reflected_by_the_balance()
    {
        await using var app = new TestApp();
        var client = app.CreateClient();

        var response = await client.PostAsJsonAsync("/wallets/deposit", new { walletId = Seeded, amount = 30m });

        response.EnsureSuccessStatusCode();
        Assert.Equal(30m, (await response.Content.ReadFromJsonAsync<Deposit.Output>())!.Balance);
        Assert.Equal(30m, await BalanceOf(client, Seeded));
    }

    [Fact(DisplayName = "FM-2: a negative amount is rejected and the balance is untouched")]
    public async Task Negative_amount_is_rejected()
    {
        await using var app = new TestApp();
        var client = app.CreateClient();
        await client.PostAsJsonAsync("/wallets/deposit", new { walletId = Seeded, amount = 12m });

        var response = await client.PostAsJsonAsync("/wallets/deposit", new { walletId = Seeded, amount = -5m });

        Assert.Equal(HttpStatusCode.BadRequest, response.StatusCode);
        Assert.Equal(12m, await BalanceOf(client, Seeded));
    }

    [Fact(DisplayName = "FM-3: every invalid field is reported at once")]
    public async Task Every_invalid_field_is_reported()
    {
        await using var app = new TestApp();
        var client = app.CreateClient();

        var response = await client.PostAsJsonAsync("/wallets/deposit", new { walletId = Guid.Empty, amount = -1m });

        Assert.Equal(HttpStatusCode.BadRequest, response.StatusCode);
        var body = await response.Content.ReadAsStringAsync();
        Assert.Contains("walletId", body);
        Assert.Contains("amount", body);
    }

    [Fact(DisplayName = "FM-4: a missing wallet answers 404")]
    public async Task Missing_wallet_is_not_found()
    {
        await using var app = new TestApp();
        var client = app.CreateClient();

        var response = await client.PostAsJsonAsync("/wallets/deposit", new { walletId = Guid.NewGuid(), amount = 5m });

        Assert.Equal(HttpStatusCode.NotFound, response.StatusCode);
    }

    [Fact(DisplayName = "FM-5: a retry with the same Idempotency-Key credits once")]
    public async Task Same_key_credits_once()
    {
        await using var app = new TestApp();
        var client = app.CreateClient();

        var first = await Post(client, "retry-1", 10m);
        var retry = await Post(client, "retry-1", 10m);

        Assert.Equal(HttpStatusCode.OK, first.StatusCode);
        Assert.Equal(HttpStatusCode.OK, retry.StatusCode);
        Assert.Equal(10m, (await retry.Content.ReadFromJsonAsync<Deposit.Output>())!.Balance);
        Assert.Equal(10m, await BalanceOf(client, Seeded));
    }

    [Fact(DisplayName = "FM-6: a new Idempotency-Key credits again")]
    public async Task New_key_credits_again()
    {
        await using var app = new TestApp();
        var client = app.CreateClient();

        await Post(client, "key-a", 10m);
        await Post(client, "key-b", 10m);

        Assert.Equal(20m, await BalanceOf(client, Seeded));
    }

    private static Task<HttpResponseMessage> Post(HttpClient client, string key, decimal amount)
    {
        var request = new HttpRequestMessage(HttpMethod.Post, "/wallets/deposit")
        {
            Content = JsonContent.Create(new { walletId = Seeded, amount }),
        };
        request.Headers.Add("Idempotency-Key", key);
        return client.SendAsync(request);
    }

    private static async Task<decimal> BalanceOf(HttpClient client, Guid walletId) =>
        (await client.GetFromJsonAsync<GetBalance.Output>($"/wallets/{walletId}/balance"))!.Balance;
}
