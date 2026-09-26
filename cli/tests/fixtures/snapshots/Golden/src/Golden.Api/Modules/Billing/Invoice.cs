namespace Golden.Api.Modules.Billing;

[Entity]
public class Invoice
{
    public Guid Id { get; private set; }

    private Invoice() { }

    /// <summary>Open a new Invoice with the given identity.</summary>
    public static Result<Invoice> Open(Guid id) =>
        new Invoice { Id = id }.EnsureValid();

    // The one invariant check every factory and change returns through; keep it free of side effects.
    private Result<Invoice> EnsureValid()
    {
        var validation = new Validation()
            .Check(Id != Guid.Empty, "id", BillingErrorCodes.IdRequired, "is required");
        if (validation.Failed)
            return validation.ToError();
        return this;
    }
}
