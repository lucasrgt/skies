namespace Sample.Api.Modules.Wallets;

/// <summary>The Wallets module's error codes — the stable, namespaced, language-neutral keys the frontend
/// localizes from. One registry per module: it <em>is</em> the catalog of what can go wrong here. The doctor
/// (<c>SKY0018</c>) requires every <c>Error.*</c> / <c>Validation.Check</c> to use a code from a registry like
/// this rather than an inline literal, so the full set stays discoverable — the OpenAPI document enumerates it
/// into the typed client, and the frontend's i18n is checked exhaustively against it.</summary>
public static class WalletsErrorCodes
{
    /// <summary>No wallet exists for the given id.</summary>
    public const string NotFound = "wallets.not_found";

    /// <summary>The withdrawal would overdraw the balance.</summary>
    public const string InsufficientFunds = "wallets.insufficient_funds";

    /// <summary>The wallet id is required (entity invariant).</summary>
    public const string IdRequired = "wallet.id.required";

    /// <summary>The balance cannot be negative (entity invariant).</summary>
    public const string BalanceNegative = "wallet.balance.negative";

    /// <summary>The <c>walletId</c> input is required.</summary>
    public const string WalletIdRequired = "walletId.required";

    /// <summary>The <c>fromWalletId</c> input of a transfer is required.</summary>
    public const string FromWalletIdRequired = "fromWalletId.required";

    /// <summary>The <c>toWalletId</c> input of a transfer is required.</summary>
    public const string ToWalletIdRequired = "toWalletId.required";

    /// <summary>A transfer names the same wallet as its source and destination.</summary>
    public const string TransferToSameWallet = "wallets.transfer_same_wallet";
}
