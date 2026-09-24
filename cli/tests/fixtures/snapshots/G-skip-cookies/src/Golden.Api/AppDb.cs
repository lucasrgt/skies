using Golden.Api.Tenancy;
using Golden.Api.Modules.Account;
using Microsoft.EntityFrameworkCore;
using Skies.Framework.Auth;

namespace Golden.Api;

/// <summary>The application's single database — one logical store for every module's tables. A module is a
/// bounded context <em>by convention</em>: it writes only its own entities (SKY0009) and references another
/// module by id, never an EF relationship — so it could be carved into its own database later. But all
/// modules share this one DbContext, so a read can join across them in-process (the dashboard case). New
/// modules add their DbSets + configuration here. Email is unique globally (one-human-one-account).</summary>
public class AppDb(DbContextOptions<AppDb> options, ITenant tenant) : TenantDbContext(options, tenant)
{
    public DbSet<User> Users => Set<User>();

    public DbSet<UserSession> UserSessions => Set<UserSession>();

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
    }
}
