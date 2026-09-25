using Skies.Framework.Auth;
using Microsoft.EntityFrameworkCore;

namespace Golden.Api.Modules.Account;

/// <summary>
/// Register a new account with an email and password. Public. Email is unique globally, so a taken email is taken
/// everywhere. The password rule (the length bounds) is this slice's policy; the hashing is the framework's
/// <see cref="IPasswordHasher"/>, so the plaintext never reaches storage.
/// Every new account opens an org of its own and is its first member.
/// </summary>
/// <remarks>Security contract: a taken email creates nothing and damages nothing, yet gets the very answer a new
/// account gets, after the same hashing work, so registration cannot be used to find out who has an account. The
/// address's owner is told through <see cref="IAccountNotices"/> instead. Its spec under `.specs/` proves the happy
/// registration and the indistinguishable duplicate end-to-end.</remarks>
[Slice]
public static class Register
{
    public record Input(string Email, string Password);

    public record Output();

    private const int MinPasswordLength = 8;

    public static async Task<Result<Output>> Handle(Input input, AppDb db, IPasswordHasher hasher, IAccountNotices notices, TimeProvider clock, CancellationToken ct)
    {
        // A field missing from the JSON body binds as null: it fails validation like any other bad value.
        var password = input.Password ?? "";
        var email = Email.From(input.Email);
        var validation = new Validation()
            .Collect("email", email)
            .Check(password.Length >= MinPasswordLength, "password", AccountErrorCodes.PasswordTooShort, "must be at least 8 characters")
            .Check(password.Length <= IPasswordHasher.MaxPasswordLength, "password", AccountErrorCodes.PasswordTooLong, "must be at most 128 characters");
        if (validation.Failed)
            return validation.ToError();

        // Hashed before the lookup, so a taken email costs the same work as a new one.
        var passwordHash = hasher.Hash(password);
        if (await db.Users
#pragma warning disable SKY0030 // uniqueness is global: a taken email is taken in every org
            .IgnoreQueryFilters()
#pragma warning restore SKY0030
            .AnyAsync(u => u.Email == email.Value, ct))
        {
            await notices.AlreadyRegisteredAsync(email.Value, ct);
            return new Output();
        }

        var now = clock.GetUtcNow().UtcDateTime;
        var org = Org.Open(email.Value.Value, now);
        if (org.IsFailure)
            return org.Error;
        var created = User.Register(org.Value.Id, email.Value, passwordHash, now);
        if (created.IsFailure)
            return created.Error;
        db.Orgs.Add(org.Value);
        db.Users.Add(created.Value);
        await db.SaveChangesAsync(ct);
        return new Output();
    }

    public static void Map(IEndpointRouteBuilder app) =>
        app.MapPost("/register", async (Input input, AppDb db, IPasswordHasher hasher, IAccountNotices notices, TimeProvider clock, CancellationToken ct) =>
            (await Handle(input, db, hasher, notices, clock, ct)).ToHttp())
            .WithName(nameof(Register))
            .AllowAnonymous();   // public: registering is pre-identity
}
