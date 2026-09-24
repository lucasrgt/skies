using Skies.Framework.Abstractions;

namespace Golden.Api.Modules.Billing;

/// <summary>Owns the module's services and routes.</summary>
[Module]
public static class BillingModule
{
    /// <summary>The module's own service registration — empty until a slice needs DI; the seam is uniform.</summary>
    public static IServiceCollection AddServices(IServiceCollection services, IConfiguration configuration) =>
        services;

    public static void Map(IEndpointRouteBuilder app)
    {
        // The route group makes authorization explicit for its slices.
        //   var billing = app.MapGroup("/billing").RequireAuthorization(); // or .AllowAnonymous()
        //   <Slice>.Map(billing);
        var billing = app.MapGroup("/billing").RequireAuthorization();
        CreateInvoice.Map(billing);
        GetInvoice.Map(billing);
    }
}
