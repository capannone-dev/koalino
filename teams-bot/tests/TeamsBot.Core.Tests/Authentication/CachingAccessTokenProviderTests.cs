// Specifies the Graph access-token cache: reuse until shortly before expiry, forced refresh after
// a 401, retries of transient acquisition failures, and a single acquisition for concurrent callers.

using TeamsBot.Core.Authentication;
using TeamsBot.Core.Tests.Support;
using Xunit;

namespace TeamsBot.Core.Tests.Authentication;

/// <summary>Tests for <see cref="CachingAccessTokenProvider"/>.</summary>
public sealed class CachingAccessTokenProviderTests
{
    private DateTimeOffset now = new(2026, 10, 27, 12, 0, 0, TimeSpan.Zero);
    private int acquisitions;

    /// <summary>Creates a provider whose source returns token-N valid for one hour, failing the first <paramref name="failures"/> times.</summary>
    private CachingAccessTokenProvider Provider(int failures = 0, int maximumAttempts = 3) => new(
        async cancellationToken =>
        {
            await Task.Yield();
            var attempt = Interlocked.Increment(ref acquisitions);
            if (attempt <= failures)
            {
                throw new HttpRequestException("token endpoint unavailable");
            }
            return new AccessToken($"token-{attempt}", now.AddHours(1));
        },
        new RecordingLogger<CachingAccessTokenProvider>(),
        () => now,
        maximumAttempts,
        (delay, cancellationToken) => Task.CompletedTask);

    /// <summary>A cached token is reused until five minutes before it expires.</summary>
    [Fact]
    public async Task TokensAreReusedUntilShortlyBeforeExpiry()
    {
        var provider = Provider();
        Assert.Equal("token-1", await provider.GetTokenAsync(CancellationToken.None));

        now = now.AddMinutes(50);
        Assert.Equal("token-1", await provider.GetTokenAsync(CancellationToken.None));

        now = now.AddMinutes(6);
        Assert.Equal("token-2", await provider.GetTokenAsync(CancellationToken.None));
    }

    /// <summary>After a 401, invalidating forces a fresh token.</summary>
    [Fact]
    public async Task InvalidateForcesARefresh()
    {
        var provider = Provider();
        await provider.GetTokenAsync(CancellationToken.None);

        provider.Invalidate();

        Assert.Equal("token-2", await provider.GetTokenAsync(CancellationToken.None));
    }

    /// <summary>Transient failures are retried; exhausting the attempts raises a token error.</summary>
    [Fact]
    public async Task TransientFailuresAreRetried()
    {
        Assert.Equal("token-3", await Provider(failures: 2).GetTokenAsync(CancellationToken.None));

        acquisitions = 0;
        await Assert.ThrowsAsync<TokenAcquisitionException>(() => Provider(failures: 5).GetTokenAsync(CancellationToken.None));
    }

    /// <summary>Concurrent callers share one acquisition.</summary>
    [Fact]
    public async Task ConcurrentCallersShareOneAcquisition()
    {
        var provider = Provider();

        var tokens = await Task.WhenAll(Enumerable.Range(0, 10).Select(_ => provider.GetTokenAsync(CancellationToken.None)));

        Assert.All(tokens, token => Assert.Equal("token-1", token));
        Assert.Equal(1, acquisitions);
    }
}
