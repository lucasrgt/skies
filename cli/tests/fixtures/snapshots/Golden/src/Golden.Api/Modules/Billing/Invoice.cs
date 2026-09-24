namespace Golden.Api.Modules.Billing;

/// <summary>
/// Invoice owns its identity and its invariants: it has no public setter, is born through <see cref="Open"/>,
/// and every create and mutate path returns through <see cref="EnsureValid"/>, so it is never observed or
/// persisted in a broken state.
/// </summary>
[Entity]
public class Invoice
{
    /// <summary>The entity's identity, assigned when it is opened.</summary>
    public Guid Id { get; private set; }

    // EF Core materializes through this constructor; the domain creates one only through Open.
    private Invoice() { }

    /// <summary>Open a new Invoice with the given identity.</summary>
    public static Result<Invoice> Open(Guid id) =>
        new Invoice { Id = id }.EnsureValid();

    private Result<Invoice> EnsureValid()
    {
        var validation = new Validation()
            .Check(Id != Guid.Empty, "id", BillingErrorCodes.IdRequired, "is required");
        if (validation.Failed)
            return validation.ToError();
        return this;
    }
}
