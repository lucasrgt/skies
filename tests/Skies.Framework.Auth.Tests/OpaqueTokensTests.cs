using Skies.Framework.Auth;

namespace Skies.Framework.Auth.Tests;

// Opaque tokens are bearer secrets: unguessable, safe on the wire, stored only as a deterministic hash, and matched
// in constant time so a near-miss never counts.
public class OpaqueTokensTests
{
    [Fact]
    public void Issued_tokens_are_unique_url_safe_and_carry_their_own_hash()
    {
        var tokens = Enumerable.Range(0, 200).Select(_ => OpaqueTokens.Issue()).ToList();

        Assert.Equal(tokens.Count, tokens.Select(t => t.Raw).Distinct().Count());
        foreach (var (raw, hash) in tokens)
        {
            Assert.Equal(43, raw.Length);   // 32 bytes, unpadded Base64Url
            Assert.All(raw, c => Assert.True(char.IsAsciiLetterOrDigit(c) || c is '-' or '_', $"'{c}' is not URL-safe"));
            Assert.Equal(OpaqueTokens.Hash(raw), hash);
            Assert.NotEqual(raw, hash);
        }
    }

    [Fact]
    public void The_hash_is_deterministic_hex_so_a_presented_token_can_be_looked_up()
    {
        var hash = OpaqueTokens.Hash("the-token");

        Assert.Equal(hash, OpaqueTokens.Hash("the-token"));
        Assert.Equal(64, hash.Length);
        Assert.True(hash.All(Uri.IsHexDigit));
        Assert.NotEqual(hash, OpaqueTokens.Hash("the-tokeN"));
    }

    [Fact]
    public void Matches_accepts_only_the_exact_token()
    {
        var (raw, hash) = OpaqueTokens.Issue();

        Assert.True(OpaqueTokens.Matches(raw, hash));
        Assert.False(OpaqueTokens.Matches(raw + "x", hash));
        Assert.False(OpaqueTokens.Matches(raw, hash.ToLowerInvariant()));   // a case-folding store is no match
        Assert.False(OpaqueTokens.Matches(raw, hash[..^1]));                // a different length compares false, never throws
    }

    [Theory]
    [InlineData("", "ABC")]
    [InlineData("raw", "")]
    [InlineData("", "")]
    public void Matches_refuses_empty_inputs(string raw, string hash)
    {
        Assert.False(OpaqueTokens.Matches(raw, hash));
    }
}
