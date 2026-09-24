using Skies.Framework.Starter.Api.Modules.Health;
using Assay.Net;

namespace Skies.Framework.Starter.Tests.Modules.Health;

public class PingTests
{
    [AVP(typeof(Ping), "echoes-input")]
    [Fact]
    public async Task Ping_echoes_the_message()
    {
        var result = await Ping.Handle(new Ping.Input("hello"), default);

        Assert.True(result.IsSuccess);
        Assert.Equal("hello", result.Value.Message);
    }
}
