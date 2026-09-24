namespace Golden.Api.Modules.Billing;

/// <summary>
/// Invoice — a domain entity. It owns its identity and its invariants: no public setter, born through
/// <see cref="Open"/>, and every create and mutate path returns through <see cref="EnsureValid"/>, so a
/// Invoice is never observed or persisted in a broken state.
/// </summary>
[Entity]
public class Invoice
{
    /// <summary>The entity's identity, assigned when it is opened.</summary>
    public Guid Id { get; private set; }

    // Parameterless and private: the constructor EF Core materialises through. The domain opens a
    // Invoice via Open, so there is no public way to construct a blank one.
    private Invoice() { }

    /// <summary>Open a new Invoice with the given identity.</summary>
    public static Result<Invoice> Open(Guid id) =>
        new Invoice { Id = id }.EnsureValid();

    // Add intention-revealing methods for this entity's state transitions here. A change that can
    // violate an invariant returns Result<Invoice> and funnels through EnsureValid; one that cannot
    // fail (a value object already guarantees its input) stays a void method.

    // The single invariant funnel: every create and mutate path returns through here. Add this
    // entity's invariants as Check lines so a broken Invoice can never come to exist.
    private Result<Invoice> EnsureValid()
    {
        var validation = new Validation()
            .Check(Id != Guid.Empty, "id", BillingErrorCodes.IdRequired, "is required");
        if (validation.Failed)
            return validation.ToError();
        return this;
    }
}
