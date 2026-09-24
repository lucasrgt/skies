namespace Golden.Api.Modules.Catalog;

/// <summary>The Catalog module's error codes: stable keys the frontend localizes from. They live here, as
/// constants, so the full set is enumerable into the OpenAPI contract and the typed client.</summary>
public static class CatalogErrorCodes
{
    /// <summary>The id is required (entity invariant).</summary>
    public const string IdRequired = "catalog.id_required";

    /// <summary>No Product exists for the given id.</summary>
    public const string ProductNotFound = "catalog.product_not_found";

    /// <summary>The Product changed since the client read it; reload it and apply the change again.</summary>
    public const string ProductChanged = "catalog.product_changed";
}
