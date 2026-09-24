using Microsoft.AspNetCore.Hosting;
using Golden.Tests;

namespace Specs.S0001;

public class Deployment
{
    [Fact(DisplayName = "FM-21: local providers cannot start in Production")]
    public void Production_requires_configuration() => CannotStart("Production");

    [Fact(DisplayName = "FM-21: local providers cannot start in Staging")]
    public void Staging_requires_configuration() => CannotStart("Staging");

    private static void CannotStart(string environment)
    {
        using var app = new TestApp().WithWebHostBuilder(builder => builder.UseEnvironment(environment));
        var error = Assert.Throws<InvalidOperationException>(() => app.CreateClient());
        Assert.Contains("Configure a persistent AppDb provider", error.Message, StringComparison.Ordinal);
    }
}
