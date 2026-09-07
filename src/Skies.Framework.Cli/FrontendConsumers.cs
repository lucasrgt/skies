using System.Text.Json;
using System.Text.RegularExpressions;

namespace Skies.Framework.Cli;

/// <summary>Bounds shared-package impact by declared package dependencies, preserving unknown consumers.</summary>
internal static class FrontendConsumers
{
    private sealed record Package(string Name, HashSet<string> Dependencies);

    internal static IReadOnlySet<FrontendImpact> Expand(
        IReadOnlyList<FrontendImpact> packages,
        IReadOnlyList<FrontendImpact> changed,
        List<string> reasons)
    {
        var manifests = packages.ToDictionary(package => package, package => Read(package.Package));
        var reached = changed.ToHashSet();
        var pending = new Queue<FrontendImpact>(changed);
        while (pending.TryDequeue(out var dependency))
        {
            var manifest = manifests[dependency];
            if (manifest is null)
            {
                reasons.Add($"frontend: {dependency.Package.Path} has no readable package identity; "
                    + "its consumers are unknown, retaining every surface");
                reached.UnionWith(packages);
                break;
            }
            foreach (var consumer in packages.Where(package => !reached.Contains(package)))
            {
                var candidate = manifests[consumer];
                if (candidate is not null && !candidate.Dependencies.Contains(manifest.Name))
                    continue;
                reached.Add(consumer);
                pending.Enqueue(consumer);
                reasons.Add(candidate is null
                    ? $"frontend: {consumer.Package.Path} has unreadable dependencies; retaining it as a possible consumer"
                    : $"frontend: {candidate.Name} depends on {manifest.Name}; selecting its runtime proofs");
            }
        }
        return reached;
    }

    private static Package? Read(FrontendPackage package)
    {
        try
        {
            return package.Platform == FrontendPlatform.Flutter
                ? ReadFlutter(Path.Combine(package.Path, "pubspec.yaml"))
                : ReadNpm(Path.Combine(package.Path, "package.json"));
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException or JsonException)
        {
            return null;
        }
    }

    private static Package? ReadNpm(string path)
    {
        using var document = JsonDocument.Parse(File.ReadAllText(path));
        var root = document.RootElement;
        if (root.ValueKind != JsonValueKind.Object || !root.TryGetProperty("name", out var name)
            || name.ValueKind != JsonValueKind.String || string.IsNullOrWhiteSpace(name.GetString()))
            return null;
        var dependencies = new HashSet<string>(StringComparer.Ordinal);
        foreach (var section in new[] { "dependencies", "devDependencies", "peerDependencies", "optionalDependencies" })
        {
            if (!root.TryGetProperty(section, out var values))
                continue;
            if (values.ValueKind != JsonValueKind.Object)
                return null;
            foreach (var value in values.EnumerateObject())
            {
                if (value.Value.ValueKind != JsonValueKind.String)
                    return null;
                dependencies.Add(value.Name);
                var version = value.Value.GetString()!;
                if (version.StartsWith("npm:", StringComparison.Ordinal))
                {
                    var alias = Regex.Match(version, @"^npm:((?:@[^/]+/)?[^@]+)(?:@.*)?$");
                    if (!alias.Success)
                        return null;
                    dependencies.Add(alias.Groups[1].Value);
                }
                else if (version.Contains(':', StringComparison.Ordinal)
                    && !Regex.IsMatch(version, @"^workspace:[*~^0-9]"))
                    return null; // Path, Git, and nonstandard aliases can hide a different package identity.
            }
        }
        return new Package(name.GetString()!, dependencies);
    }

    private static Package? ReadFlutter(string path)
    {
        if (File.Exists(Path.Combine(Path.GetDirectoryName(path)!, "pubspec_overrides.yaml")))
            return null;
        string? name = null;
        var dependencies = new HashSet<string>(StringComparer.Ordinal);
        var keys = new HashSet<string>(StringComparer.Ordinal);
        var section = false;
        var dependencyIndent = 0;
        foreach (var raw in File.ReadLines(path))
        {
            if (raw.Contains('\t', StringComparison.Ordinal))
                return null;
            var line = raw.Split('#', 2)[0].TrimEnd();
            if (string.IsNullOrWhiteSpace(line))
                continue;
            var indent = line.Length - line.TrimStart(' ').Length;
            if (indent == 0)
            {
                var key = Regex.Match(line, @"^([a-zA-Z_][a-zA-Z0-9_]*):");
                if (!key.Success || !keys.Add(key.Groups[1].Value))
                    return null;
                var identity = Regex.Match(line, "^name:\\s*['\"]?([a-zA-Z_][a-zA-Z0-9_]*)['\"]?\\s*$");
                if (identity.Success)
                    name = identity.Groups[1].Value;
                var header = Regex.Match(line, "^(dependencies|dev_dependencies|dependency_overrides):\\s*(.*)$");
                section = header.Success;
                dependencyIndent = 0;
                if (section && header.Groups[2].Value is not ("" or "{}"))
                    return null;
                continue;
            }
            if (!section)
                continue;
            if (dependencyIndent == 0)
                dependencyIndent = indent;
            if (indent > dependencyIndent)
                continue;
            var dependency = Regex.Match(line.TrimStart(), "^['\"]?([a-zA-Z_][a-zA-Z0-9_]*)['\"]?:");
            if (indent != dependencyIndent || !dependency.Success)
                return null;
            dependencies.Add(dependency.Groups[1].Value);
        }
        return name is null ? null : new Package(name, dependencies);
    }
}
