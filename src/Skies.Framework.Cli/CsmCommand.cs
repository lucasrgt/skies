using System.Runtime.InteropServices;

namespace Skies.Framework.Cli;

/// <summary>Runs the Skies-pinned Codebase Semantic Memory bundle.</summary>
internal static class CsmCommand
{
    internal const string Version = "0.1.1";

    internal static readonly FoundationTool Tool = new(
        "csm",
        "Codebase Semantic Memory",
        Version,
        "lucasrgt/codebase-semantic-memory",
        new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["aarch64-apple-darwin"] = "8547a12c05e9e9d84d567e14dda913fe7b3868d36a8105a82300103d5063f982",
            ["aarch64-unknown-linux-gnu"] = "16c4a5498f43268f04a26c7294b10adeab2b0cb316cc991a1a813eba28e86fd9",
            ["x86_64-apple-darwin"] = "014ca0773d401f6ca4bdb49d7dd5a25bc6c574b4ff3a0ccb947ad2fb8b3b5b47",
            ["x86_64-pc-windows-msvc"] = "80fe0665d96f36a21807de46544146b37a0c4d1a9c37308485aba5a1d0ba1e15",
            ["x86_64-unknown-linux-gnu"] = "5d96cffa47afaba70b112464b4305a3589e74503128480b5bb4827a8c00f4dc9",
        });

    private static bool ready;

    /// <summary>Run one CSM-managed tool through the Skies command surface.</summary>
    internal static int RunTool(string id, string[] arguments)
    {
        var readiness = EnsureReady();
        return readiness == 0
            ? Tool.Run([id, .. arguments])
            : readiness;
    }

    /// <summary>Capture one CSM-managed tool for the composed context workflow.</summary>
    internal static FoundationTool.Execution CaptureTool(string id, string[] arguments)
    {
        var readiness = EnsureReady();
        return readiness == 0
            ? Tool.Capture([id, .. arguments])
            : new FoundationTool.Execution(readiness, "", "skies foundations: CSM synchronization failed.\n");
    }

    /// <summary>Initialize a new managed store or adopt a legacy standalone layout.</summary>
    internal static int Initialize(bool adopt)
    {
        var code = Tool.Run([adopt ? "adopt" : "init"]);
        return code == 0 ? MarkReady() : code;
    }

    /// <summary>Synchronize the exact tool versions pinned by CSM.</summary>
    internal static int Synchronize()
    {
        var code = Tool.Run(["sync"]);
        return code == 0 ? MarkReady() : code;
    }

    internal static string Target(string platform, Architecture architecture) =>
        FoundationTool.Target("Codebase Semantic Memory", Version, platform, architecture);

    internal static bool ChecksumMatches(byte[] archive, string target) =>
        Tool.ChecksumMatches(archive, target);

    private static int EnsureReady()
    {
        if (ready)
            return 0;

        var doctor = Tool.Capture(["doctor", "--json"]);
        if (doctor.ExitCode == 0)
            return MarkReady();

        if (doctor.ExitCode != 1)
        {
            if (!string.IsNullOrWhiteSpace(doctor.Error))
                Console.Error.Write(doctor.Error);
            Console.Error.WriteLine(
                "skies foundations: initialize this repository with `dotnet tool run skies foundations init`.");
            return Math.Max(2, doctor.ExitCode);
        }

        Console.WriteLine("skies foundations: synchronizing the CSM-pinned tools...");
        return Synchronize();
    }

    private static int MarkReady()
    {
        try
        {
            CsmProject.AdaptInstructions(Directory.GetCurrentDirectory());
            ready = true;
            return 0;
        }
        catch (Exception exception) when (exception is IOException or UnauthorizedAccessException)
        {
            Console.Error.WriteLine($"skies foundations: could not adapt CSM skills: {exception.Message}");
            return 2;
        }
    }
}
