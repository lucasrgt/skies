using System.Security.Claims;
using System.Text;
using Microsoft.IdentityModel.JsonWebTokens;
using Microsoft.IdentityModel.Tokens;

namespace Skies.Framework.Auth;

/// <summary>Issues HMAC-SHA256 access tokens, 15 minutes by default. The <paramref name="secret"/>, <paramref
/// name="issuer"/>, and <paramref name="audience"/> are the same values the JwtBearer validator is
/// configured with (see <see cref="JwtAccessTokenExtensions.AddJwtAccessTokens"/>), so the tokens this
/// mints are exactly what the middleware accepts. Issuer and audience are parameters, not constants — the
/// framework names no app.</summary>
/// <remarks>An access token is stateless: signing out revokes the refresh family, but a token already issued stays
/// valid until it expires. <paramref name="lifetime"/> is that window; shorten it to shrink the exposure, at the cost
/// of more refreshes.</remarks>
public sealed class AccessTokens(string secret, string issuer, string audience, TimeProvider clock, TimeSpan? lifetime = null)
    : IAccessTokens
{
    /// <summary>How long an access token lives when no lifetime is given.</summary>
    public static readonly TimeSpan DefaultLifetime = TimeSpan.FromMinutes(15);

    private readonly TimeSpan lifetime = lifetime is { } given && given > TimeSpan.Zero
        ? given
        : lifetime is null ? DefaultLifetime : throw new ArgumentOutOfRangeException(nameof(lifetime), "must be positive");

    /// <inheritdoc />
    public string Issue(Guid userId, Guid orgId, string? role, Guid sessionId, string? name)
    {
        var key = new SymmetricSecurityKey(Encoding.UTF8.GetBytes(secret));
        var claims = new List<Claim>
        {
            new("sub", userId.ToString()),
            new("org", orgId.ToString()),
            new("sid", sessionId.ToString()),
        };
        // Absent, never empty: an empty "role"/"name" claim reads back as "" not null, so a caller's
        // `Role is null` (no role) check would silently never fire. Omit the claim when there is nothing.
        if (!string.IsNullOrEmpty(role))
            claims.Add(new Claim("role", role));
        if (!string.IsNullOrEmpty(name))
            claims.Add(new Claim("name", name));

        var descriptor = new SecurityTokenDescriptor
        {
            Issuer = issuer,
            Audience = audience,
            Expires = clock.GetUtcNow().UtcDateTime.Add(this.lifetime),
            SigningCredentials = new SigningCredentials(key, SecurityAlgorithms.HmacSha256),
            Subject = new ClaimsIdentity(claims),
        };
        return new JsonWebTokenHandler().CreateToken(descriptor);
    }
}
