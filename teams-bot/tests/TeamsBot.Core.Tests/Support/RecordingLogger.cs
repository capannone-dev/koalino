// Test helper: an ILogger that records formatted entries so tests can assert on warnings and
// verify that secrets or media never appear in logs.

using System.Collections.Concurrent;
using Microsoft.Extensions.Logging;

namespace TeamsBot.Core.Tests.Support;

/// <summary>Records every log entry in memory.</summary>
/// <typeparam name="T">The category type.</typeparam>
public sealed class RecordingLogger<T> : ILogger<T>
{
    private readonly ConcurrentQueue<(LogLevel Level, string Message)> entries = new();

    /// <summary>All recorded entries, oldest first.</summary>
    public IReadOnlyList<(LogLevel Level, string Message)> Entries => entries.ToArray();

    /// <summary>Whether an entry at <paramref name="level"/> contains <paramref name="text"/>.</summary>
    /// <param name="level">The level to match.</param>
    /// <param name="text">A substring of the formatted message.</param>
    /// <returns><c>true</c> when such an entry exists.</returns>
    public bool Contains(LogLevel level, string text) =>
        entries.Any(entry => entry.Level == level && entry.Message.Contains(text, StringComparison.Ordinal));

    /// <summary>Whether any entry contains <paramref name="text"/>.</summary>
    /// <param name="text">A substring of the formatted message.</param>
    /// <returns><c>true</c> when such an entry exists.</returns>
    public bool ContainsAnywhere(string text) =>
        entries.Any(entry => entry.Message.Contains(text, StringComparison.Ordinal));

    /// <inheritdoc />
    public IDisposable? BeginScope<TState>(TState state)
        where TState : notnull => null;

    /// <inheritdoc />
    public bool IsEnabled(LogLevel logLevel) => true;

    /// <inheritdoc />
    public void Log<TState>(LogLevel logLevel, EventId eventId, TState state, Exception? exception, Func<TState, Exception?, string> formatter)
    {
        ArgumentNullException.ThrowIfNull(formatter);
        entries.Enqueue((logLevel, formatter(state, exception) + (exception is null ? string.Empty : " " + exception.Message)));
    }
}
