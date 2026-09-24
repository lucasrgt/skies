using Skies.Framework.Auth;
using Microsoft.EntityFrameworkCore;

namespace Golden.Api.Modules.Account;

/// <summary>Complete a password reset. Public. Validates the new password (this slice's policy), consumes the token
/// through the framework's <see cref="VerificationTokens"/> (once, before it expires), sets a fresh hash, and ends every
/// session. The user lookup crosses the tenant filter (the caller is anonymous), so the account is found by id
/// regardless of org.</summary>
/// <remarks>Security contract: the sad path is the takeover guard — a wrong or expired token must be rejected and
/// leave the password unchanged. If it passed silently, anyone could reset any account's password. Its spec under `.specs/` proves both the happy reset and the bad-token rejection end-to-end.</remarks>
[Slice]
public static class ResetPassword
{
    public record Input(string Token, string NewPassword);

    public record Output();

    public static async Task<Result<Output>> Handle(Input input, AppDb db, VerificationTokens verification, IPasswordHasher hasher, RefreshSessions sessions, CancellationToken ct)
    {
        var validation = new Validation()
            .Check(input.NewPassword.Length >= 8, "new_password", AccountErrorCodes.PasswordTooShort, "must be at least 8 characters")
            .Check(input.NewPassword.Length <= IPasswordHasher.MaxPasswordLength, "new_password", AccountErrorCodes.PasswordTooLong, "must be at most 128 characters");
        if (validation.Failed)
            return validation.ToError();

        var check = await verification.ConsumeLinkAsync(VerificationPurpose.PasswordReset, input.Token, ct);
        if (!check.Verified)
            return Error.Unauthorized(AccountErrorCodes.ResetTokenInvalid, "invalid or expired token");

        var user = await db.Users.IgnoreQueryFilters().FirstOrDefaultAsync(u => u.Id == check.UserId, ct);
        if (user is null)
            return Error.NotFound(AccountErrorCodes.UserNotFound, "user not found");

        user.ResetPassword(hasher.Hash(input.NewPassword));
        await db.SaveChangesAsync(ct);

        // A password change ends every existing session: after a takeover recovery, the attacker's refresh
        // families die too, not just the victim's. The new password is the only way back in.
        await sessions.RevokeAllAsync(user.Id, ct);
        return new Output();
    }

    public static void Map(IEndpointRouteBuilder app) =>
        app.MapPost("/password-reset", async (Input input, AppDb db, VerificationTokens verification, IPasswordHasher hasher, RefreshSessions sessions, CancellationToken ct) =>
            (await Handle(input, db, verification, hasher, sessions, ct)).ToHttp())
            .WithName(nameof(ResetPassword))
            .AllowAnonymous();   // public: the reset token IS the credential
}
