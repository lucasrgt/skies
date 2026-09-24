namespace Skies.Framework.EntityFrameworkCore;

/// <summary>The org (tenant) the current unit of work acts in: the seam between how an app resolves a request's org
/// (its policy, e.g. the access token's <c>org</c> claim) and the DbContext that scopes by it. The app implements it
/// in a few lines and registers it scoped; the context reads it through <see cref="ITenantDbContext"/>.</summary>
/// <remarks><see cref="Guid.Empty"/> means "no org": an anonymous request that resolves none reads no tenant-scoped row
/// and cannot save one without naming its org. There is deliberately no default org to fall back to, because a
/// fallback silently pools every unresolved request into one tenant.</remarks>
public interface ITenant
{
    /// <summary>The org this unit of work acts in, or <see cref="Guid.Empty"/> when none is resolved.</summary>
    Guid OrgId { get; }
}

/// <summary>An <see cref="ITenant"/> fixed to one org: for work that runs outside a request (a background job acting
/// for a known org) and for tests that drive a DbContext as a given org.</summary>
/// <param name="OrgId">The org every read is scoped to and every insert is stamped with.</param>
public sealed record FixedTenant(Guid OrgId) : ITenant;

/// <summary>Marks an entity as belonging to one org. A context that calls
/// <see cref="TenantModelBuilderExtensions.ApplyTenantFilters"/> reads only the current org's rows of every marked
/// entity, and <see cref="TenantStamping"/> stamps <see cref="OrgId"/> on insert, so a slice never sets or filters the
/// org by hand.</summary>
/// <remarks>Keep the property encapsulated (<c>public Guid OrgId { get; private set; }</c>): stamping writes it through
/// EF's property metadata, so no slice can assign a foreign org. The one legitimate explicit org is the entity's own
/// factory naming an org it was just created with (registration opening a new org).</remarks>
public interface ITenantScoped
{
    /// <summary>The owning org.</summary>
    Guid OrgId { get; }
}

/// <summary>A DbContext that knows the org it is scoped to. Implemented by the app's own context (one property that
/// reads its <see cref="ITenant"/>), which is what the tenant filter and <see cref="TenantStamping"/> read, per context
/// instance.</summary>
public interface ITenantDbContext
{
    /// <summary>The org every tenant-scoped read filters by and every insert is stamped with; <see cref="Guid.Empty"/>
    /// when the unit of work resolved none.</summary>
    Guid CurrentOrgId { get; }
}
