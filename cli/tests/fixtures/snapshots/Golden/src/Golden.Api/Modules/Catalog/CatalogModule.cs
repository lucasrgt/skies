using Skies.Framework.Abstractions;

namespace Golden.Api.Modules.Catalog;

/// <summary>Owns the module's services and routes.</summary>
[Module]
public static class CatalogModule
{
    /// <summary>The module's own service registration — empty until a slice needs DI; the seam is uniform.</summary>
    public static IServiceCollection AddServices(IServiceCollection services, IConfiguration configuration) =>
        services;

    public static void Map(IEndpointRouteBuilder app)
    {
        // The route group makes authorization explicit for its slices.
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
