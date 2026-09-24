using Skies.Framework.Abstractions;
using Skies.Framework.Identity;
using Skies.Framework.Mail;
using Skies.Framework.Sms;

namespace Skies.Framework.Ports.Tests;

[Collection("process console")]
public class DevelopmentAdapterTests
{
    [Fact]
    public async Task Fake_identity_accepts_a_visible_development_token_as_the_external_user()
    {
        var result = await new FakeExternalIdentity().VerifyAsync("dev@example.test");

        Assert.True(result.IsSuccess);
        Assert.Equal(new ExternalUser("fake", "dev@example.test", "dev@example.test"), result.Value);
    }

    [Theory]
    [InlineData("")]
    [InlineData("   ")]
    public async Task Fake_identity_rejects_an_empty_token(string token)
    {
        Result<ExternalUser> result = await new FakeExternalIdentity().VerifyAsync(token);

        Assert.True(result.IsFailure);
        Assert.Equal(ErrorKind.Unauthorized, result.Error.Kind);
        Assert.Equal("identity.invalid_token", result.Error.Code);
    }

    [Fact]
    public async Task Console_mail_keeps_the_recipient_subject_and_body_visible()
    {
        var text = await CaptureConsole(() =>
            new ConsoleEmailSender().SendAsync(new EmailMessage("a@example.test", "Welcome", "Open the link")));

        Assert.Contains("to=a@example.test subject=\"Welcome\"", text);
        Assert.Contains("Open the link", text);
    }

    [Fact]
    public async Task Console_sms_keeps_the_recipient_and_message_visible()
    {
        var text = await CaptureConsole(() =>
            new ConsoleSmsSender().SendAsync("+15550000000", "Code 123456"));

        Assert.Contains("to=+15550000000: Code 123456", text);
    }

    private static async Task<string> CaptureConsole(Func<Task> action)
    {
        var previous = Console.Out;
        using var output = new StringWriter();
        try
        {
            Console.SetOut(output);
            await action();
            return output.ToString();
        }
        finally
        {
            Console.SetOut(previous);
        }
    }
}

[CollectionDefinition("process console", DisableParallelization = true)]
public sealed class ProcessConsoleCollection { }
