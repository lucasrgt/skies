using System.Text;
using System.Text.RegularExpressions;

namespace Skies.Framework.Cli;

/// <summary>Runs selected Flutter integration files through one compiled, temporary test entry point.</summary>
internal static class FlutterIntegrationSuite
{
    internal static int Run(string root, IEnumerable<string> selected,
        Func<string[], string, int>? runner = null)
    {
        runner ??= (arguments, directory) => Tooling.Run("flutter", arguments, directory);
        var specs = selected.Distinct(StringComparer.Ordinal).Order(StringComparer.Ordinal).ToArray();
        if (specs.Length == 0)
            return 0;
        var integration = Path.GetFullPath(Path.Combine(root, "integration_test"));
        foreach (var spec in specs)
        {
            var source = Path.GetFullPath(Path.Combine(root, spec));
            var relative = Path.GetRelativePath(integration, source);
            if (Path.IsPathRooted(relative) || relative == ".."
                || relative.StartsWith(".." + Path.DirectorySeparatorChar, StringComparison.Ordinal)
                || !File.Exists(source))
                throw new InvalidDataException($"Selected integration proof is missing or outside integration_test: {spec}");
        }
        // Async registration and custom entry points retain Flutter's per-file lifecycle.
        if (specs.Length == 1 || specs.Any(spec => !Regex.IsMatch(
                File.ReadAllText(Path.Combine(root, spec)), @"(?m)^\s*void\s+main\s*\(\s*\)\s*\{")))
            return runner(["test", .. specs], root);

        var path = Path.Combine(integration, ".skies_suite_" + Guid.NewGuid().ToString("N") + "_test.dart");
        try
        {
            File.WriteAllText(path, Source(integration, root, specs));
            Console.WriteLine($"skies gate: Flutter integration — {specs.Length} selected files, one compilation");
            return runner(["test", Path.GetRelativePath(root, path)], root);
        }
        finally
        {
            File.Delete(path);
        }
    }

    private static string Source(string integration, string root, IReadOnlyList<string> specs)
    {
        var source = new StringBuilder("import 'package:flutter_test/flutter_test.dart';\n"
            + "import 'package:integration_test/integration_test.dart';\n");
        for (var index = 0; index < specs.Count; index++)
        {
            var relative = Path.GetRelativePath(integration, Path.Combine(root, specs[index])).Replace('\\', '/');
            source.AppendLine($"import {Literal(relative)} as proof{index};");
        }
        source.AppendLine("void main() {");
        // The binding registers tearDownAll; create it outside the file groups so
        // the device receives completion only after every selected file finishes.
        source.AppendLine("  IntegrationTestWidgetsFlutterBinding.ensureInitialized();");
        for (var index = 0; index < specs.Count; index++)
            source.AppendLine($"  group({Literal(specs[index])}, proof{index}.main);");
        source.AppendLine("}");
        return source.ToString();
    }

    private static string Literal(string value) => "'" + value.Replace("\\", "\\\\")
        .Replace("'", "\\'").Replace("$", "\\$").Replace("\r", "\\r").Replace("\n", "\\n") + "'";
}
