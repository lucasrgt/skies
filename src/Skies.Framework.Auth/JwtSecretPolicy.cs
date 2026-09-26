using System.Text;
using Microsoft.AspNetCore.Authentication.JwtBearer;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Options;

namespace Skies.Framework.Auth;

/// <summary>The signing secret's deployment rule, checked when the host starts (<c>ValidateOnStart</c>): outside
/// Development the app refuses to start on a secret that is missing, shorter than 32 bytes, or a development key.
/// A token signed with a key anyone can read is a token anyone can forge, so this is a boot failure, not a log line.
/// Development keeps the checked-in key so a fresh clone runs (a missing secret is refused there too: it signs
/// nothing); the build-time OpenAPI pass signs nothing, so it is not refused either.</summary>
internal sealed class JwtSecretPolicy(string? secret, IHostEnvironment? environment) : IValidateOptions<JwtBearerOptions>
{
    // Earlier generated apps set "<app>-local-development-key-not-for-deployment"; every such key is public.
    private const string DevelopmentKeySuffix = "-local-development-key-not-for-deployment";

    public ValidateOptionsResult Validate(string? name, JwtBearerOptions options)
    {
        if (name != JwtBearerDefaults.AuthenticationScheme || GeneratingOpenApiDocument)
            return ValidateOptionsResult.Success;
        // A missing secret signs nothing anywhere; the other problems are only Development's to tolerate.
        var problem = Problem(secret);
        if (environment?.IsDevelopment() == true && !string.IsNullOrWhiteSpace(secret))
            return ValidateOptionsResult.Success;
        return problem is null
            ? ValidateOptionsResult.Success
            : ValidateOptionsResult.Fail(
                $"The JWT signing secret {problem}, and the app is running in " +
                $"'{environment?.EnvironmentName ?? "an unknown environment"}'. Configure a random secret of at least " +
                $"{SkiesAuthOptions.MinSecretBytes} bytes, kept out of source control, as SkiesAuthOptions.JwtSecret " +
                "(a generated app reads the Jwt:Secret setting: set the Jwt__Secret environment variable or a secret " +
                "store entry, e.g. from `openssl rand -base64 48`).");
    }

    // `dotnet build` boots the app under the OpenAPI document tool (outside Development, with the development key) only
    // to read its endpoints; nothing is signed there, so the rule is for real starts.
    private static bool GeneratingOpenApiDocument =>
        System.Reflection.Assembly.GetEntryAssembly()?.GetName().Name == "GetDocument.Insider";

    /// <summary>Why <paramref name="secret"/> cannot sign tokens outside Development, or <see langword="null"/>.</summary>
    internal static string? Problem(string? secret)
    {
        if (string.IsNullOrWhiteSpace(secret))
            return "is missing";
        if (secret == SkiesAuthOptions.DevelopmentSecret || secret.EndsWith(DevelopmentKeySuffix, StringComparison.Ordinal))
            return "is the development key, which is public";
        if (Encoding.UTF8.GetByteCount(secret) < SkiesAuthOptions.MinSecretBytes)
            return $"is shorter than {SkiesAuthOptions.MinSecretBytes} bytes";
        return null;
    }
}
