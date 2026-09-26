using System.Net.Http.Json;
using Full.Api;
using Full.Tests;
using Microsoft.AspNetCore.Http.Connections;
using Microsoft.AspNetCore.SignalR;
using Microsoft.AspNetCore.SignalR.Client;

namespace Specs.S9998;

public class Rooms
{
    private sealed record Tokens(string AccessToken);

    private sealed record Message(string Kind, string Text);

    [Fact(DisplayName = "FM-1: another org never hears a room's broadcast, even under the same room key")]
    public async Task Another_org_never_hears_the_room()
    {
        await using var app = new TestApp();
        var alice = await SignIn(app, "alice@example.com");
        await using var sender = await Connect(app, alice);
        await using var colleague = await Connect(app, alice);
        await using var outsider = await Connect(app, await SignIn(app, "bob@example.com"));
        var heard = Listen(colleague);
        var overheard = Listen(outsider);
        foreach (var connection in new[] { sender, colleague, outsider })
            await connection.InvokeAsync("JoinRoom", "general");

        await sender.InvokeAsync("Broadcast", "general", new Message("paid", "invoice paid"));

        Assert.Equal("invoice paid", (await heard.Task.WaitAsync(TimeSpan.FromSeconds(10))).Text);
        await Task.Delay(TimeSpan.FromMilliseconds(500));
        Assert.False(overheard.Task.IsCompleted);
    }

    [Fact(DisplayName = "FM-2: a connection cannot broadcast into a room it has not joined")]
    public async Task Broadcasting_needs_membership()
    {
        await using var app = new TestApp();
        var alice = await SignIn(app, "alice@example.com");
        await using var member = await Connect(app, alice);
        await using var stranger = await Connect(app, alice);
        var heard = Listen(member);
        await member.InvokeAsync("JoinRoom", "general");

        await Assert.ThrowsAsync<HubException>(() => stranger.InvokeAsync("Broadcast", "general", new Message("note", "spam")));

        await Task.Delay(TimeSpan.FromMilliseconds(500));
        Assert.False(heard.Task.IsCompleted);
    }

    [Fact(DisplayName = "FM-4: an oversized or kindless message is refused and never reaches the room")]
    public async Task Messages_are_bounded()
    {
        await using var app = new TestApp();
        var alice = await SignIn(app, "alice@example.com");
        await using var sender = await Connect(app, alice);
        await using var colleague = await Connect(app, alice);
        var heard = Listen(colleague);
        await sender.InvokeAsync("JoinRoom", "general");
        await colleague.InvokeAsync("JoinRoom", "general");

        await Assert.ThrowsAsync<HubException>(() => sender.InvokeAsync("Broadcast", "general", new Message("note", new string('x', 4001))));
        await Assert.ThrowsAsync<HubException>(() => sender.InvokeAsync("Broadcast", "general", new Message("", "hi")));

        await Task.Delay(TimeSpan.FromMilliseconds(500));
        Assert.False(heard.Task.IsCompleted);
    }

    [Fact(DisplayName = "FM-3: a connection without an access token is refused")]
    public async Task An_anonymous_connection_is_refused()
    {
        await using var app = new TestApp();
        app.CreateClient();

        await Assert.ThrowsAnyAsync<Exception>(() => Connect(app, null));
    }

    private static TaskCompletionSource<Message> Listen(HubConnection connection)
    {
        var received = new TaskCompletionSource<Message>(TaskCreationOptions.RunContinuationsAsynchronously);
        connection.On<Message>("Receive", message => received.TrySetResult(message));
        return received;
    }

    private static async Task<HubConnection> Connect(TestApp app, string? accessToken)
    {
        var connection = new HubConnectionBuilder()
            .WithUrl(new Uri(app.Server.BaseAddress, "hubs/payments"), options =>
            {
                options.Transports = HttpTransportType.LongPolling;
                options.HttpMessageHandlerFactory = _ => app.Server.CreateHandler();
                if (accessToken is not null)
                    options.AccessTokenProvider = () => Task.FromResult<string?>(accessToken);
            })
            .Build();
        await connection.StartAsync();
        return connection;
    }

    private static async Task<string> SignIn(TestApp app, string email)
    {
        var client = app.CreateClient();
        (await client.PostAsJsonAsync("/account/register", new { email, password = "password1" })).EnsureSuccessStatusCode();
        var login = await client.PostAsJsonAsync("/account/login", new { email, password = "password1" });
        login.EnsureSuccessStatusCode();
        return (await login.Content.ReadFromJsonAsync<Tokens>(AppJson.Options))!.AccessToken;
    }
}
