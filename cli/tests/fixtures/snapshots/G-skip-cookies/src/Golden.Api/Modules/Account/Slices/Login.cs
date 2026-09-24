using Skies.Framework.Auth;
using Microsoft.EntityFrameworkCore;

namespace Golden.Api.Modules.Account;

/// <summary>Authenticate with email + password; issue an access token (JWT) and a refresh token. The
/// user is looked up by email — globally, one-human-one-account — and the identity is read off the
/// user and put into the JWT, never assumed.</summary>
/// <remarks>Security contract: a silent regression on the sad path (bad credentials accidentally yielding a
/// token) is an auth bypass — trivial code, catastrophic blast radius. Its spec under `.specs/` proves the happy and the
/// deny path end-to-end.</remarks>
[Slice]
public static class Login
{
    public record Input(string Email, string Password);

    public record Output(string AccessToken, string RefreshToken, RegistrationStep Step, Role? Role);

    public static async Task<Result<Output>> Handle(Input input, AppDb db, IPasswordHasher hasher, RefreshSessions sessions, IAccessTokens tokens, CancellationToken ct)
    {
        var email = Email.From(input.Email);
        if (email.IsFailure)
            return Error.Unauthorized(AccountErrorCodes.InvalidCredentials, "invalid email or password");

        var user = await db.Users
            // Anonymous login has no org of its own, so the lookup crosses the tenant filter; the org
            // is then read off the found user and put into the JWT.
            .IgnoreQueryFilters()
            .FirstOrDefaultAsync(u => u.Email == email.Value, ct);
        // Verify even when no account matched: the hasher then spends the same work on a dummy, so a missing email is
        // not observable by response timing (user enumeration), and both failures return the same error.
        if (!hasher.Verify(input.Password, user?.PasswordHash) || user is null)
            return Error.Unauthorized(AccountErrorCodes.InvalidCredentials, "invalid email or password");

        var session = await sessions.StartAsync(user.Id, ct);   // a new family: the session id (sid) the token carries
        var access = tokens.Issue(user.Id, user.OrgId, user.Role?.ToString(), session.FamilyId, user.Name);
        return new Output(access, session.Token, user.RegistrationStep, user.Role);
    }

    public static void Map(IEndpointRouteBuilder app) =>
        app.MapPost("/login", async (Input input, AppDb db, IPasswordHasher hasher, RefreshSessions sessions, IAccessTokens tokens, CancellationToken ct) =>
            (await Handle(input, db, hasher, sessions, tokens, ct)).ToHttp())
            .WithName(nameof(Login))
            .AllowAnonymous();   // public: logging in is how you get a token (SKY0022 — the decision, made visible)
}
