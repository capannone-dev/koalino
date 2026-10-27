// Test helper: an ILoggerFactory whose loggers all record into one shared list.

using System.Collections.Concurrent;
using Microsoft.Extensions.Logging;

namespace TeamsBot.Core.Tests.Support;

/// <summary>Creates loggers that record into one shared list.</summary>
public sealed class RecordingLoggerFactory : ILoggerFactory
{
    private readonly ConcurrentQueue<(LogLevel Level, string Message)> entries = new();

    /// <summary>Whether an entry at <paramref name="level"/> contains <paramref name="text"/> (case-insensitive).</summary>
    /// <param name="level">The level to match.</param>
    /// <param name="text">A substring of the formatted message.</param>
    /// <returns><c>true</c> when such an entry exists.</returns>
    public bool Contains(LogLevel level, string text) =>
        entries.Any(entry => entry.Level == level && entry.Message.Contains(text, StringComparison.OrdinalIgnoreCase));

    /// <inheritdoc />
    public void AddProvider(ILoggerProvider provider)
    {
    }

    /// <inheritdoc />
    public ILogger CreateLogger(string categoryName) => new SharedLogger(entries);

    /// <inheritdoc />
    public void Dispose()
    {
    }

    /// <summary>A logger writing into the shared list.</summary>
    private sealed class SharedLogger(ConcurrentQueue<(LogLevel Level, string Message)> entries) : ILogger
    {
        public IDisposable? BeginScope<TState>(TState state)
            where TState : notnull => null;

        public bool IsEnabled(LogLevel logLevel) => true;

        public void Log<TState>(LogLevel logLevel, EventId eventId, TState state, Exception? exception, Func<TState, Exception?, string> formatter) =>
            entries.Enqueue((logLevel, formatter(state, exception)));
    }
}
