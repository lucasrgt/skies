namespace Golden.Api.Modules.Catalog;

/// <summary>
/// Product owns its identity and its invariants: it has no public setter, is born through <see cref="Open"/>,
/// and every create and mutate path returns through <see cref="EnsureValid"/>, so it is never observed or
/// persisted in a broken state.
/// </summary>
[Entity]
public class Product : ITenantScoped
{
    /// <summary>The entity's identity, assigned when it is opened.</summary>
    public Guid Id { get; private set; }

    public Guid OrgId { get; private set; }
    public string Name { get; private set; } = "";

    /// <summary>The optimistic-concurrency token: a concurrent update or delete of the same
    /// row fails loudly with DbUpdateConcurrencyException instead of silently erasing the other
    /// write.</summary>
    [System.ComponentModel.DataAnnotations.Timestamp]
    public byte[]? RowVersion { get; private set; }

    // EF Core materializes through this constructor; the domain creates one only through Open.
    private Product() { }

    /// <summary>Open a new Product with its identity and fields. Creation returns through
    /// <see cref="EnsureValid"/>, so a Product that breaks an invariant is refused before it
    /// exists.</summary>
    public static Result<Product> Open(Guid id, string name) =>
        new Product { Id = id, Name = name }.EnsureValid();

    /// <summary>Replace the Product's fields in one step. It returns through
    /// <see cref="EnsureValid"/>; the caller saves only on success, so a refused change never
    /// reaches the database. Give it the domain's own verb once the entity has one.</summary>
    public Result<Product> Update(string name)
    {
        Name = name;
        return EnsureValid();
    }

    private Result<Product> EnsureValid()
    {
        var validation = new Validation()
            .Check(Id != Guid.Empty, "id", CatalogErrorCodes.IdRequired, "is required");
        if (validation.Failed)
            return validation.ToError();
        return this;
    }
}
