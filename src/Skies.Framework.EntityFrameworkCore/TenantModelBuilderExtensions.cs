using System.Linq.Expressions;
using Microsoft.EntityFrameworkCore;

namespace Skies.Framework.EntityFrameworkCore;

/// <summary>The read half of tenancy: a query filter that scopes every <see cref="ITenantScoped"/> entity to the
/// context's <see cref="ITenantDbContext.CurrentOrgId"/>.</summary>
public static class TenantModelBuilderExtensions
{
    /// <summary>The name of the query filter this applies, so a query can lift exactly this one
    /// (<c>IgnoreQueryFilters([TenantModelBuilderExtensions.FilterName])</c>) and an app's own named filters (soft
    /// delete) compose with it instead of replacing it.</summary>
    public const string FilterName = "tenant";

    /// <summary>Filter every <see cref="ITenantScoped"/> entity in the model to <paramref name="context"/>'s current
    /// org. Call it once from <c>OnModelCreating</c>, passing the context itself: <c>model.ApplyTenantFilters(this)</c>.
    /// </summary>
    /// <remarks>
    /// This walks the model, not the assemblies: it reads the entity types the context itself registered and applies
    /// the one filter to each marked root type. That scan is kept on purpose, and kept this narrow. A per-entity opt-in
    /// fails open: the entity whose filter someone forgets reads every org's rows, the one tenancy bug that must not be
    /// expressible. The filter is rooted at the context instance, so EF re-reads the current org for each query while
    /// the model itself is built once and cached. A context whose unit of work resolved no org reads no row at all.
    /// </remarks>
    /// <param name="model">The model being built.</param>
    /// <param name="context">The context being configured (<c>this</c>), which supplies the current org per query.</param>
    /// <returns>The same model builder.</returns>
    public static ModelBuilder ApplyTenantFilters(this ModelBuilder model, ITenantDbContext context)
    {
        ArgumentNullException.ThrowIfNull(model);
        if (context is not DbContext)
            throw new ArgumentException("Pass the DbContext being configured (this), so each query reads its own org.", nameof(context));

        // EF substitutes the context constant with the context running the query; the property is read through the
        // interface, so the app's context needs no member beyond CurrentOrgId.
        var currentOrg = Expression.Property(
            Expression.Convert(Expression.Constant(context), typeof(ITenantDbContext)),
            nameof(ITenantDbContext.CurrentOrgId));
        var scoped = model.Model.GetEntityTypes()
            .Where(type => type.BaseType is null && !type.IsOwned() && typeof(ITenantScoped).IsAssignableFrom(type.ClrType))
            .ToList();
        foreach (var type in scoped)
        {
            var row = Expression.Parameter(type.ClrType, "row");
            var sameOrg = Expression.Equal(Expression.Property(row, nameof(ITenantScoped.OrgId)), currentOrg);
            model.Entity(type.ClrType).HasQueryFilter(FilterName, Expression.Lambda(sameOrg, row));
        }
        return model;
    }
}
