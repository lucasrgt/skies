namespace Golden.Api.Modules.Billing;

/// <summary>The Billing module's error codes: stable keys the frontend localizes from. They live here, as
/// constants, so the full set is enumerable into the OpenAPI contract and the typed client.</summary>
public static class BillingErrorCodes
{
    /// <summary>CreateInvoice is scaffolded but not implemented yet; remove this code when it is.</summary>
    public const string CreateInvoiceNotImplemented = "billing.create_invoice_not_implemented";

    /// <summary>GetInvoice is scaffolded but not implemented yet; remove this code when it is.</summary>
    public const string GetInvoiceNotImplemented = "billing.get_invoice_not_implemented";

    /// <summary>The id is required (entity invariant).</summary>
    public const string IdRequired = "id.required";
}
