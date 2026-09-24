using System.Collections.Concurrent;
using System.Diagnostics.CodeAnalysis;

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

    /// <summary>The whole outcome recorded for <paramref name="key"/>, for a write whose replay is more than one
    /// balance (a transfer answers with both sides). Only an outcome of type <typeparamref name="T"/> matches.</summary>
    bool TryGetOutcome<T>(string key, [MaybeNullWhen(false)] out T outcome);

    /// <summary>Record the whole outcome of applying <paramref name="key"/> so a repeat replays it.</summary>
    void SaveOutcome<T>(string key, T outcome) where T : notnull;
}

/// <inheritdoc cref="IIdempotencyStore"/>
public sealed class InMemoryIdempotencyStore : IIdempotencyStore
{
    private readonly ConcurrentDictionary<string, decimal> _outcomes = new();

    // Whole outcomes live apart from the balances, so a key recorded by one kind of write never replays as another.
    private readonly ConcurrentDictionary<string, object> _records = new();

    /// <inheritdoc/>
    public bool TryGet(string key, out decimal balance) => _outcomes.TryGetValue(key, out balance);

    /// <inheritdoc/>
    public void Save(string key, decimal balance) => _outcomes[key] = balance;

    /// <inheritdoc/>
    public bool TryGetOutcome<T>(string key, [MaybeNullWhen(false)] out T outcome)
    {
        if (_records.TryGetValue(key, out var recorded) && recorded is T typed)
        {
            outcome = typed;
            return true;
        }

        outcome = default;
        return false;
    }

    /// <inheritdoc/>
    public void SaveOutcome<T>(string key, T outcome) where T : notnull => _records[key] = outcome;
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
