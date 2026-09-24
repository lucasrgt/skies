using System.Security.Cryptography;
using System.Text;
using Konscious.Security.Cryptography;
using Skies.Framework.Auth;

namespace Skies.Framework.Auth.Tests;

// The hasher is the one place a password's safety is decided, so these pin its contract: a hash verifies only its
// own password, every unusable input fails closed without throwing, and the storage encoding stays the one Skies 4
// apps already wrote, so their users keep signing in after the move to the package.
public class PasswordHasherTests
{
    private readonly Argon2idPasswordHasher hasher = new();

    [Fact]
    public void A_hash_verifies_its_own_password_and_nothing_else()
    {
        var hash = hasher.Hash("correct horse");

        Assert.True(hasher.Verify("correct horse", hash));
        Assert.False(hasher.Verify("correct horsf", hash));
        Assert.False(hasher.Verify("", hash));
    }

    [Fact]
    public void An_empty_password_is_never_hashed()
    {
        Assert.Throws<ArgumentException>(() => hasher.Hash(""));
    }

    // The limit is what keeps one request from buying an unbounded slow-hash computation: the longest allowed
    // password round-trips, one character more is never derived, on either side.
    [Fact]
    public void A_password_past_the_maximum_length_is_refused_on_hash_and_fails_on_verify()
    {
        var longest = new string('p', IPasswordHasher.MaxPasswordLength);
        var hash = hasher.Hash(longest);

        Assert.True(hasher.Verify(longest, hash));
        Assert.Throws<ArgumentException>(() => hasher.Hash(longest + "p"));
        Assert.False(hasher.Verify(longest + "p", hash));
        Assert.False(hasher.Verify(new string('p', 1_000_000), null));
    }

    [Fact]
    public void Each_hash_is_salted_so_equal_passwords_store_differently()
    {
        Assert.NotEqual(hasher.Hash("same").Value, hasher.Hash("same").Value);
    }

    // The no-account branch of login: a missing user's null hash must fail (after dummy work), never throw.
    [Fact]
    public void A_missing_hash_fails_closed()
    {
        Assert.False(hasher.Verify("anything", null));
    }

    [Theory]
    [InlineData("")]
    [InlineData("not-a-hash")]
    [InlineData("a.b.c")]
    [InlineData("!!!.@@@")]
    [InlineData("AAAA.AAAA")]
    public void An_unusable_stored_value_fails_closed_without_throwing(string stored)
    {
        Assert.False(hasher.Verify("anything", PasswordHash.FromStored(stored)));
    }

    [Fact]
    public void The_none_hash_never_verifies_even_the_empty_password()
    {
        Assert.False(hasher.Verify("", PasswordHash.None));
        Assert.False(hasher.Verify("password", PasswordHash.None));
    }

    // Derived here with Konscious directly, exactly as the Skies 4 blueprint's BuildingBlocks/PasswordHash did.
    [Fact]
    public void A_hash_in_the_skies_4_encoding_still_verifies()
    {
        var salt = RandomNumberGenerator.GetBytes(16);
        var derived = new Argon2id(Encoding.UTF8.GetBytes("legacy-pw1"))
        {
            Salt = salt,
            DegreeOfParallelism = 1,
            MemorySize = 19456,
            Iterations = 2,
        }.GetBytes(32);
        var stored = PasswordHash.FromStored($"{Convert.ToBase64String(salt)}.{Convert.ToBase64String(derived)}");

        Assert.True(hasher.Verify("legacy-pw1", stored));
        Assert.False(hasher.Verify("legacy-pw2", stored));
    }

    [Fact]
    public void A_hash_never_prints_its_value()
    {
        var hash = hasher.Hash("secret");

        Assert.DoesNotContain(hash.Value, hash.ToString(), StringComparison.Ordinal);
        Assert.DoesNotContain(hash.Value, $"{hash}", StringComparison.Ordinal);
    }
}
