using Skies.Framework.Auth;
using Skies.Framework.Mail;
using Microsoft.EntityFrameworkCore;

namespace Golden.Api.Modules.Account;

/// <summary>Send the caller a fresh email-verification link. Authenticated. The framework's
/// <see cref="VerificationTokens"/> mints a high-entropy token and stores only its hash; this slice owns its lifetime
/// (24h) and the message. The send is best-effort: the token is already persisted, so a later resend recovers.</summary>
[Slice]
public static class RequestEmailVerification
{
    public record Input();

    public record Output();

    private static readonly TimeSpan Lifetime = TimeSpan.FromHours(24);

    public static async Task<Result<Output>> Handle(Input input, AppDb db, VerificationTokens verification, IEmailSender email, ICurrentUser current, CancellationToken ct)
    {
        var user = await db.Users.FirstOrDefaultAsync(u => u.Id == current.UserId, ct);
        if (user is null)
            return Error.NotFound(AccountErrorCodes.UserNotFound, "user not found");

        var raw = await verification.IssueLinkAsync(user.Id, VerificationPurpose.EmailVerification, Lifetime, ct);

        await email.SendAsync(new EmailMessage(
            user.Email.Value,
            "Confirm your email",
            $"Confirm your email with this token: {raw}"), ct);
        return new Output();
    }

    public static void Map(IEndpointRouteBuilder app) =>
        app.MapPost("/verify-email/request", async (Input input, AppDb db, VerificationTokens verification, IEmailSender email, ICurrentUser current, CancellationToken ct) =>
            (await Handle(input, db, verification, email, current, ct)).ToHttp())
            .WithName(nameof(RequestEmailVerification))
            .RequireAuthorization();
}
