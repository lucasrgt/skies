using System.Net;
using System.Net.Http.Json;
using System.Text.Json;
using Assay.Net;
using Assay.Net.Archetypes;
using Microsoft.Extensions.DependencyInjection;
using Sample.Api;
using Sample.Api.Modules.Wallets;
using Sample.Tests;

namespace Specs.S0009;

// The cases read the transfer's response as JSON rather than through Transfer.Output, so they build on the red
// revision (where the slice does not exist) and fail there for the observable reason: no /wallets/transfer route.
public class TransferSpec
{
    private static readonly Guid Source = Guid.Parse("11111111-1111-1111-1111-111111111111");
    private static readonly Guid Destination = Guid.Parse("22222222-2222-2222-2222-222222222222");

    [Fact(DisplayName = "FM-1: a transfer debits the source and credits the destination")]
    public async Task Transfer_moves_the_amount()
    {
        await using var app = await Booted(sourceBalance: 50m);
        var client = app.CreateClient();

        var response = await client.PostAsJsonAsync("/wallets/transfer",
            new { fromWalletId = Source, toWalletId = Destination, amount = 20m });

        response.EnsureSuccessStatusCode();
        var body = await response.Content.ReadFromJsonAsync<JsonElement>();
        Assert.Equal(30m, body.GetProperty("fromBalance").GetDecimal());
        Assert.Equal(20m, body.GetProperty("toBalance").GetDecimal());
        Assert.Equal(30m, await BalanceOf(client, Source));
        Assert.Equal(20m, await BalanceOf(client, Destination));
    }

    [Fact(DisplayName = "FM-2: overdrawing the source answers 422 and moves neither balance")]
    public async Task Overdraw_is_refused()
    {
        await using var app = await Booted(sourceBalance: 10m);
        var client = app.CreateClient();

        var response = await client.PostAsJsonAsync("/wallets/transfer",
            new { fromWalletId = Source, toWalletId = Destination, amount = 25m });

        Assert.Equal(HttpStatusCode.UnprocessableEntity, response.StatusCode);
        Assert.Equal(10m, await BalanceOf(client, Source));
        Assert.Equal(0m, await BalanceOf(client, Destination));
    }

    [Fact(DisplayName = "FM-3: a transfer from a wallet to itself answers 422 and leaves it untouched")]
    public async Task Same_wallet_is_refused()
    {
        await using var app = await Booted(sourceBalance: 10m);
        var client = app.CreateClient();

        var response = await client.PostAsJsonAsync("/wallets/transfer",
            new { fromWalletId = Source, toWalletId = Source, amount = 5m });

        Assert.Equal(HttpStatusCode.UnprocessableEntity, response.StatusCode);
        Assert.Equal(10m, await BalanceOf(client, Source));
    }

    [Fact(DisplayName = "FM-4: a missing destination answers 404 and the source keeps its money")]
    public async Task Missing_destination_is_not_found()
    {
        await using var app = await Booted(sourceBalance: 10m);
        var client = app.CreateClient();

        var response = await client.PostAsJsonAsync("/wallets/transfer",
            new { fromWalletId = Source, toWalletId = Guid.NewGuid(), amount = 5m });

        Assert.Equal(HttpStatusCode.NotFound, response.StatusCode);
        Assert.Contains(WalletsErrorCodes.NotFound, await response.Content.ReadAsStringAsync());
        Assert.Equal(10m, await BalanceOf(client, Source));
    }

    [Fact(DisplayName = "FM-4: a missing source answers 404 and the destination is not credited")]
    public async Task Missing_source_is_not_found()
    {
        await using var app = await Booted(sourceBalance: 10m);
        var client = app.CreateClient();

        var response = await client.PostAsJsonAsync("/wallets/transfer",
            new { fromWalletId = Guid.NewGuid(), toWalletId = Destination, amount = 5m });

        Assert.Equal(HttpStatusCode.NotFound, response.StatusCode);
        Assert.Contains(WalletsErrorCodes.NotFound, await response.Content.ReadAsStringAsync());
        Assert.Equal(0m, await BalanceOf(client, Destination));
    }

    [Fact(DisplayName = "FM-5: every invalid field is reported at once")]
    public async Task Every_invalid_field_is_reported()
    {
        await using var app = await Booted(sourceBalance: 10m);
        var client = app.CreateClient();

        var response = await client.PostAsJsonAsync("/wallets/transfer",
            new { fromWalletId = Guid.Empty, toWalletId = Guid.Empty, amount = -1m });

        Assert.Equal(HttpStatusCode.BadRequest, response.StatusCode);
        var body = await response.Content.ReadAsStringAsync();
        Assert.Contains("fromWalletId", body);
        Assert.Contains("toWalletId", body);
        Assert.Contains("amount", body);
    }

    // Assay's request-idempotency verifier posts twice with one key (the second must replay the source's balance)
    // and once with another (which must move the money again) against the real app. The verdict is saved before
    // asserting, so a failing run leaves the reason in evidence/.
    [Fact(DisplayName = "FM-6: a retry with the same Idempotency-Key moves the money once")]
    public async Task Same_key_moves_once()
    {
        await using var app = await Booted(sourceBalance: 100m);

        var verdict = await Runner.Run(
            Catalog.LoadDefault(),
            new RequestIdempotency(),
            "Transfer",
            new RequestIdempotencySubject("http://localhost", "/wallets/transfer",
                new { fromWalletId = Source, toWalletId = Destination, amount = 10m }, IdField: "fromBalance"),
            transport: app.CreateClient);
        SpecEvidence.Save("avp-FM-6.json", verdict);

        var result = verdict.Results.Single(r => r.CriterionId == "idempotency-key-honored");
        Assert.True(result.Status == VerdictStatus.Pass, result.Reason);
        // Two distinct keys applied, one retry replayed: 20 moved, and the total is conserved.
        var client = app.CreateClient();
        Assert.Equal(80m, await BalanceOf(client, Source));
        Assert.Equal(20m, await BalanceOf(client, Destination));
    }

    // Boots the app with the seeded wallet funded through the Deposit endpoint and a second, empty wallet opened
    // through the entity's own smart constructor (the sample has no "open wallet" endpoint).
    private static async Task<TestApp> Booted(decimal sourceBalance)
    {
        var app = new TestApp();
        using (var scope = app.Services.CreateScope())
        {
            var db = scope.ServiceProvider.GetRequiredService<AppDb>();
            db.Wallets.Add(Wallet.Open(Destination).Value);
            await db.SaveChangesAsync();
        }

        var deposit = await app.CreateClient().PostAsJsonAsync("/wallets/deposit",
            new { walletId = Source, amount = sourceBalance });
        deposit.EnsureSuccessStatusCode();
        return app;
    }

    private static async Task<decimal> BalanceOf(HttpClient client, Guid walletId) =>
        (await client.GetFromJsonAsync<GetBalance.Output>($"/wallets/{walletId}/balance"))!.Balance;
}
