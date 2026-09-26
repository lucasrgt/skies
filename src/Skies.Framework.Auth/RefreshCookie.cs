using System.Net;
using Microsoft.AspNetCore.Http;

namespace Skies.Framework.Auth;

/// <summary>The app-specific knobs of the refresh cookie. <paramref name="Name"/> and the
/// <paramref name="Path"/> it is scoped to (typically the auth routes, so it is sent nowhere else) are the
/// everyday two. The <em>non-negotiable</em> security attributes are still not here — httpOnly and Secure
/// are the framework's opinion, never a per-app choice (with the sole exception of plain-HTTP loopback,
/// where browsers cannot return a Secure cookie during local development). <paramref name="SameSite"/> and
/// <paramref name="Domain"/> <em>are</em> tunable, because a real multi-subdomain case needs them: a
/// "remember this device" cookie that must survive a cross-persona hop between two store apps on sibling
/// subdomains cannot live under the host-only <see cref="SameSiteMode.Strict"/> default. Both keep that
/// strict, host-only default — widen them only with intent. <paramref name="Domain"/> <see langword="null"/>
/// means host-only (the browser scopes the cookie to the exact origin); set it (e.g. <c>.example.com</c>) to
/// share the cookie across subdomains.</summary>
public sealed record RefreshCookieOptions(
    string Name,
    string Path = "/",
    string? Domain = null,
    SameSiteMode SameSite = SameSiteMode.Strict)
{
    /// <summary>How long a planted cookie lives, used by <see cref="RefreshCookie.Deliver{TBody, TWebBody}"/>.
    /// <see cref="SkiesAuthExtensions.AddSkiesAuth{TSessionStore}"/> sets it from
    /// <see cref="RefreshSessionOptions.Lifetime"/>, so the cookie and the session row expire together.</summary>
    public TimeSpan Lifetime { get; init; } = RefreshSessionOptions.Default.Lifetime;
}

/// <summary>How the refresh token reaches each client. Web stores it in an httpOnly, Secure cookie
/// (<see cref="SameSiteMode.Strict"/> and host-only by default) — invisible to JS, so an XSS payload cannot
/// exfiltrate it; the access token still rides in the body for the Authorization header. Mobile / API
/// clients get the refresh in the body and keep it in secure storage. A request opts into web delivery with
/// the <c>X-Client: web</c> header — that one signal is why the same endpoints serve both web and mobile.
/// The cookie's <see cref="RefreshCookieOptions.SameSite"/> and <see cref="RefreshCookieOptions.Domain"/> are
/// app-tunable for the multi-subdomain case; httpOnly and Secure are not. Secure is omitted only for an
/// HTTP request whose host is loopback, so the same cookie flow remains testable under a local Vite proxy;
/// every non-loopback host stays Secure even when TLS terminates upstream.</summary>
public sealed class RefreshCookie(RefreshCookieOptions options, TimeProvider? clock = null)
{
    // The header a web client sends to opt into cookie delivery. A framework convention, not app config.
    private const string ClientHeader = "X-Client";
    private const string WebClient = "web";

    /// <summary>True when the caller is the web client, and so wants cookie-based refresh.</summary>
    public bool IsWeb(HttpRequest req) =>
        string.Equals(req.Headers[ClientHeader], WebClient, StringComparison.OrdinalIgnoreCase);

    /// <summary>The refresh token to act on: the httpOnly cookie for web, the request body otherwise.</summary>
    public string RefreshFrom(HttpRequest req, string? fromBody) =>
        req.Cookies.TryGetValue(options.Name, out var fromCookie) && fromCookie.Length > 0 ? fromCookie : fromBody ?? "";

    /// <summary>Plant the web refresh cookie, scoped to the auth routes and the token's lifetime. httpOnly and
    /// Secure are forced on outside plain-HTTP loopback development; <see cref="RefreshCookieOptions.SameSite"/> and
    /// <see cref="RefreshCookieOptions.Domain"/> ride from the options (strict, host-only unless widened).</summary>
    public void SetRefresh(HttpResponse res, string token, DateTimeOffset expires) =>
        res.Cookies.Append(options.Name, token, new CookieOptions
        {
            HttpOnly = true,
            Secure = ShouldSecure(res.HttpContext.Request),
            SameSite = options.SameSite,
            Domain = options.Domain,
            Expires = expires,
            Path = options.Path,
        });

    /// <summary>Answer a successful sign-in or refresh. A web caller gets <paramref name="refreshToken"/> planted as the
    /// cookie (living <see cref="RefreshCookieOptions.Lifetime"/>) and <paramref name="webBody"/>, which must not carry
    /// the refresh token; any other caller gets <paramref name="body"/>, refresh token included, for its secure
    /// storage. The slice keeps the failure path (<c>ToHttp</c>) and chooses both bodies; this owns which one leaves
    /// and how the cookie is set.</summary>
    public IResult Deliver<TBody, TWebBody>(HttpContext http, string refreshToken, TBody body, TWebBody webBody)
    {
        if (!IsWeb(http.Request))
            return Results.Ok(body);
        SetRefresh(http.Response, refreshToken, (clock ?? TimeProvider.System).GetUtcNow().Add(options.Lifetime));
        return Results.Ok(webBody);
    }

    private static bool ShouldSecure(HttpRequest request)
    {
        if (request.IsHttps)
            return true;

        var host = request.Host.Host;
        if (string.Equals(host, "localhost", StringComparison.OrdinalIgnoreCase))
            return false;

        return !IPAddress.TryParse(host, out var address) || !IPAddress.IsLoopback(address);
    }

    /// <summary>Drop the web refresh cookie (logout). Harmless when no cookie was set. The Path and Domain must
    /// match the planted cookie's, or the browser keeps it — so both ride from the same options.</summary>
    public void Clear(HttpResponse res) =>
        res.Cookies.Delete(options.Name, new CookieOptions { Path = options.Path, Domain = options.Domain });
}
