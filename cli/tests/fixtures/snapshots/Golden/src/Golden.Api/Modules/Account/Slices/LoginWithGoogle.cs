using Skies.Framework.Auth;
using Skies.Framework.Identity;
using Microsoft.EntityFrameworkCore;

namespace Golden.Api.Modules.Account;

/// <summary>Sign in with a Google identity. Verifies the token, finds the user by the Google-verified
/// email (global identity, so the lookup crosses the tenant filter), and issues a session.</summary>
/// <remarks>Security contract: the sad path is the bypass guard — an unverifiable token must be denied, never
/// yield a session. If it passed silently, a forged token logs in as any user (takeover). Its spec under `.specs/` proves
/// both the happy sign-in and the forged-token rejection end-to-end.</remarks>
[Slice]
public static class LoginWithGoogle
{
    public record Input(string IdToken);

    public record Output(string AccessToken, string RefreshToken, RegistrationStep Step, Role? Role);

    public static async Task<Result<Output>> Handle(Input input, AppDb db, IExternalIdentityVerifier google, RefreshSessions sessions, IAccessTokens tokens, CancellationToken ct)
    {
        var identity = await google.VerifyAsync(input.IdToken, ct);
        if (identity.IsFailure)
            return identity.Error;

        var email = Email.From(identity.Value.Email);
        if (email.IsFailure)
            return Error.Unauthorized(AccountErrorCodes.InvalidToken, "invalid google token");

        var user = await db.Users.IgnoreQueryFilters().FirstOrDefaultAsync(u => u.Email == email.Value, ct);
        if (user is null)
            return Error.Unauthorized(AccountErrorCodes.NoAccount, "no account for this google identity");

        var session = await sessions.StartAsync(user.Id, ct);
        var access = tokens.Issue(user.Id, user.OrgId, user.Role?.ToString(), session.FamilyId, user.Name);
        return new Output(access, session.Token, user.RegistrationStep, user.Role);
    }

    public static void Map(IEndpointRouteBuilder app) =>
        app.MapPost("/login/google", async (Input input, AppDb db, IExternalIdentityVerifier google, RefreshSessions sessions, IAccessTokens tokens, CancellationToken ct) =>
            (await Handle(input, db, google, sessions, tokens, ct)).ToHttp())
            .WithName(nameof(LoginWithGoogle))
            .AllowAnonymous();   // public: signing in with Google is how you get a token (SKY0022 — the decision, made visible)
}
