using Skies.Framework.Abstractions;

namespace Sample.Api.Modules.Wallets;

/// <summary>
/// The Wallets module's wiring root. A module owns both halves of its composition: <see cref="AddServices"/>
/// (its own DI) and <see cref="Map"/> (its routes, under the /wallets base, each slice relative to it). The host
/// calls neither directly — the module registry does, one line per module — so the composition root stays a thin
/// index. Module-wide endpoint config (auth, tags, versioning) belongs in <see cref="Map"/> too, on the group.
/// </summary>
[Module]
public static class WalletsModule
{
    /// <summary>The module's own service registration. Wallets needs none yet — the app-wide DbContext is wired
    /// in the composition root — but the seam is declared uniformly, the way every module carries both halves.</summary>
    public static IServiceCollection AddServices(IServiceCollection services, IConfiguration configuration) =>
        services;

    /// <summary>Seed a demo wallet so the sample reads non-empty on first run. Seed <em>content</em> belongs to the
    /// module that owns the data (it writes only its own entities); the composition root only orchestrates the
    /// scope and order (see <c>Program.cs</c>). Idempotent — a populated store is left untouched.</summary>
    public static void Seed(AppDb db)
    {
        if (db.Wallets.Any())
            return;
        db.Wallets.Add(Wallet.Open(Guid.Parse("11111111-1111-1111-1111-111111111111")).Value);
        db.SaveChanges();
    }

    public static void Map(IEndpointRouteBuilder app)
    {
        // The sample ships no identity provider, so the group is anonymous *on purpose* — the doctor (SKY0022)
        // demands the decision be written down either way. A real app says .RequireAuthorization() here.
        var wallets = app.MapGroup("/wallets").AllowAnonymous();

        Deposit.Map(wallets);
        Withdraw.Map(wallets);
        GetBalance.Map(wallets);
        ListWallets.Map(wallets);
        Transfer.Map(wallets);
    }
}
