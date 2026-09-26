namespace Golden.Api.Modules.Billing;

/// <summary>The Billing module's wiring: its own services and its routes under /billing, both called from
/// the module registry (Modules/Modules.cs).</summary>
[Module]
public static class BillingModule
{
    public static IServiceCollection AddServices(IServiceCollection services, IConfiguration configuration) =>
        services;

    public static void Map(IEndpointRouteBuilder app)
    {
        var billing = app.MapGroup("/billing").RequireAuthorization();
        CreateInvoice.Map(billing);
        GetInvoice.Map(billing);
    }
}
