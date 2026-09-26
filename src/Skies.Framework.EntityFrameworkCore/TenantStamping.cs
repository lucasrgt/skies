using Microsoft.EntityFrameworkCore;
using Microsoft.EntityFrameworkCore.ChangeTracking;
using Microsoft.EntityFrameworkCore.Diagnostics;

namespace Skies.Framework.EntityFrameworkCore;

/// <summary>The write half of tenancy: before every save, synchronous or asynchronous, every tracked
/// <see cref="ITenantScoped"/> row is checked against the context's <see cref="ITenantDbContext.CurrentOrgId"/>, and an
/// insert without an org is stamped with it. A save that breaks one of the rules below is refused with an
/// <see cref="InvalidOperationException"/> before anything reaches the database.</summary>
/// <remarks>
/// An interceptor rather than a <c>SaveChangesAsync</c> override, because an override has to be repeated for every
/// save overload and the one left out (the synchronous <c>SaveChanges</c>) silently stores unstamped rows. Register it
/// from the context: <c>protected override void OnConfiguring(DbContextOptionsBuilder options) =&gt;
/// options.AddInterceptors(TenantStamping.Instance);</c>. It is stateless and reads the org from the context it is
/// saving, so one instance serves every context.
/// <para><b>A unit of work acting in an org</b> (a signed-in request, a <see cref="FixedTenant"/> for a known org)
/// writes only that org's rows. An insert that names another org is refused, and so is an update or delete of a row
/// whose stored org is another org's. The stored org is the entry's original value, so a detached row attached by hand
/// (<c>db.Remove(row)</c>, <c>db.Update(row)</c>) is caught as surely as a queried one. The read filter keeps other
/// orgs' rows out of queries; this keeps them out of writes, however the row was obtained.</para>
/// <para><b>A unit of work with no org</b> (<see cref="FixedTenant.System"/>, or an anonymous request) is the system
/// scope, the one explicit escape. It reads no tenant-scoped row through the filter, so a row it changes was loaded on
/// purpose across the filter (<c>IgnoreQueryFilters</c>, which the doctor's SKY0030 flags and asks a reason for) or
/// attached by hand. It may change such rows, and it may insert a row only when the row names its org itself: that is
/// how registration stores a user in the org it has just opened, and how a job writes for a given org.</para>
/// <para>In every scope a row keeps the org it was inserted with: moving a row to another org is refused.</para>
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

        var current = tenant.CurrentOrgId;
        foreach (var entry in context.ChangeTracker.Entries<ITenantScoped>())
        {
            var org = entry.Property(nameof(ITenantScoped.OrgId));
            if (entry.State == EntityState.Added)
                StampInsert(entry, org, current);
            else if (entry.State is EntityState.Modified or EntityState.Deleted)
                CheckExisting(entry, org, current);
        }
    }

    private static void StampInsert(EntityEntry<ITenantScoped> entry, PropertyEntry org, Guid current)
    {
        if (org.CurrentValue is Guid named && named != Guid.Empty)
        {
            if (current != Guid.Empty && named != current)
                throw new InvalidOperationException(
                    $"A {entry.Metadata.ClrType.Name} was inserted into org {named} by a unit of work acting in org " +
                    $"{current}. A request writes only its own org; {OwnScope}");
            return;
        }
        if (current == Guid.Empty)
            throw new InvalidOperationException(
                $"A {entry.Metadata.ClrType.Name} was saved with no org: this unit of work resolved none. Sign the " +
                "request in, or name the org in the entity's factory.");
        org.CurrentValue = current;
    }

    private static void CheckExisting(EntityEntry<ITenantScoped> entry, PropertyEntry org, Guid current)
    {
        if (entry.State == EntityState.Modified && org.IsModified && !Equals(org.OriginalValue, org.CurrentValue))
            throw new InvalidOperationException(
                $"A {entry.Metadata.ClrType.Name} cannot move to another org; its OrgId is fixed at insert.");
        var stored = org.OriginalValue as Guid? ?? Guid.Empty;
        if (current != Guid.Empty && stored != current)
            throw new InvalidOperationException(
                $"A {entry.Metadata.ClrType.Name} of org {stored} was " +
                $"{(entry.State == EntityState.Deleted ? "deleted" : "updated")} by a unit of work acting in org " +
                $"{current}. A request changes only its own org's rows; {OwnScope}");
    }

    private const string OwnScope =
        "work that writes for another org runs in a scope of its own: a context over FixedTenant(thatOrg), or over " +
        "FixedTenant.System for work that spans orgs.";
}
