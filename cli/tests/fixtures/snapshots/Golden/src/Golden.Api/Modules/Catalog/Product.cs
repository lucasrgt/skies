namespace Golden.Api.Modules.Catalog;

[Entity]
public class Product : ITenantScoped
{
    public Guid Id { get; private set; }

    public Guid OrgId { get; private set; }
    public string Name { get; private set; } = "";

    /// <summary>The optimistic-concurrency token: a concurrent update or delete of the same
    /// row fails loudly with DbUpdateConcurrencyException instead of silently erasing the other
    /// write.</summary>
    [System.ComponentModel.DataAnnotations.Timestamp]
    public byte[]? RowVersion { get; private set; }

    private Product() { }

    /// <summary>Open a new Product with its identity and fields. Creation returns through
    /// <see cref="EnsureValid"/>, so a Product that breaks an invariant is refused before it
    /// exists.</summary>
    public static Result<Product> Open(Guid id, string name) =>
        new Product { Id = id, Name = name }.EnsureValid();

    /// <summary>Validate proposed values before changing this Product.</summary>
    public Result<Product> Update(string name)
    {
        var proposed = (Product)MemberwiseClone();
        proposed.Name = name;
        var validation = proposed.EnsureValid();
        if (validation.IsFailure) return validation.Error;
        Name = proposed.Name;
        return this;
    }

    // Check state without side effects; updates validate a copy before applying changes.
    private Result<Product> EnsureValid()
    {
        var validation = new Validation()
            .Check(Id != Guid.Empty, "id", CatalogErrorCodes.IdRequired, "is required");
        if (validation.Failed)
            return validation.ToError();
        return this;
    }
}
