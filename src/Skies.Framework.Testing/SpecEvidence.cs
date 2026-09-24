using System.Text.Json;
using System.Text.Json.Serialization;

namespace Skies.Framework.Testing;

/// <summary>
/// Drops artifacts into the evidence folder of the spec being proven, so a test can leave behind what a reviewer
/// replays or reads: an Assay verdict, a response body, a captured log. <c>skies proof record</c> and
/// <c>verify</c> set <c>SKIES_EVIDENCE</c> for every runner, and whatever lands there is copied into the spec's
/// <c>evidence/</c> and hashed by its receipt.
///
/// <para>
/// A failure mode tagged <c>[avp: criterion]</c> in spec.md is proven only when its case saves the verdict as
/// <c>avp-FM-&lt;n&gt;.json</c>:
/// </para>
/// <code>
/// var verdict = await Runner.Run(catalog, new RequestIdempotency(), "Deposit", subject, transport: app.CreateClient);
/// SpecEvidence.Save("avp-FM-5.json", verdict);
/// </code>
///
/// <para>
/// Outside a proof run (a plain <c>dotnet test</c>, the IDE) the variable is unset and every call is a no-op, so
/// the same test runs anywhere. The helper is plain System.Text.Json and file IO with no dependency on Assay: any
/// object serializes, and a verdict from either Assay implementation is read back by the engine.
/// </para>
/// </summary>
public static class SpecEvidence
{
    /// <summary>The environment variable the <c>skies</c> binary sets to the evidence folder of the current run.</summary>
    public const string Variable = "SKIES_EVIDENCE";

    // Indented and with enums as names, so a committed verdict reads as "Pass"/"Fail" in review rather than 0/1.
    // Web defaults give camelCase properties, the shape the TypeScript Assay writes, so both implementations' files
    // look alike in a spec folder.
    private static readonly JsonSerializerOptions Options = new(JsonSerializerDefaults.Web)
    {
        WriteIndented = true,
        Converters = { new JsonStringEnumConverter() },
    };

    /// <summary>
    /// The evidence folder of the current proof run, or <see langword="null"/> when no proof is running. A test can
    /// write any file here directly (a screenshot, a HAR) when <see cref="Save"/>'s JSON is not the right shape.
    /// </summary>
    public static string? Directory =>
        Environment.GetEnvironmentVariable(Variable) is { Length: > 0 } directory ? directory : null;

    /// <summary>
    /// Writes <paramref name="value"/> as indented JSON to <paramref name="name"/> inside the evidence folder and
    /// returns the full path, or does nothing and returns <see langword="null"/> when no proof is running. Call it
    /// before asserting, so a failing run still leaves the artifact that explains the failure.
    /// </summary>
    /// <param name="name">A file name, optionally in subfolders (<c>avp-FM-5.json</c>, <c>http/deposit.json</c>).
    /// It must stay inside the evidence folder.</param>
    /// <param name="value">Any object System.Text.Json can serialize, typically an Assay <c>Verdict</c>.</param>
    /// <exception cref="ArgumentException"><paramref name="name"/> is empty, rooted, or climbs out of the folder.</exception>
    public static string? Save(string name, object value)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(name);
        ArgumentNullException.ThrowIfNull(value);
        if (Directory is not { } directory)
            return null;

        var root = Path.GetFullPath(directory);
        var path = Path.GetFullPath(Path.Combine(root, name));
        if (Path.IsPathRooted(name) || !path.StartsWith(root + Path.DirectorySeparatorChar, StringComparison.Ordinal))
            throw new ArgumentException($"'{name}' must be a relative path inside the evidence folder.", nameof(name));

        System.IO.Directory.CreateDirectory(Path.GetDirectoryName(path)!);
        File.WriteAllText(path, JsonSerializer.Serialize(value, value.GetType(), Options) + Environment.NewLine);
        return path;
    }
}
