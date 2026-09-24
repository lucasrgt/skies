using Skies.Framework.Auth;
using Microsoft.EntityFrameworkCore;

namespace Golden.Api.Modules.Account;

/// <summary>
/// Register a new account with an email and password. Public. Email is unique globally, so a taken
/// email is taken everywhere. The password rule (the minimum length) is this slice's policy; the hashing is the
/// framework's <see cref="IPasswordHasher"/>, so the plaintext never reaches storage.
/// </summary>
/// <remarks>Security contract: the sad path (a taken email) guards the one-human-one-account identity invariant.
/// If a duplicate were silently created, login-by-email becomes ambiguous — an identity breach in the
/// auth system. Its spec under `.specs/` proves both the happy registration and the duplicate rejection end-to-end.</remarks>
[Slice]
public static class Register
{
    public record Input(string Email, string Password);

    public record Output(Guid UserId);

    public static async Task<Result<Output>> Handle(Input input, AppDb db, IPasswordHasher hasher, TimeProvider clock, CancellationToken ct)
    {
        var email = Email.From(input.Email);
        var validation = new Validation()
            .Collect("email", email)
            .Check(input.Password.Length >= 8, "password", AccountErrorCodes.PasswordTooShort, "must be at least 8 characters");
        if (validation.Failed)
            return validation.ToError();

        if (await db.Users
            .AnyAsync(u => u.Email == email.Value, ct))
            return Error.Conflict(AccountErrorCodes.EmailTaken, "an account with this email already exists");

        var now = clock.GetUtcNow().UtcDateTime;
        var created = User.Register(email.Value, hasher.Hash(input.Password), now);
        if (created.IsFailure)
            return created.Error;
        db.Users.Add(created.Value);
        await db.SaveChangesAsync(ct);

        return new Output(created.Value.Id);
    }

    public static void Map(IEndpointRouteBuilder app) =>
        app.MapPost("/register", async (Input input, AppDb db, IPasswordHasher hasher, TimeProvider clock, CancellationToken ct) =>
            (await Handle(input, db, hasher, clock, ct)).ToHttp())
            .WithName(nameof(Register))
            .AllowAnonymous();   // public: registering is pre-identity
}
