using System.Text.Json;
using Skies.Framework.Cli;

namespace Skies.Framework.Cli.Tests;

public sealed class FrontendConsumersTests : IDisposable
{
    private readonly string root = Path.Combine(Path.GetTempPath(), "skies-consumers-" + Guid.NewGuid().ToString("N"));

    [Fact]
    public void A_flutter_library_reaches_transitive_consumers_but_not_unrelated_platforms()
    {
        var library = Flutter("ui", "name: ui\ndependencies:\n  flutter:\n    sdk: flutter\n");
        var core = Flutter("core", "name: core\ndependencies:\n  ui:\n    path: ../ui\n");
        var app = Flutter("app", "name: app\ndependencies:\n  core: any\n");
        var unrelated = Flutter("other", "name: other\ndependencies: {}\n");
        var web = Npm("web", new { name = "web", dependencies = new { react = "19" } });
        var reached = FrontendConsumers.Expand([library, core, app, unrelated, web], [library], []);
        Assert.Equal(3, reached.Count);
        Assert.Contains(app, reached);
        Assert.DoesNotContain(unrelated, reached);
        Assert.DoesNotContain(web, reached);
    }

    [Fact]
    public void Gate_planning_does_not_promote_unrelated_packages_after_a_library_change()
    {
        var library = Flutter("ui", "name: ui\n");
        var app = Flutter("app", "name: app\ndependencies:\n  ui: any\n");
        var web = Npm("web", new { name = "web" });
        Directory.CreateDirectory(Path.Combine(library.Package.Path, "lib"));
        File.WriteAllText(Path.Combine(library.Package.Path, "lib/button.dart"), "class Button {}\n");
        var plan = GateImpact.Build(root, ["ui/lib/button.dart"], [], [], [], [],
            [library.Package, app.Package, web.Package]);
        var authoritative = GateCommand.ApplyFastFeedback(plan, false);
        Assert.True(authoritative.Frontends[0].Full);
        Assert.True(authoritative.Frontends[1].Full);
        Assert.False(authoritative.Frontends[2].Selected);
    }

    [Fact]
    public void Npm_peer_and_development_dependencies_preserve_consumers_and_cycles_terminate()
    {
        var library = Npm("ui", new { name = "ui", dependencies = new { app = "*" } });
        var app = Npm("app", new { name = "app", peerDependencies = new { ui = "*" } });
        var tests = Npm("tests", new { name = "tests", devDependencies = new { app = "*" } });
        var other = Npm("other", new { name = "other" });
        var reached = FrontendConsumers.Expand([library, app, tests, other], [library], []);
        Assert.Equal(3, reached.Count);
        Assert.Contains(tests, reached);
    }

    [Theory]
    [InlineData("name: app\ndependencies: {ui: any}\n")]
    [InlineData("name: app\ndependencies:\n  <<: *shared\n")]
    [InlineData("name: app\n'dependencies':\n  ui: any\n")]
    [InlineData("name: app\n<<: *shared\n")]
    [InlineData("name: app\ndependencies:\n\tui: any\n")]
    public void Unsupported_dependency_syntax_cannot_silently_drop_a_consumer(string yaml)
    {
        var library = Flutter("ui", "name: ui\n");
        var app = Flutter("app", yaml);
        Assert.Contains(app, FrontendConsumers.Expand([library, app], [library], []));
    }

    [Theory]
    [InlineData("npm:@scope/ui@^1.0.0")]
    [InlineData("file:../ui")]
    public void Aliased_npm_dependencies_cannot_hide_a_consumer(string version)
    {
        var library = Npm("ui", new { name = "@scope/ui" });
        var app = Npm("app", new { name = "app", dependencies = new { alias = version } });
        Assert.Contains(app, FrontendConsumers.Expand([library, app], [library], []));
    }

    [Fact]
    public void Flutter_override_files_retain_unknown_consumers()
    {
        var library = Flutter("ui", "name: ui\n");
        var app = Flutter("app", "name: app\n");
        File.WriteAllText(Path.Combine(app.Package.Path, "pubspec_overrides.yaml"), "dependency_overrides:\n  ui: any\n");
        Assert.Contains(app, FrontendConsumers.Expand([library, app], [library], []));
    }

    [Fact]
    public void Missing_library_identity_retains_the_existing_conservative_fallback()
    {
        var library = Npm("ui", new { });
        var app = Npm("app", new { name = "app" });
        Assert.Equal(2, FrontendConsumers.Expand([library, app], [library], []).Count);
    }

    private FrontendImpact Flutter(string directory, string manifest)
    {
        var path = Path.Combine(root, directory);
        Directory.CreateDirectory(path);
        File.WriteAllText(Path.Combine(path, "pubspec.yaml"), manifest);
        return new FrontendImpact(new FrontendPackage(path, FrontendPackageRole.Library, FrontendPlatform.Flutter));
    }

    private FrontendImpact Npm(string directory, object manifest)
    {
        var path = Path.Combine(root, directory);
        Directory.CreateDirectory(path);
        File.WriteAllText(Path.Combine(path, "package.json"), JsonSerializer.Serialize(manifest));
        return new FrontendImpact(new FrontendPackage(path, FrontendPackageRole.Library));
    }

    public void Dispose() => Directory.Delete(root, recursive: true);
}
