// Specifies the API-key check that protects POST /api/join.

using TeamsBot.Core.Hosting;
using Xunit;

namespace TeamsBot.Core.Tests.Hosting;

/// <summary>Tests for <see cref="ApiKeyVerifier"/>.</summary>
public sealed class ApiKeyVerifierTests
{
    private const string ConfiguredKey = "0123456789abcdef0123456789abcdef";

    /// <summary>Only the exact configured key is accepted.</summary>
    [Theory]
    [InlineData(ConfiguredKey, true)]
    [InlineData("0123456789abcdef0123456789abcdeF", false)]
    [InlineData("0123456789abcdef", false)]
    [InlineData("", false)]
    [InlineData(null, false)]
    public void AcceptsOnlyTheConfiguredKey(string? provided, bool expected)
    {
        Assert.Equal(expected, new ApiKeyVerifier(ConfiguredKey).IsAuthorized(provided));
    }

    /// <summary>Without a sufficiently long configured key the endpoint is disabled.</summary>
    [Theory]
    [InlineData(null)]
    [InlineData("")]
    [InlineData("short")]
    public void IsDisabledWithoutAStrongKey(string? configured)
    {
        var verifier = new ApiKeyVerifier(configured);

        Assert.False(verifier.IsEnabled);
        Assert.False(verifier.IsAuthorized(configured));
    }
}
