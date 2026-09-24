using Microsoft.EntityFrameworkCore;

namespace Golden.Api.Modules.Account;

/// <summary>Revoke a refresh token's whole family — logging out kills that device's session lineage.
/// The token hash is the credential and the session row is not tenant-scoped, so the lookup needs no
/// org. Idempotent: an unknown token still succeeds, so it never reveals whether a token was valid.
/// The matching access token lives out its remaining ≤15 minutes; only the refresh family dies now.</summary>
[Slice]
public static class Logout
{
    public record Input(string RefreshToken);

    public record Output();

    public static async Task<Result<Output>> Handle(Input input, AppDb db, CancellationToken ct)
    {
        var hash = SessionToken.Hash(input.RefreshToken);
        var session = await db.UserSessions.FirstOrDefaultAsync(s => s.TokenHash == hash, ct);
        if (session is not null)
            await Refresh.RevokeFamily(db, session.FamilyId, ct);
        return new Output();
    }

    public static void Map(IEndpointRouteBuilder app) =>
        app.MapPost("/logout", async (Input body, AppDb db, CancellationToken ct) =>
            (await Handle(body, db, ct)).ToHttp())
            .WithName(nameof(Logout))
            .AllowAnonymous();   // public: logout takes the refresh token, not the (possibly expired) access token (SKY0022)
}
