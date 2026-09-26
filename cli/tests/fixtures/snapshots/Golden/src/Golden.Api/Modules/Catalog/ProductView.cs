namespace Golden.Api.Modules.Catalog;

/// <summary>A Product as the API returns it: the slices answer with this record, never the entity, so the
/// contract moves only when this record does. <c>Version</c> is the concurrency token a client sends back to
/// update or delete the row.
/// The owning org stays off the wire.</summary>
public record ProductView(Guid Id, string Name, Guid Version)
{
    public static ProductView From(Product e) => new(e.Id, e.Name, e.Version);
}
