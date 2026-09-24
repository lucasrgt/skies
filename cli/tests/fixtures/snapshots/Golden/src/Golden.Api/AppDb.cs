using Golden.Api.Modules.Account;
using Microsoft.EntityFrameworkCore;
using Skies.Framework.Auth;
using Skies.Framework.EntityFrameworkCore;
using Golden.Api.Modules.Catalog;

namespace Golden.Api;

/// <summary>The application's single database — one logical store for every module's tables. A module is a
/// bounded context <em>by convention</em>: it writes only its own entities and references another
/// module by id, never an EF relationship — so it could be carved into its own database later. But all
/// modules share this one DbContext, so a read can join across them in-process (the dashboard case). New
/// modules add their DbSets + configuration here. Email is unique globally (one-human-one-account).</summary>
/// <remarks>Tenancy: every <see cref="ITenantScoped"/> entity reads only <see cref="CurrentOrgId"/>'s rows, and an
/// insert without an org is stamped with it on every save, synchronous or not. The org comes from the request (see
/// <c>Tenancy/RequestTenant</c>); an anonymous request has none.</remarks>
public class AppDb(DbContextOptions<AppDb> options, ITenant tenant) : DbContext(options), ITenantDbContext
{
    /// <summary>The org this context reads and writes for the current request.</summary>
    public Guid CurrentOrgId => tenant.OrgId;

    public DbSet<Org> Orgs => Set<Org>();

    public DbSet<User> Users => Set<User>();

    public DbSet<UserSession> UserSessions => Set<UserSession>();

    public DbSet<VerificationToken> VerificationTokens => Set<VerificationToken>();

    public DbSet<Product> Products => Set<Product>();

    protected override void OnConfiguring(DbContextOptionsBuilder builder) =>
        builder.AddInterceptors(TenantStamping.Instance);

    protected override void OnModelCreating(ModelBuilder model)
    {
        base.OnModelCreating(model);

        var user = model.Entity<User>();
        user.Property(u => u.Email).HasConversion(e => e.Value, v => Email.FromStored(v));
        user.Property(u => u.PasswordHash).HasConversion(h => h.Value, v => PasswordHash.FromStored(v));
        user.HasIndex(u => u.Email).IsUnique();

        var session = model.Entity<UserSession>();
        session.HasIndex(s => s.TokenHash);
        session.HasIndex(s => s.FamilyId);

        model.Entity<VerificationToken>().HasIndex(t => new { t.UserId, t.Purpose });

        model.Entity<VerificationToken>().HasIndex(t => t.SecretHash);

        // Last, so it covers every entity registered above.
        model.ApplyTenantFilters(this);
    }
}
