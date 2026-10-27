// Specifies the "TeamsBot" configuration section: issue defaults, mapping onto the component
// options, and refusal of settings that would stream media in plaintext over a network.

using TeamsBot.Core.Hosting;
using Xunit;

namespace TeamsBot.Core.Tests.Hosting;

/// <summary>Tests for <see cref="TeamsBotOptions"/>.</summary>
public sealed class TeamsBotOptionsTests
{
    /// <summary>Defaults match the issue: ws://localhost:8765, 1 fps, JPEG quality 80.</summary>
    [Fact]
    public void DefaultsMatchTheSpecification()
    {
        var options = new TeamsBotOptions();

        Assert.Equal(new Uri("ws://localhost:8765"), options.WebSocketUrl);
        Assert.Equal(1.0, options.VideoFps);
        Assert.Equal(80, options.VideoQuality);
        Assert.Empty(options.Validate());
    }

    /// <summary>Settings flow into the WebSocket client and video processor options.</summary>
    [Fact]
    public void MapsOntoComponentOptions()
    {
        var options = new TeamsBotOptions { WebSocketUrl = new Uri("ws://127.0.0.1:9000"), SharedSecret = "s3cret", VideoFps = 0.5, VideoQuality = 70 };

        var client = options.ToWebSocketClientOptions();
        var video = options.ToVideoProcessingOptions();

        Assert.Equal(new Uri("ws://127.0.0.1:9000"), client.Url);
        Assert.Equal("s3cret", client.SharedSecret);
        Assert.Equal(0.5, video.Fps);
        Assert.Equal(70, video.JpegQuality);
    }

    /// <summary>Plaintext WebSocket URLs are only allowed to loopback; invalid media settings are reported.</summary>
    [Theory]
    [InlineData("ws://192.168.1.20:8765", 1.0, 80, "wss")]
    [InlineData("http://localhost:8765", 1.0, 80, "ws")]
    [InlineData("ws://localhost:8765", 0.0, 80, "VideoFps")]
    [InlineData("ws://localhost:8765", 1.0, 101, "VideoQuality")]
    public void RejectsUnsafeOrInvalidSettings(string url, double fps, int quality, string expectedFragment)
    {
        var options = new TeamsBotOptions { WebSocketUrl = new Uri(url), VideoFps = fps, VideoQuality = quality };

        var error = Assert.Single(options.Validate());

        Assert.Contains(expectedFragment, error, StringComparison.Ordinal);
    }

    /// <summary>wss:// may point anywhere (e.g. through a TLS tunnel), and IPv6 loopback is local.</summary>
    [Theory]
    [InlineData("wss://bridge.example.com:8765")]
    [InlineData("ws://[::1]:8765")]
    public void AcceptsEncryptedOrLoopbackUrls(string url)
    {
        Assert.Empty(new TeamsBotOptions { WebSocketUrl = new Uri(url) }.Validate());
    }
}
