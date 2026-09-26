using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Options;
using Skies.Framework.Auth;

namespace Skies.Framework.Auth.Tests;

// A token signed with a key anyone can read is a token anyone can forge. Outside Development the host must refuse to
// start on a missing, short, or development secret, and say what to configure; Development keeps the checked-in key.
public class JwtSecretPolicyTests
{
    private const string Strong = "a-random-deployment-secret-of-well-over-thirty-two-bytes";

    public static TheoryData<string?, string> Refused => new()
    {
        { null, "is missing" },
        { "", "is missing" },
        { "short-secret", "shorter than 32 bytes" },
        { SkiesAuthOptions.DevelopmentSecret, "development key" },
        { "myapp-local-development-key-not-for-deployment", "development key" },
    };

    [Theory]
    [MemberData(nameof(Refused))]
    public async Task Outside_development_a_weak_secret_stops_the_host(string? secret, string reason)
    {
        using var host = Host("Production", secret);

        var error = await Assert.ThrowsAsync<OptionsValidationException>(() => host.StartAsync());

        Assert.Contains(reason, error.Message, StringComparison.Ordinal);
        Assert.Contains("Jwt:Secret", error.Message, StringComparison.Ordinal);
        Assert.Contains("'Production'", error.Message, StringComparison.Ordinal);
    }

    [Fact]
    public async Task Staging_is_outside_development_too()
    {
        using var host = Host("Staging", SkiesAuthOptions.DevelopmentSecret);

        await Assert.ThrowsAsync<OptionsValidationException>(() => host.StartAsync());
    }

    [Fact]
    public async Task A_strong_secret_starts_outside_development()
    {
        using var host = Host("Production", Strong);

        await host.StartAsync();
        await host.StopAsync();
    }

    [Fact]
    public async Task Development_starts_with_the_development_key()
    {
        using var host = Host("Development", SkiesAuthOptions.DevelopmentSecret);

        await host.StartAsync();
        await host.StopAsync();
    }

    [Fact]
    public async Task Development_still_refuses_a_missing_secret()
    {
        using var host = Host("Development", null);

        var error = await Assert.ThrowsAsync<OptionsValidationException>(() => host.StartAsync());

        Assert.Contains("is missing", error.Message, StringComparison.Ordinal);
    }

    [Fact]
    public async Task The_bare_token_registration_carries_the_same_rule()
    {
        var builder = Microsoft.Extensions.Hosting.Host.CreateApplicationBuilder(
            new HostApplicationBuilderSettings { EnvironmentName = "Production" });
        builder.Services.AddSingleton(TimeProvider.System);
        builder.Services.AddJwtAccessTokens(SkiesAuthOptions.DevelopmentSecret, "myapp", "myapp");
        using var host = builder.Build();

        await Assert.ThrowsAsync<OptionsValidationException>(() => host.StartAsync());
    }

    private static IHost Host(string environment, string? secret)
    {
        var builder = Microsoft.Extensions.Hosting.Host.CreateApplicationBuilder(
            new HostApplicationBuilderSettings { EnvironmentName = environment });
        builder.Services.AddSkiesAuth<ListRefreshStore>(new SkiesAuthOptions(secret!, "myapp", "myapp"));
        return builder.Build();
    }
}
