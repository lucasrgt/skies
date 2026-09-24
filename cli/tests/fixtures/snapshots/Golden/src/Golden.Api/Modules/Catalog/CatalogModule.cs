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
        // This module's slices map onto its group; `skies g slice` adds each line, declaring the group (failing
        // closed) when there is none. The group carries the module's authorization decision — SKY0022 wants it
        // explicit either way, and a slice mapped here inherits it:
        //   var catalog = app.MapGroup("/catalog").RequireAuthorization(); // or .AllowAnonymous()
        //   <Slice>.Map(catalog);
        var catalog = app.MapGroup("/catalog").RequireAuthorization();
        ListProduct.Map(catalog);
        LookupProduct.Map(catalog);
        CreateProduct.Map(catalog);
        UpdateProduct.Map(catalog);
        DeleteProduct.Map(catalog);
    }
}
