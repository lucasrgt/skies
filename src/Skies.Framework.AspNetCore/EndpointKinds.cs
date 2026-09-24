using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Http;

namespace Skies.Framework.AspNetCore;

/// <summary>
/// The closed vocabulary of an endpoint's <em>nature</em> — what kind of caller it is for. The default,
/// <see cref="App"/>, is the dominant case (a Skies app is UI-first): app-facing, so it must be wired by a
/// frontend (the <c>SKYFE008</c> coverage warning). The others are the legitimate exceptions that have no app
/// wiring and so leave the generated client: an <see cref="Asset"/> (a browser-loaded file URL), a
/// <see cref="Webhook"/> (third-party callback), and an <see cref="Internal"/> (server-to-server / dev-only).
/// Classification, not suppression — the marker says what the endpoint <em>is</em>; the handler says what it does.
/// </summary>
public enum EndpointKind
{
    /// <summary>App-facing — must be wired by a frontend (the default).</summary>
    App,

    /// <summary>A file or image fetched through a URL carried by another contract; never a generated data operation.</summary>
    Asset,

    /// <summary>A third-party callback (e.g. a payment webhook); never has UI wiring.</summary>
    Webhook,

    /// <summary>Server-to-server or dev-only; outside any client.</summary>
    Internal,
}

/// <summary>
/// Tags an endpoint with its <see cref="EndpointKind"/> so the contract carries the nature into OpenAPI, where
/// the client generator filters it: an <see cref="EndpointKind.Asset"/>, <see cref="EndpointKind.Webhook"/>, or
/// <see cref="EndpointKind.Internal"/>
/// endpoint is tagged and excluded from the app's generated client, so it never produces a hook and never trips
/// the loose-endpoint coverage warning. The marker is a builder call, <c>WithEndpointKind(EndpointKind.X)</c>, not
/// a class attribute, because a minimal-API handler is a lambda that cannot carry a class attribute to the
/// endpoint. App-facing is the default and needs no call (opt-out: only the exceptions are marked).
/// </summary>
public static class EndpointKindExtensions
{
    /// <summary>The OpenAPI tag a browser asset endpoint carries — the generator excludes it from the data client.</summary>
    public const string AssetTag = "skies:asset";

    /// <summary>The OpenAPI tag a webhook endpoint carries — the generator excludes it from the app client.</summary>
    public const string WebhookTag = "skies:webhook";

    /// <summary>The OpenAPI tag an internal endpoint carries — the generator excludes it from the app client.</summary>
    public const string InternalTag = "skies:internal";

    /// <summary>Mark an endpoint's nature. <see cref="EndpointKind.App"/> is a no-op (the default); an asset,
    /// webhook, or internal endpoint is tagged so the client generator leaves it out of the app SDK.</summary>
    /// <example>
    /// <code>
    /// app.MapPost("/charges/webhook", Handler).WithEndpointKind(EndpointKind.Webhook);
    /// </code>
    /// </example>
    public static RouteHandlerBuilder WithEndpointKind(this RouteHandlerBuilder builder, EndpointKind kind) =>
        kind switch
        {
            EndpointKind.Asset => builder.WithTags(AssetTag),
            EndpointKind.Webhook => builder.WithTags(WebhookTag),
            EndpointKind.Internal => builder.WithTags(InternalTag),
            _ => builder,
        };
}
