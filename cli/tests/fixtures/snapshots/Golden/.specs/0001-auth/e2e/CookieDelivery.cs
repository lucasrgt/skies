using System.Net.Http.Json;
using Microsoft.AspNetCore.Mvc.Testing;
using Golden.Tests;

namespace Specs.S0001;

/// <summary>Web delivery: with `X-Client: web` the refresh token rides in an httpOnly cookie and never in the JSON
/// body. The client does not handle cookies, so the raw Set-Cookie is visible and the cookie is replayed by hand.</summary>
public class CookieDelivery
{
    // The name AccountSetup configures via AddRefreshCookie: the wire contract a browser client depends on.
    private const string Cookie = "golden_refresh";
    private const string Email = "web@example.com";

    [Fact(DisplayName = "FM-21: a web login sets an httpOnly refresh cookie and keeps the token out of the body")]
    public async Task Web_login_sets_an_httponly_cookie_and_omits_the_token_from_the_body()
    {
        await using var app = new TestApp();
        var client = app.CreateClient(new WebApplicationFactoryClientOptions { HandleCookies = false });
        await AuthApi.Register(client, Email);

        var response = await client.SendAsync(WebLogin());

        response.EnsureSuccessStatusCode();
        var setCookie = Assert.Single(response.Headers.GetValues("Set-Cookie"));
        Assert.Contains(Cookie, setCookie, StringComparison.Ordinal);
        Assert.Contains("httponly", setCookie, StringComparison.OrdinalIgnoreCase);
        var body = await response.Content.ReadAsStringAsync();
        Assert.Contains("accessToken", body, StringComparison.OrdinalIgnoreCase);
        Assert.DoesNotContain("refreshToken", body, StringComparison.OrdinalIgnoreCase);
    }

    [Fact(DisplayName = "FM-22: a web refresh reads the cookie with no body and reissues it")]
    public async Task Web_refresh_reads_the_cookie_and_reissues_it()
    {
        await using var app = new TestApp();
        var client = app.CreateClient(new WebApplicationFactoryClientOptions { HandleCookies = false });
        await AuthApi.Register(client, Email);
        var cookie = CookieValue(await client.SendAsync(WebLogin()));

        var request = new HttpRequestMessage(HttpMethod.Post, "/account/refresh");
        request.Headers.Add("X-Client", "web");
        request.Headers.Add("Cookie", $"{Cookie}={cookie}");
        var response = await client.SendAsync(request);

        response.EnsureSuccessStatusCode();
        Assert.Contains(response.Headers.GetValues("Set-Cookie"), c => c.Contains(Cookie, StringComparison.Ordinal));
    }

    private static HttpRequestMessage WebLogin()
    {
        var request = new HttpRequestMessage(HttpMethod.Post, "/account/login")
        {
            Content = JsonContent.Create(new { email = Email, password = AuthApi.Password }),
        };
        request.Headers.Add("X-Client", "web");
        return request;
    }

    private static string CookieValue(HttpResponseMessage response)
    {
        var setCookie = response.Headers.GetValues("Set-Cookie").First(c => c.StartsWith(Cookie, StringComparison.Ordinal));
        var pair = setCookie.Split(';')[0];   // "golden_refresh=VALUE"
        return pair[(pair.IndexOf('=') + 1)..];
    }
}
