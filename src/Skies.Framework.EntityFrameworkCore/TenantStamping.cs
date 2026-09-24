using Microsoft.EntityFrameworkCore;
using Microsoft.EntityFrameworkCore.ChangeTracking;
using Microsoft.EntityFrameworkCore.Diagnostics;

namespace Skies.Framework.EntityFrameworkCore;

/// <summary>The write half of tenancy: before every save, synchronous or asynchronous, an inserted
/// <see cref="ITenantScoped"/> row without an org is stamped with the context's
/// <see cref="ITenantDbContext.CurrentOrgId"/>. A save that would store a row with no org, or move a row to another
/// org, is refused with an <see cref="InvalidOperationException"/> before anything reaches the database.</summary>
/// <remarks>
/// An interceptor rather than a <c>SaveChangesAsync</c> override, because an override has to be repeated for every
/// save overload and the one left out (the synchronous <c>SaveChanges</c>) silently stores unstamped rows. Register it
/// from the context: <c>protected override void OnConfiguring(DbContextOptionsBuilder options) =&gt;
/// options.AddInterceptors(TenantStamping.Instance);</c>. It is stateless and reads the org from the context it is
/// saving, so one instance serves every context.
/// <para>A row whose own factory already named its org keeps it: that is how registration stores a user in the org it
/// has just opened while the anonymous request itself resolves none. Every other insert takes the caller's org.</para>
/// </remarks>
public sealed class TenantStamping : SaveChangesInterceptor
{
    /// <summary>The shared instance to register on a context.</summary>
    public static TenantStamping Instance { get; } = new();

    private TenantStamping() { }

    /// <inheritdoc />
    public override InterceptionResult<int> SavingChanges(DbContextEventData eventData, InterceptionResult<int> result)
    {
        Stamp(eventData.Context);
        return result;
    }

    /// <inheritdoc />
    public override ValueTask<InterceptionResult<int>> SavingChangesAsync(
        DbContextEventData eventData, InterceptionResult<int> result, CancellationToken cancellationToken = default)
    {
        Stamp(eventData.Context);
        return ValueTask.FromResult(result);
    }

    /// <summary>Stamp and check every tracked <see cref="ITenantScoped"/> row of <paramref name="context"/>, exactly as
    /// a save does. Public so a context that cannot take an interceptor can call it from its own save path.</summary>
    /// <param name="context">The context about to save; it must implement <see cref="ITenantDbContext"/>.</param>
    public static void Stamp(DbContext? context)
    {
        if (context is null)
            return;
        if (context is not ITenantDbContext tenant)
            throw new InvalidOperationException(
                $"{context.GetType().Name} saves tenant-scoped rows but does not implement {nameof(ITenantDbContext)}.");

        foreach (var entry in context.ChangeTracker.Entries<ITenantScoped>())
        {
            var org = entry.Property(nameof(ITenantScoped.OrgId));
            if (entry.State == EntityState.Added)
                StampInsert(entry, org, tenant.CurrentOrgId);
            else if (entry.State == EntityState.Modified && org.IsModified && !Equals(org.OriginalValue, org.CurrentValue))
                throw new InvalidOperationException(
                    $"A {entry.Metadata.ClrType.Name} cannot move to another org; its OrgId is fixed at insert.");
        }
    }

    private static void StampInsert(EntityEntry<ITenantScoped> entry, PropertyEntry org, Guid currentOrg)
    {
        if (org.CurrentValue is Guid named && named != Guid.Empty)
            return;
        if (currentOrg == Guid.Empty)
            throw new InvalidOperationException(
                $"A {entry.Metadata.ClrType.Name} was saved with no org: this unit of work resolved none. Sign the " +
                "request in, or name the org in the entity's factory.");
        org.CurrentValue = currentOrg;
    }
}
