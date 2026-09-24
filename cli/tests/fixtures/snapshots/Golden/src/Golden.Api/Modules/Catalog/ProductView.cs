namespace Golden.Api.Modules.Catalog;

/// <summary>A Product as the API returns it. The slices answer with this record, never the entity,
/// so the owning org and the concurrency token stay off the wire,
/// and the contract moves only when this record does.</summary>
public record ProductView(Guid Id, string Name)
{
    /// <summary>Projects a loaded Product.</summary>
    public static ProductView From(Product e) => new(e.Id, e.Name);
}
