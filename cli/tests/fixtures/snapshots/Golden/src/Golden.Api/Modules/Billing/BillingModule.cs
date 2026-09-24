using Skies.Framework.Abstractions;

namespace Golden.Api.Modules.Billing;

/// <summary>The Billing module's wiring root — it owns both halves of its composition: AddServices (its
/// own DI) and Map (its routes, under /billing). The module registry calls both; the
/// doctor (SKY0015/SKY0016) checks the shape and that it is registered.</summary>
[Module]
public static class BillingModule
{
    /// <summary>The module's own service registration — empty until a slice needs DI; the seam is uniform.</summary>
    public static IServiceCollection AddServices(IServiceCollection services, IConfiguration configuration) =>
        services;

    public static void Map(IEndpointRouteBuilder app)
    {
        // This module's slices map onto its group; `skies g slice` adds each line, declaring the group (failing
        // closed) when there is none. The group carries the module's authorization decision — SKY0022 wants it
        // explicit either way, and a slice mapped here inherits it:
        //   var billing = app.MapGroup("/billing").RequireAuthorization(); // or .AllowAnonymous()
        //   <Slice>.Map(billing);
        var billing = app.MapGroup("/billing").RequireAuthorization();
        CreateInvoice.Map(billing);
        GetInvoice.Map(billing);
    }
}
