using System.Runtime.CompilerServices;

namespace Skies.Framework.Cli.Tests;

/// <summary>Prevents hooks from redirecting temporary fixture commits into the caller's checkout.</summary>
internal static class GitTestEnvironment
{
    [ModuleInitializer]
    internal static void IsolateFixtures()
    {
        foreach (var variable in new[]
        {
            "GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE", "GIT_PREFIX", "GIT_COMMON_DIR",
            "GIT_OBJECT_DIRECTORY", "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        })
            Environment.SetEnvironmentVariable(variable, null);
    }
}
