using Skies.Framework.Auth;
using Microsoft.EntityFrameworkCore;

namespace Golden.Api.Modules.Account;

/// <summary>Exchange a valid refresh token for a fresh access + refresh pair. The framework's
/// <see cref="RefreshSessions"/> rotates the session: a spent token presented again is theft and burns the whole family,
/// and a family past its absolute age is retired. This slice maps each refusal to its error code and mints the new
/// access token from the user.</summary>
/// <remarks>Security contract: the sad path <em>is</em> the security feature — replaying a rotated token must
/// burn the whole family (theft detection). If that fails silently a stolen token lives forever, so
/// Its spec under `.specs/` proves both the rotation (happy) and the theft-burn (sad) end-to-end.</remarks>
[Slice]
public static class Refresh
{
    public record Input(string RefreshToken);

    public record Output(string AccessToken, string RefreshToken);

    public static async Task<Result<Output>> Handle(Input input, AppDb db, RefreshSessions sessions, IAccessTokens tokens, CancellationToken ct)
    {
        var rotation = await sessions.RotateAsync(input.RefreshToken, ct);
        if (!rotation.Rotated)
            return rotation.Outcome switch
            {
                RefreshOutcome.Reused => Error.Unauthorized(AccountErrorCodes.SessionRevoked, "session revoked"),
                // Lost the race to a concurrent refresh of the same live token (tabs sharing a cookie): the winner
                // already delivered the replacement, so this is the benign retry, not theft.
                RefreshOutcome.Superseded => Error.Unauthorized(AccountErrorCodes.SessionRetry, "refresh superseded, retry"),
                _ => Error.Unauthorized(AccountErrorCodes.InvalidSession, "invalid or expired session"),
            };

        var user = await db.Users
#pragma warning disable SKY0030 // the access token has expired, so the request carries no org; the session names the user
            .IgnoreQueryFilters()
#pragma warning restore SKY0030
            .FirstOrDefaultAsync(u => u.Id == rotation.UserId, ct);
        if (user is null)
            return Error.Unauthorized(AccountErrorCodes.InvalidSession, "invalid or expired session");

        var access = tokens.Issue(user.Id, user.OrgId, user.Role?.ToString(), rotation.FamilyId, user.Name);
        return new Output(access, rotation.Token);
    }

    public static void Map(IEndpointRouteBuilder app) =>
        app.MapPost("/refresh", async (Input? body, HttpContext http, AppDb db, RefreshSessions sessions, IAccessTokens tokens, RefreshCookie cookies, CancellationToken ct) =>
            Respond(await Handle(new Input(cookies.RefreshFrom(http.Request, body?.RefreshToken)), db, sessions, tokens, ct), http, cookies))
            .WithName(nameof(Refresh))
            .AllowAnonymous();   // public: the refresh token IS the credential, the access token is expired

    private static IResult Respond(Result<Output> result, HttpContext http, RefreshCookie cookies)
    {
        if (result.IsFailure)
            return result.ToHttp();
        var o = result.Value;
        return cookies.Deliver(http, o.RefreshToken, o, new { o.AccessToken });
    }
}
