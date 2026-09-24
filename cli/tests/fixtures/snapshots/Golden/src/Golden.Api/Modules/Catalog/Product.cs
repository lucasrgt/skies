namespace Golden.Api.Modules.Catalog;

/// <summary>
/// Product — a domain entity. It owns its identity and its invariants: no public setter, born through
/// <see cref="Open"/>, and every create and mutate path returns through <see cref="EnsureValid"/>, so a
/// Product is never observed or persisted in a broken state.
/// </summary>
[Entity]
public class Product : ITenantScoped
{
    /// <summary>The entity's identity, assigned when it is opened.</summary>
    public Guid Id { get; private set; }

    public Guid OrgId { get; private set; }
    public string Name { get; private set; } = "";

    /// <summary>The optimistic-concurrency token (SKY0026): a concurrent update or delete of the same
    /// row fails loudly with DbUpdateConcurrencyException instead of silently erasing the other
    /// write.</summary>
    [System.ComponentModel.DataAnnotations.Timestamp]
    public byte[]? RowVersion { get; private set; }

    // Parameterless and private: the constructor EF Core materialises through. The domain opens a
    // Product via Open, so there is no public way to construct a blank one.
    private Product() { }

    /// <summary>Open a new Product with its identity and fields. Creation returns through
    /// <see cref="EnsureValid"/>, so a Product that breaks an invariant is refused before it
    /// exists.</summary>
    public static Result<Product> Open(Guid id, string name) =>
        new Product { Id = id, Name = name }.EnsureValid();

    // Add intention-revealing methods for this entity's state transitions here. A change that can
    // violate an invariant returns Result<Product> and funnels through EnsureValid; one that cannot
    // fail (a value object already guarantees its input) stays a void method.

    /// <summary>Replace the Product's fields in one step. It returns through
    /// <see cref="EnsureValid"/>; the caller saves only on success, so a refused change never
    /// reaches the database. Give it the domain's own verb once the entity has one.</summary>
    public Result<Product> Update(string name)
    {
        Name = name;
        return EnsureValid();
    }

    // The single invariant funnel: every create and mutate path returns through here. Add this
    // entity's invariants as Check lines so a broken Product can never come to exist.
    private Result<Product> EnsureValid()
    {
        var validation = new Validation()
            .Check(Id != Guid.Empty, "id", CatalogErrorCodes.IdRequired, "is required");
        if (validation.Failed)
            return validation.ToError();
        return this;
    }
}
