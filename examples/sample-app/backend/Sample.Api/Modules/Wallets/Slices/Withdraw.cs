using Microsoft.EntityFrameworkCore;

namespace Sample.Api.Modules.Wallets;

/// <summary>Withdraw money from a wallet.</summary>
/// <remarks>
/// Why: a withdrawal is the only outflow that shrinks a balance, and it must never overdraw. That
/// invariant lives on the <see cref="Wallet"/> entity — <c>Wallet.Withdraw</c> returns a failure when the
/// balance cannot cover the amount — so no slice can bypass it and the balance stays authoritative
/// server-side. The slice only orchestrates: validate input, load, delegate, persist. It is idempotent: a
/// request carrying an Idempotency-Key is applied at most once — a retry replays the recorded outcome instead
/// of debiting again (spec 0002, FM-5).
/// </remarks>
[Slice]
public static class Withdraw
{
    public record Input(Guid WalletId, decimal Amount);

    public record Output(Guid WalletId, decimal Balance);

    public static async Task<Result<Output>> Handle(
        Input input, AppDb db, IIdempotencyStore idem, string? idempotencyKey, CancellationToken ct)
    {
        var amount = Money.From(input.Amount);
        var validation = new Validation()
            .Require(input.WalletId, "walletId", WalletsErrorCodes.WalletIdRequired)
            .Collect("amount", amount);
        if (validation.Failed)
            return validation.ToError();

        // Idempotency: a retried request carrying the same key replays the recorded outcome instead of
        // debiting again.
        if (!string.IsNullOrEmpty(idempotencyKey) && idem.TryGet(idempotencyKey, out var prior))
            return new Output(input.WalletId, prior);

        var wallet = await db.Wallets.FindAsync([input.WalletId], ct);
        if (wallet is null)
            return Error.NotFound(WalletsErrorCodes.NotFound, $"wallet {input.WalletId} not found");

        // The overdraw rule lives on the entity; the slice only propagates its verdict. On failure the
        // balance is untouched and SaveChanges never runs — no partial state crosses the boundary.
        var withdrawn = wallet.Withdraw(amount.Value);
        if (withdrawn.IsFailure)
            return withdrawn.Error;

        await db.SaveChangesAsync(ct);

        // Record the outcome under the key so a retry replays it rather than debiting a second time.
        if (!string.IsNullOrEmpty(idempotencyKey))
            idem.Save(idempotencyKey, wallet.Balance.Amount);

        return new Output(wallet.Id, wallet.Balance.Amount);
    }

    public static void Map(IEndpointRouteBuilder app) =>
        app.MapPost("/withdraw",
                async (Input input, AppDb db, IIdempotencyStore idem, HttpContext http, CancellationToken ct) =>
                    (await Handle(input, db, idem, http.Request.Headers["Idempotency-Key"], ct)).ToHttp())
            .WithName(nameof(Withdraw));
}
