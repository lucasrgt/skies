namespace Golden.Api.Modules.Billing;

[Entity]
public class Invoice
{
    public Guid Id { get; private set; }

    private Invoice() { }

    /// <summary>Open a new Invoice with the given identity.</summary>
    public static Result<Invoice> Open(Guid id) =>
        new Invoice { Id = id }.EnsureValid();

    // Check state without side effects; updates validate a copy before applying changes.
    private Result<Invoice> EnsureValid()
    {
        var validation = new Validation()
            .Check(Id != Guid.Empty, "id", BillingErrorCodes.IdRequired, "is required");
        if (validation.Failed)
            return validation.ToError();
        return this;
    }
}
