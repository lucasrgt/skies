namespace Golden.Api.Modules.Billing;

/// <summary>The Billing module's error codes — stable, namespaced i18n keys the frontend localizes from.
/// Every Error/Check references a const here, so the full set stays discoverable: AddSkiesOpenApi
/// enumerates it into the OpenAPI ErrorBody.code schema for the typed client.</summary>
public static class BillingErrorCodes
{
    /// <summary>The id input is required.</summary>
    public const string IdRequired = "id.required";
}
