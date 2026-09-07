using System.Runtime.InteropServices;
using Skies.Framework.Cli;

namespace Skies.Framework.Cli.Tests;

public class FoundationToolTests
{
    [Fact]
    public void The_framework_pins_one_csm_release_instead_of_four_independent_tools()
    {
        Assert.Equal("0.1.1", CsmCommand.Version);
        Assert.Equal("csm", CsmCommand.Tool.Id);
        Assert.Equal("Codebase Semantic Memory", CsmCommand.Tool.DisplayName);
    }

    [Theory]
    [InlineData("windows", Architecture.X64, "x86_64-pc-windows-msvc")]
    [InlineData("linux", Architecture.X64, "x86_64-unknown-linux-gnu")]
    [InlineData("linux", Architecture.Arm64, "aarch64-unknown-linux-gnu")]
    [InlineData("macos", Architecture.X64, "x86_64-apple-darwin")]
    [InlineData("macos", Architecture.Arm64, "aarch64-apple-darwin")]
    public void Csm_resolves_every_published_platform(
        string platform,
        Architecture architecture,
        string expected)
    {
        Assert.Equal(expected, CsmCommand.Target(platform, architecture));
    }

    [Fact]
    public void Csm_fails_closed_on_unpublished_platforms_and_unknown_assets()
    {
        Assert.Throws<PlatformNotSupportedException>(
            () => CsmCommand.Target("windows", Architecture.Arm64));
        Assert.Throws<PlatformNotSupportedException>(
            () => CsmCommand.Target("freebsd", Architecture.X64));

        var archive = "not a published archive"u8.ToArray();
        Assert.False(CsmCommand.ChecksumMatches(archive, "x86_64-pc-windows-msvc"));
        Assert.False(CsmCommand.ChecksumMatches(archive, "unknown-target"));
    }
}
