using System.Collections.Concurrent;

namespace Sample.Api;

/// <summary>
/// Demo-grade idempotency for write slices: it remembers the outcome of an Idempotency-Key so a retried
/// request replays that outcome instead of applying the write twice. In-memory and process-local — a real app
/// backs this with a durable store (a table, Redis) and scopes the key to the operation.
/// </summary>
public interface IIdempotencyStore
{
    /// <summary>The balance recorded for <paramref name="key"/>, if this key was already applied.</summary>
    bool TryGet(string key, out decimal balance);

    /// <summary>Record the outcome of applying <paramref name="key"/> so a repeat replays it.</summary>
    void Save(string key, decimal balance);
}

/// <inheritdoc cref="IIdempotencyStore"/>
public sealed class InMemoryIdempotencyStore : IIdempotencyStore
{
    private readonly ConcurrentDictionary<string, decimal> _outcomes = new();

    /// <inheritdoc/>
    public bool TryGet(string key, out decimal balance) => _outcomes.TryGetValue(key, out balance);

    /// <inheritdoc/>
    public void Save(string key, decimal balance) => _outcomes[key] = balance;
}

public static partial class Platform
{
    /// <summary>Register the demo idempotency store (a singleton — it remembers keys for the process lifetime).</summary>
    public static IServiceCollection AddIdempotency(this IServiceCollection services)
    {
        services.AddSingleton<IIdempotencyStore, InMemoryIdempotencyStore>();
        return services;
    }
}
