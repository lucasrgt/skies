using Skies.Framework.Cli;

namespace Skies.Framework.Cli.Tests;

public sealed class FlutterIntegrationSuiteTests : IDisposable
{
    private readonly string root = Directory.CreateTempSubdirectory("skies-flutter-suite-").FullName;

    [Fact]
    public void Deduplicated_proofs_share_one_compilation_with_separate_test_groups()
    {
        Write("a");
        Write("b");
        string? generated = null;
        var calls = 0;
        var code = FlutterIntegrationSuite.Run(root,
            ["integration_test/b_test.dart", "integration_test/a_test.dart", "integration_test/a_test.dart"],
            (arguments, directory) =>
            {
                calls++;
                generated = Path.Combine(directory, arguments[1]);
                var source = File.ReadAllText(generated);
                Assert.Contains("group('integration_test/a_test.dart', proof0.main)", source);
                Assert.Contains("group('integration_test/b_test.dart', proof1.main)", source);
                Assert.True(source.IndexOf("IntegrationTestWidgetsFlutterBinding.ensureInitialized();", StringComparison.Ordinal)
                    < source.IndexOf("group(", StringComparison.Ordinal));
                Assert.Equal(2, arguments.Length);
                return 1;
            });
        Assert.Equal(1, code);
        Assert.Equal(1, calls);
        Assert.False(File.Exists(generated));
        Assert.True(File.Exists(Path.Combine(root, "integration_test/a_test.dart")));
    }

    [Fact]
    public void Async_entry_points_keep_the_original_flutter_lifecycle()
    {
        Write("a", "Future<void> main() async {}\n");
        Write("b");
        FlutterIntegrationSuite.Run(root, ["integration_test/a_test.dart", "integration_test/b_test.dart"],
            (arguments, _) =>
            {
                Assert.Equal(["test", "integration_test/a_test.dart", "integration_test/b_test.dart"], arguments);
                return 0;
            });
    }

    [Fact]
    public void Cleanup_also_runs_when_the_runner_throws()
    {
        Write("a");
        Write("b");
        Assert.Throws<IOException>(() => FlutterIntegrationSuite.Run(root,
            ["integration_test/a_test.dart", "integration_test/b_test.dart"], (_, _) => throw new IOException()));
        Assert.Equal(2, Directory.GetFiles(Path.Combine(root, "integration_test")).Length);
    }

    [Fact]
    public void Missing_or_external_proofs_fail_before_any_execution()
    {
        Assert.Throws<InvalidDataException>(() => FlutterIntegrationSuite.Run(root, ["../outside_test.dart"],
            (_, _) => throw new Exception("must not run")));
    }

    private void Write(string name, string content = "void main() {}\n")
    {
        Directory.CreateDirectory(Path.Combine(root, "integration_test"));
        File.WriteAllText(Path.Combine(root, "integration_test", name + "_test.dart"), content);
    }

    public void Dispose() => Directory.Delete(root, recursive: true);
}
