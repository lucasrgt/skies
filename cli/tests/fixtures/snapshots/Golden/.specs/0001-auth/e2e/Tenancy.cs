using Microsoft.IdentityModel.JsonWebTokens;
using Golden.Api;
using Golden.Api.BuildingBlocks;
using Golden.Api.Modules.Account;
using Golden.Api.Tenancy;
using Golden.Tests;
using Microsoft.EntityFrameworkCore;
using Skies.Framework.Auth;
using Skies.Framework.EntityFrameworkCore;

namespace Specs.S0001;

/// <summary>Tenancy and global identity: each registration opens its own org, sign-in crosses orgs, reads never do,
/// and a request without a signed-in caller resolves no org at all.</summary>
public class TenancyAndGlobalIdentity
{
    private static readonly Argon2idPasswordHasher Hasher = new();

    [Fact(DisplayName = "FM-7: each registration opens an org of its own, carried in its access token")]
    public async Task Each_registration_opens_its_own_org()
    {
        await using var app = new TestApp();
        var client = app.CreateClient();

        var alice = await AuthApi.SignUp(client, "alice@example.com");
        var bob = await AuthApi.SignUp(client, "bob@example.com");

        var (aliceOrg, bobOrg) = (OrgOf(alice.AccessToken), OrgOf(bob.AccessToken));
        Assert.NotEqual(Guid.Empty, aliceOrg);
        Assert.NotEqual(Guid.Empty, bobOrg);
        Assert.NotEqual(aliceOrg, bobOrg);
    }

    [Fact(DisplayName = "FM-12: a user signs in whatever org the request resolves to")]
    public async Task Sign_in_is_global()
    {
        var store = Guid.NewGuid().ToString();
        await Seed(store, Guid.NewGuid(), "a@example.com", Hasher.Hash("password1"));

        await using var otherOrg = NewDb(store, new FixedTenant(Guid.NewGuid()));
        var tokens = new AccessTokens("test-secret-for-jwt-signing-please-32+chars", "golden", "golden", TimeProvider.System);
        var sessions = new RefreshSessions(new UserSessionStore(otherOrg), RefreshSessionOptions.Default, TimeProvider.System);
        var result = await Login.Handle(new Login.Input("a@example.com", "password1"), otherOrg, Hasher, sessions, tokens, default);

        Assert.True(result.IsSuccess);
        Assert.NotEmpty(result.Value.AccessToken);
    }

    // Two orgs share a store, but a read as org A never sees org B's rows.
    [Fact(DisplayName = "FM-25: reads never cross the current org")]
    public async Task Reads_never_cross_the_current_org()
    {
        var (orgA, orgB) = (Guid.NewGuid(), Guid.NewGuid());
        var store = Guid.NewGuid().ToString();
        await Seed(store, orgA, "a@org-a.com", PasswordHash.FromStored("x.y"));
        await Seed(store, orgB, "b@org-b.com", PasswordHash.FromStored("x.y"));

        await using var db = NewDb(store, new FixedTenant(orgA));
        var users = await db.Users.ToListAsync();

        var user = Assert.Single(users);
        Assert.Equal("a@org-a.com", user.Email.Value);
        Assert.Equal(orgA, user.OrgId);
    }

    // The filter keeps other orgs out of reads; the stamping keeps them out of writes, however the row was obtained:
    // named on insert, attached by hand, or loaded across the filter.
    [Fact(DisplayName = "FM-27: a request in one org cannot insert, update, or delete another org's rows")]
    public async Task Writes_never_cross_the_current_org()
    {
        var (orgA, orgB) = (Guid.NewGuid(), Guid.NewGuid());
        var store = Guid.NewGuid().ToString();
        var theirs = await Seed(store, orgB, "b@org-b.com", PasswordHash.FromStored("x.y"));

        await using var planting = NewDb(store, new FixedTenant(orgA));
        planting.Users.Add(User.Register(orgB, Email.FromStored("planted@org-b.com"), PasswordHash.FromStored("x.y"), DateTime.UtcNow).Value);
        await Assert.ThrowsAsync<InvalidOperationException>(() => planting.SaveChangesAsync());

        await using var deleting = NewDb(store, new FixedTenant(orgA));
        deleting.Users.Remove(theirs);
        await Assert.ThrowsAsync<InvalidOperationException>(() => deleting.SaveChangesAsync());

        await using var updating = NewDb(store, new FixedTenant(orgA));
        var loaded = await updating.Users.IgnoreQueryFilters().SingleAsync();
        updating.Entry(loaded).Property(u => u.Role).CurrentValue = Role.Admin;
        await Assert.ThrowsAsync<InvalidOperationException>(() => updating.SaveChangesAsync());

        await using var asB = NewDb(store, new FixedTenant(orgB));
        var survivor = Assert.Single(await asB.Users.ToListAsync());
        Assert.Equal("b@org-b.com", survivor.Email.Value);
        Assert.Null(survivor.Role);
    }

    // The request tenant as the app resolves it, for a caller with no access token: no org, so no org's rows.
    [Fact(DisplayName = "FM-26: an anonymous request resolves no org and reads no org's rows")]
    public async Task An_anonymous_request_has_no_org()
    {
        var store = Guid.NewGuid().ToString();
        await Seed(store, Guid.NewGuid(), "a@org-a.com", PasswordHash.FromStored("x.y"));
        var anonymous = new RequestTenant(new ClaimsCurrentUser(null));

        await using var db = NewDb(store, anonymous);

        Assert.Equal(Guid.Empty, anonymous.OrgId);
        Assert.Empty(await db.Users.ToListAsync());
    }

    // Returns the stored user, detached: the context that wrote it is gone.
    private static async Task<User> Seed(string store, Guid org, string email, PasswordHash hash)
    {
        await using var db = NewDb(store, new FixedTenant(org));
        var user = User.Register(org, Email.FromStored(email), hash, DateTime.UtcNow).Value;
        db.Users.Add(user);
        await db.SaveChangesAsync();
        return user;
    }

    private static Guid OrgOf(string accessToken) =>
        Guid.Parse(new JsonWebTokenHandler().ReadJsonWebToken(accessToken).GetClaim("org").Value);

    private static AppDb NewDb(string store, ITenant tenant) =>
        new(new DbContextOptionsBuilder<AppDb>().UseInMemoryDatabase(store).Options, tenant);
}
