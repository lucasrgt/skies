namespace Golden.Api.Modules.Catalog;

/// <summary>The Catalog module's error codes — stable, namespaced i18n keys the frontend localizes from.
/// Every Error/Check references a const here (SKY0018), so the full set stays discoverable: AddSkiesOpenApi
/// enumerates it into the OpenAPI ErrorBody.code schema for the typed client.</summary>
public static class CatalogErrorCodes
{
    /// <summary>The id is required (entity invariant).</summary>
    public const string IdRequired = "id.required";

    /// <summary>No Product exists for the given id.</summary>
    public const string ProductNotFound = "product.not_found";
}
