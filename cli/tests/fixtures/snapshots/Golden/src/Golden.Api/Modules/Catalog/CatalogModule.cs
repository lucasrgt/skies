namespace Golden.Api.Modules.Catalog;

/// <summary>The Catalog module's wiring: its own services and its routes under /catalog, both called from
/// the module registry (Modules/Modules.cs).</summary>
[Module]
public static class CatalogModule
{
    public static IServiceCollection AddServices(IServiceCollection services, IConfiguration configuration) =>
        services;

    public static void Map(IEndpointRouteBuilder app)
    {
        var catalog = app.MapGroup("/catalog").RequireAuthorization();
        ListProduct.Map(catalog);
        LookupProduct.Map(catalog);
        CreateProduct.Map(catalog);
        UpdateProduct.Map(catalog);
        DeleteProduct.Map(catalog);
    }
}
