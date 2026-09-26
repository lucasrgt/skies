using Skies.Framework.Auth;
using Microsoft.EntityFrameworkCore;

namespace Golden.Api.Modules.Account;

/// <summary>Revoke one of the caller's sessions by its id (the refresh family). Only the owner can
/// revoke it; an unknown or another user's id is a not-found, never a hint that it exists. The ownership check is this
/// slice's; the revocation is the framework's family burn, so every rotated slot in the lineage dies.</summary>
[Slice]
public static class RevokeSession
{
    public record Input(Guid SessionId);

    public record Output();

    public static async Task<Result<Output>> Handle(Input input, AppDb db, RefreshSessions sessions, ICurrentUser current, CancellationToken ct)
    {
        var owned = await db.UserSessions.AnyAsync(s => s.FamilyId == input.SessionId && s.UserId == current.UserId, ct);
        if (!owned)
            return Error.NotFound(AccountErrorCodes.SessionNotFound, "session not found");

        await sessions.RevokeFamilyAsync(input.SessionId, ct);
        return new Output();
    }

    public static void Map(IEndpointRouteBuilder app) =>
        app.MapPost("/sessions/revoke", async (Input input, AppDb db, RefreshSessions sessions, ICurrentUser current, CancellationToken ct) =>
            (await Handle(input, db, sessions, current, ct)).ToHttp())
            .WithName(nameof(RevokeSession))
            .RequireAuthorization();
}
