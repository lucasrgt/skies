using Skies.Framework.Auth;
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
        app.MapPost("/logout", async (Input? body, HttpContext http, AppDb db, RefreshCookie cookies, CancellationToken ct) =>
            Respond(await Handle(new Input(cookies.RefreshFrom(http.Request, body?.RefreshToken)), db, ct), http, cookies))
            .WithName(nameof(Logout))
            .AllowAnonymous();   // public: logout takes the refresh token, not the (possibly expired) access token (SKY0022)

    private static IResult Respond(Result<Output> result, HttpContext http, RefreshCookie cookies)
    {
        cookies.Clear(http.Response);   // harmless for non-web; the browser drops the cookie
        return result.ToHttp();
    }
}
