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

    public Guid TenantId { get; set; }
    public string Name { get; private set; } = "";

    // Parameterless and private: the constructor EF Core materialises through. The domain opens a
    // Product via Open, so there is no public way to construct a blank one.
    private Product() { }

    /// <summary>Open a new Product with the given identity.</summary>
    public static Result<Product> Open(Guid id) =>
        new Product { Id = id }.EnsureValid();

    // Add intention-revealing methods for this entity's state transitions here. A change that can
    // violate an invariant returns Result<Product> and funnels through EnsureValid; one that cannot
    // fail (a value object already guarantees its input) stays a void method.

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
