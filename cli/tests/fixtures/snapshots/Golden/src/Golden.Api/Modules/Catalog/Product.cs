namespace Golden.Api.Modules.Catalog;

[Entity]
public class Product : ITenantScoped
{
    public Guid Id { get; private set; }

    public Guid OrgId { get; private set; }
    public string Name { get; private set; } = "";

    /// <summary>The optimistic-concurrency token: renewed by every accepted change, and compared on save
    /// against the version the writer read, so a write based on a stale read fails instead of erasing
    /// the change it never saw.</summary>
    [System.ComponentModel.DataAnnotations.ConcurrencyCheck]
    public Guid Version { get; private set; }

    private Product() { }

    /// <summary>Open a new Product with its identity and fields. Creation returns through
    /// <see cref="EnsureValid"/>, so a Product that breaks an invariant is refused before it
    /// exists.</summary>
    public static Result<Product> Open(Guid id, string name) =>
        new Product { Id = id, Name = name, Version = Guid.NewGuid() }.EnsureValid();

    /// <summary>Change this Product when the proposed values pass <see cref="EnsureValid"/>; a refused
    /// change leaves it as it was. An accepted one issues a new <see cref="Version"/>.</summary>
    public Result<Product> Update(string name)
    {
        var proposed = (Product)MemberwiseClone();
        proposed.Name = name;
        var validation = proposed.EnsureValid();
        if (validation.IsFailure) return validation.Error;
        Name = proposed.Name;
        Version = Guid.NewGuid();
        return this;
    }

    // The one invariant check every factory and change returns through; keep it free of side effects.
    private Result<Product> EnsureValid()
    {
        var validation = new Validation()
            .Check(Id != Guid.Empty, "id", CatalogErrorCodes.IdRequired, "is required");
        if (validation.Failed)
            return validation.ToError();
        return this;
    }
}
