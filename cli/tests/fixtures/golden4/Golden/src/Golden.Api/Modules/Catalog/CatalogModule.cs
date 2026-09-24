using Skies.Framework.Abstractions;

namespace Golden.Api.Modules.Catalog;

/// <summary>The Catalog module's wiring root — it owns both halves of its composition: AddServices (its
/// own DI) and Map (its routes, under /catalog). The module registry calls both; the
/// doctor (SKY0015/SKY0016) checks the shape and that it is registered.</summary>
[Module]
public static class CatalogModule
{
    /// <summary>The module's own service registration — empty until a slice needs DI; the seam is uniform.</summary>
    public static IServiceCollection AddServices(IServiceCollection services, IConfiguration configuration) =>
        services;

    public static void Map(IEndpointRouteBuilder app)
    {
        // Register this module's slices here as you generate them. The group carries the module's
        // authorization decision — SKY0022 wants it explicit either way:
        //   var catalog = app.MapGroup("/catalog").RequireAuthorization(); // or .AllowAnonymous()
        //   <Slice>.Map(catalog);
        ListProduct.Map(catalog);
        LookupProduct.Map(catalog);
        CreateProduct.Map(catalog);
        UpdateProduct.Map(catalog);
        DeleteProduct.Map(catalog);
    }
}
