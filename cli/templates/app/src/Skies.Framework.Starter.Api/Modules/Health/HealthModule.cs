namespace Skies.Framework.Starter.Api.Modules.Health;

/// <summary>The Health module's wiring: its own services and its routes under /health, both called from the
/// module registry (Modules/Modules.cs).</summary>
[Module]
public static class HealthModule
{
    public static IServiceCollection AddServices(IServiceCollection services, IConfiguration configuration) =>
        services;

    public static void Map(IEndpointRouteBuilder app)
    {
        // A liveness probe must answer before anyone signs in, so the whole group is public on purpose.
        var health = app.MapGroup("/health").AllowAnonymous();
        Ping.Map(health);
    }
}
