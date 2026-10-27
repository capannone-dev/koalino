// Caches the bot's Microsoft Graph access token (acquired by MSAL client credentials in the host)
// and refreshes it shortly before it expires, after a 401, and with retries on transient
// failures. Token values are never logged or included in exception messages.

using Microsoft.Extensions.Logging;

namespace TeamsBot.Core.Authentication;

/// <summary>An access token and its expiry.</summary>
/// <param name="Value">The bearer token (secret; never logged).</param>
/// <param name="ExpiresOn">When the token stops being accepted.</param>
public sealed record AccessToken(string Value, DateTimeOffset ExpiresOn)
{
    /// <summary>Describes the token without revealing its value.</summary>
    /// <returns>A redacted description.</returns>
    public override string ToString() => $"AccessToken {{ ExpiresOn = {ExpiresOn:O} }}";
}

/// <summary>Supplies bearer tokens for Microsoft Graph requests.</summary>
public interface IAccessTokenProvider
{
    /// <summary>Returns a token that is valid for at least the refresh margin.</summary>
    /// <param name="cancellationToken">Cancels the acquisition.</param>
    /// <returns>The bearer token.</returns>
    /// <exception cref="TokenAcquisitionException">No token could be acquired.</exception>
    Task<string> GetTokenAsync(CancellationToken cancellationToken);

    /// <summary>Discards the cached token (call after Graph answers 401), forcing the next call to acquire a new one.</summary>
    void Invalidate();
}

/// <summary>Raised when every attempt to acquire an access token failed.</summary>
public sealed class TokenAcquisitionException : Exception
{
    /// <summary>Creates the exception with a default message.</summary>
    public TokenAcquisitionException()
        : base("No access token could be acquired.")
    {
    }

    /// <summary>Creates the exception.</summary>
    /// <param name="message">A message without secrets.</param>
    public TokenAcquisitionException(string message)
        : base(message)
    {
    }

    /// <summary>Creates the exception wrapping the last failure.</summary>
    /// <param name="message">A message without secrets.</param>
    /// <param name="innerException">The last acquisition failure.</param>
    public TokenAcquisitionException(string message, Exception innerException)
        : base(message, innerException)
    {
    }
}

/// <summary>Caches tokens from an acquisition function and refreshes them before they expire.</summary>
public sealed partial class CachingAccessTokenProvider : IAccessTokenProvider, IDisposable
{
    /// <summary>A token is refreshed once it expires within this margin.</summary>
    public static readonly TimeSpan RefreshMargin = TimeSpan.FromMinutes(5);

    private static readonly TimeSpan InitialRetryDelay = TimeSpan.FromSeconds(1);

    private readonly Func<CancellationToken, Task<AccessToken>> acquireAsync;
    private readonly ILogger<CachingAccessTokenProvider> logger;
    private readonly Func<DateTimeOffset> clock;
    private readonly int maximumAttempts;
    private readonly Func<TimeSpan, CancellationToken, Task> delayAsync;

    /// <summary>Serializes acquisitions so concurrent callers share one request.</summary>
    private readonly SemaphoreSlim acquisitionGate = new(1, 1);

    private volatile AccessToken? cachedToken;

    /// <summary>Creates the provider.</summary>
    /// <param name="acquireAsync">Acquires a fresh token (MSAL client credentials in production).</param>
    /// <param name="logger">Receives refresh and failure events (never token values).</param>
    /// <param name="clock">The current time; defaults to <see cref="DateTimeOffset.UtcNow"/>.</param>
    /// <param name="maximumAttempts">Acquisition attempts per call (at least 1).</param>
    /// <param name="delayAsync">Waits between attempts; defaults to <see cref="Task.Delay(TimeSpan, CancellationToken)"/>.</param>
    /// <exception cref="ArgumentOutOfRangeException"><paramref name="maximumAttempts"/> is below 1.</exception>
    /// <remarks>Side effects: None until the first call.</remarks>
    public CachingAccessTokenProvider(
        Func<CancellationToken, Task<AccessToken>> acquireAsync,
        ILogger<CachingAccessTokenProvider> logger,
        Func<DateTimeOffset>? clock = null,
        int maximumAttempts = 3,
        Func<TimeSpan, CancellationToken, Task>? delayAsync = null)
    {
        ArgumentNullException.ThrowIfNull(acquireAsync);
        ArgumentNullException.ThrowIfNull(logger);
        ArgumentOutOfRangeException.ThrowIfLessThan(maximumAttempts, 1);
        this.acquireAsync = acquireAsync;
        this.logger = logger;
        this.clock = clock ?? (() => DateTimeOffset.UtcNow);
        this.maximumAttempts = maximumAttempts;
        this.delayAsync = delayAsync ?? Task.Delay;
    }

    /// <inheritdoc />
    /// <remarks>Side effects: May call the token endpoint (with exponential retry delays) and log failures.</remarks>
    public async Task<string> GetTokenAsync(CancellationToken cancellationToken)
    {
        if (IsFresh(cachedToken))
        {
            return cachedToken!.Value;
        }

        await acquisitionGate.WaitAsync(cancellationToken).ConfigureAwait(false);
        try
        {
            var current = cachedToken;
            if (IsFresh(current))
            {
                return current!.Value;
            }

            Exception? lastFailure = null;
            var retryDelay = InitialRetryDelay;
            for (var attempt = 1; attempt <= maximumAttempts; attempt++)
            {
                try
                {
                    var token = await acquireAsync(cancellationToken).ConfigureAwait(false);
                    cachedToken = token;
                    LogTokenAcquired(token.ExpiresOn);
                    return token.Value;
                }
                catch (Exception failure) when (failure is not OperationCanceledException || !cancellationToken.IsCancellationRequested)
                {
                    lastFailure = failure;
                    LogAcquisitionFailed(attempt, maximumAttempts, failure.GetType().Name);
                }

                if (attempt < maximumAttempts)
                {
                    await delayAsync(retryDelay, cancellationToken).ConfigureAwait(false);
                    retryDelay *= 2;
                }
            }

            throw new TokenAcquisitionException($"No access token could be acquired after {maximumAttempts} attempts.", lastFailure!);
        }
        finally
        {
            acquisitionGate.Release();
        }
    }

    /// <inheritdoc />
    /// <remarks>Side effects: Clears the cache.</remarks>
    public void Invalidate()
    {
        cachedToken = null;
        LogInvalidated();
    }

    /// <summary>Releases the acquisition gate.</summary>
    public void Dispose() => acquisitionGate.Dispose();

    /// <summary>Whether <paramref name="token"/> is valid beyond the refresh margin.</summary>
    private bool IsFresh(AccessToken? token) => token is not null && token.ExpiresOn - clock() > RefreshMargin;

    [LoggerMessage(Level = LogLevel.Information, Message = "Graph access token acquired; it expires at {ExpiresOn:O}")]
    private partial void LogTokenAcquired(DateTimeOffset expiresOn);

    [LoggerMessage(Level = LogLevel.Warning, Message = "Graph access token acquisition attempt {Attempt}/{MaximumAttempts} failed ({ErrorType})")]
    private partial void LogAcquisitionFailed(int attempt, int maximumAttempts, string errorType);

    [LoggerMessage(Level = LogLevel.Information, Message = "Graph access token invalidated; the next request acquires a new one")]
    private partial void LogInvalidated();
}
