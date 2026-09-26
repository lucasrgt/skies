using Skies.Framework.Auth;

namespace Golden.Api.Modules.Account;

/// <summary>Revoke a refresh token's whole family — logging out kills that device's session lineage.
/// Idempotent: an unknown token still succeeds, so it never reveals whether a token was valid.
/// The matching access token lives out its remaining ≤15 minutes; only the refresh family dies now.</summary>
[Slice]
public static class Logout
{
    public record Input(string RefreshToken);

    public record Output();

    public static async Task<Result<Output>> Handle(Input input, RefreshSessions sessions, CancellationToken ct)
    {
        await sessions.RevokeAsync(input.RefreshToken, ct);
        return new Output();
    }

    public static void Map(IEndpointRouteBuilder app) =>
        app.MapPost("/logout", async (Input? body, HttpContext http, RefreshSessions sessions, RefreshCookie cookies, CancellationToken ct) =>
            Respond(await Handle(new Input(cookies.RefreshFrom(http.Request, body?.RefreshToken)), sessions, ct), http, cookies))
            .WithName(nameof(Logout))
            .AllowAnonymous();   // public: logout takes the refresh token, not the (possibly expired) access token

    private static IResult Respond(Result<Output> result, HttpContext http, RefreshCookie cookies)
    {
        cookies.Clear(http.Response);   // harmless for non-web; the browser drops the cookie
        return result.ToHttp();
    }
}
