using Golden.Api.Modules.Catalog;
using Golden.Api.Modules.Billing;
using Golden.Api.Modules.Health;

namespace Golden.Api.Modules;

/// <summary>The module registry: the one explicit list of the app's modules, wired on both sides. AddModules
/// registers each module's services, MapModules its routes; adding a module is a line in each (skies g appends
/// them). Nothing is discovered by reflection, so a module missing here is a silent 404, and the doctor checks
/// that none is.</summary>
public static class Modules
{
    public static IServiceCollection AddModules(this IServiceCollection services, IConfiguration configuration)
    {
        HealthModule.AddServices(services, configuration);
        BillingModule.AddServices(services, configuration);
        CatalogModule.AddServices(services, configuration);
        return services;
    }

    public static void MapModules(this WebApplication app)
    {
        HealthModule.Map(app);
        BillingModule.Map(app);
        CatalogModule.Map(app);
    }
}
