using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.Mvc.Testing;
using Microsoft.Extensions.DependencyInjection;

namespace Skies.Framework.Testing;

/// <summary>
/// Boots the real application for the spec E2E, exercising the same wiring
/// production runs — routing, model binding, DI, the startup seed — without touching a real database
/// unless you choose one. The framework owns the boot; you own a single hook, <see cref="SwapStores"/>,
/// where you reconfigure services for the test.
///
/// <code>
/// public sealed class TestApp : SkiesWebTest&lt;Program&gt;
/// {
///     protected override void SwapStores(IServiceCollection services) =>
///         services.UseIsolatedInMemory&lt;WalletsDb&gt;();   // from the optional Skies.Framework.Testing.InMemory package
/// }
/// </code>
/// </summary>
/// <typeparam name="TProgram">The application entry-point class the host boots (its <c>Program</c>).</typeparam>
public abstract class SkiesWebTest<TProgram> : WebApplicationFactory<TProgram>
    where TProgram : class
{
    /// <inheritdoc />
    protected override void ConfigureWebHost(IWebHostBuilder builder) =>
        builder.ConfigureServices(SwapStores);

    /// <summary>
    /// Reconfigure the booted app's services for the test — the single hook the framework gives you
    /// over the host. You receive the full <see cref="IServiceCollection"/>, so you may swap anything;
    /// the common case is replacing each module database. Two paths, both yours to choose:
    ///
    /// <list type="bullet">
    /// <item><description>Fast and isolated — reference the <c>Skies.Framework.Testing.InMemory</c> package and
    /// call <c>services.UseIsolatedInMemory&lt;WalletsDb&gt;()</c>.</description></item>
    /// <item><description>A real database, e.g. Testcontainers Postgres — register your own provider:
    /// <c>services.RemoveAll&lt;DbContextOptions&lt;WalletsDb&gt;&gt;();</c> then
    /// <c>services.AddDbContext&lt;WalletsDb&gt;(o => o.UseNpgsql(connectionString));</c>, and manage the
    /// container's lifetime with xUnit's <c>IAsyncLifetime</c>.</description></item>
    /// </list>
    ///
    /// The base holds no database opinion and drags no provider dependency — the in-memory helper is an
    /// opt-in package, and any other provider is just code you write here.
    /// </summary>
    /// <param name="services">The booted application's service collection, free to reconfigure.</param>
    protected abstract void SwapStores(IServiceCollection services);
}
