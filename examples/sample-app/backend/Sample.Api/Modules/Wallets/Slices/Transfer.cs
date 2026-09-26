namespace Sample.Api.Modules.Wallets;

/// <summary>Transfer money from one wallet to another.</summary>
/// <remarks>
/// Why: a transfer is a withdrawal and a deposit that must land together — money is never created or lost in
/// transit. Both wallets change in memory through their own methods (so the source's overdraw rule is the one
/// <c>Wallet.Withdraw</c> owns) and reach the store in a single <c>SaveChangesAsync</c>, which is one transaction:
/// either both balances move or neither does. Every refusal (invalid input, same wallet, a missing wallet,
/// overdraw) returns before that save. It is idempotent: a request carrying an Idempotency-Key is applied at most
/// once — a retry replays the recorded outcome instead of moving the money again (spec 0009, FM-6).
/// </remarks>
[Slice]
public static class Transfer
{
    public record Input(Guid FromWalletId, Guid ToWalletId, decimal Amount);

    public record Output(Guid FromWalletId, decimal FromBalance, Guid ToWalletId, decimal ToBalance);

    public static async Task<Result<Output>> Handle(
        Input input, AppDb db, IIdempotencyStore idem, string? idempotencyKey, CancellationToken ct)
    {
        // Money stays the single source of the amount rule; the slice only collects its failure.
        var amount = Money.From(input.Amount);
        var validation = new Validation()
            .Require(input.FromWalletId, "fromWalletId", WalletsErrorCodes.FromWalletIdRequired)
            .Require(input.ToWalletId, "toWalletId", WalletsErrorCodes.ToWalletIdRequired)
            .Collect("amount", amount);
        if (validation.Failed)
            return validation.ToError();

        // Moving money to the wallet it came from is a well-formed request the business refuses (422, not 400).
        if (input.FromWalletId == input.ToWalletId)
            return Error.BusinessRule(WalletsErrorCodes.TransferToSameWallet,
                "a transfer needs two different wallets");

        if (!string.IsNullOrEmpty(idempotencyKey) && idem.TryGetOutcome<Output>(idempotencyKey, out var prior))
            return prior;

        var from = await db.Wallets.FindAsync([input.FromWalletId], ct);
        if (from is null)
            return Error.NotFound(WalletsErrorCodes.NotFound, $"wallet {input.FromWalletId} not found");
        var to = await db.Wallets.FindAsync([input.ToWalletId], ct);
        if (to is null)
            return Error.NotFound(WalletsErrorCodes.NotFound, $"wallet {input.ToWalletId} not found");

        // The overdraw rule lives on the entity; on failure nothing is saved, so neither balance moves.
        var withdrawn = from.Withdraw(amount.Value);
        if (withdrawn.IsFailure)
            return withdrawn.Error;
        to.Deposit(amount.Value);

        await db.SaveChangesAsync(ct);

        var output = new Output(from.Id, from.Balance.Amount, to.Id, to.Balance.Amount);
        if (!string.IsNullOrEmpty(idempotencyKey))
            idem.SaveOutcome(idempotencyKey, output);
        return output;
    }

    public static void Map(IEndpointRouteBuilder app) =>
        app.MapPost("/transfer",
                async (Input input, AppDb db, IIdempotencyStore idem, HttpContext http, CancellationToken ct) =>
                    (await Handle(input, db, idem, http.Request.Headers["Idempotency-Key"], ct)).ToHttp())
            .WithName(nameof(Transfer));
}
