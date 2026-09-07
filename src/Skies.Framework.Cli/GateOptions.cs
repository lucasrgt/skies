namespace Skies.Framework.Cli;

/// <summary>The source revision set that drives one gate execution.</summary>
internal enum GateMode
{
    /// <summary>Use the Git-affected delta; an explicit base freezes the scope to <c>base...HEAD</c>.</summary>
    Affected,

    /// <summary>Use only paths present in the Git index.</summary>
    Staged,

    /// <summary>Execute every proof regardless of the current change.</summary>
    Full,
}

/// <summary>The gate-owned options, separated from arguments forwarded to the .NET solution.</summary>
/// <param name="Mode">How changed files are discovered.</param>
/// <param name="Fast">Whether exhaustive fallbacks and browser/device execution are deferred to the selected authority boundary.</param>
/// <param name="BaseRevision">The revision compared with <c>HEAD</c> in affected mode; when explicit, local-only files are excluded.</param>
/// <param name="ToolArguments">Arguments forwarded to <c>dotnet build/test</c>.</param>
internal sealed record GateOptions(
    GateMode Mode,
    bool Fast,
    string? BaseRevision,
    IReadOnlyList<string> ToolArguments)
{
    internal string? RetryReview { get; init; }
    /// <summary>Parse gate options without allowing a caller-authored test filter to shrink the proof set.</summary>
    public static bool TryParse(string[] arguments, out GateOptions options, out string? error)
    {
        var mode = GateMode.Affected;
        var modeSeen = false;
        var fast = false;
        string? baseRevision = null;
        var forwarded = new List<string>();
        string? retryReview = null;

        for (var index = 0; index < arguments.Length; index++)
        {
            var argument = arguments[index];
            if (argument == "--retry-review")
            {
                if (++index >= arguments.Length || arguments[index].StartsWith("--", StringComparison.Ordinal))
                {
                    options = Default;
                    error = "--retry-review requires a JSON file";
                    return false;
                }
                retryReview = arguments[index];
                continue;
            }
            if (argument is "--affected" or "--staged" or "--full")
            {
                if (modeSeen)
                {
                    options = Default;
                    error = "gate modes are mutually exclusive; choose one of --affected, --staged, or --full";
                    return false;
                }

                modeSeen = true;
                mode = argument switch
                {
                    "--staged" => GateMode.Staged,
                    "--full" => GateMode.Full,
                    _ => GateMode.Affected,
                };
                continue;
            }

            if (argument == "--fast")
            {
                fast = true;
                continue;
            }

            if (argument == "--base")
            {
                if (++index >= arguments.Length || arguments[index].StartsWith("--", StringComparison.Ordinal))
                {
                    options = Default;
                    error = "--base requires a Git revision";
                    return false;
                }

                baseRevision = arguments[index];
                continue;
            }

            if (argument is "--filter" or "--test-adapter-path" || argument.StartsWith("--filter=", StringComparison.Ordinal))
            {
                options = Default;
                error = $"{argument.Split('=')[0]} is gate-owned; use --affected to select proofs from the Git change";
                return false;
            }

            forwarded.Add(argument);
        }

        if (mode != GateMode.Affected && baseRevision is not null)
        {
            options = Default;
            error = "--base is only valid with --affected";
            return false;
        }

        if (mode == GateMode.Full && fast)
        {
            options = Default;
            error = "--full and --fast conflict; a full audit executes every proof";
            return false;
        }

        options = new GateOptions(mode, fast, baseRevision, forwarded) { RetryReview = retryReview };
        error = null;
        return true;
    }

    private static GateOptions Default { get; } = new(GateMode.Affected, false, null, []);
}
