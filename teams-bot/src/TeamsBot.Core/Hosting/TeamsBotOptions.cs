// The "TeamsBot" configuration section (appsettings.json / TeamsBot__* environment variables):
// where to stream, the shared secret, and the screen-share sampling settings. It maps onto the
// component options and refuses a plaintext ws:// URL to anything but loopback, so media never
// crosses a network unencrypted.

using System.Net;
using TeamsBot.Core.Bridge;
using TeamsBot.Core.Video;

namespace TeamsBot.Core.Hosting;

/// <summary>Bot settings bound from the <c>TeamsBot</c> section.</summary>
public sealed class TeamsBotOptions
{
    /// <summary>The configuration section name.</summary>
    public const string SectionName = "TeamsBot";

    /// <summary>The Rust app's bridge endpoint (it listens on 127.0.0.1 only).</summary>
    public Uri WebSocketUrl { get; set; } = new("ws://localhost:8765");

    /// <summary>Must equal <c>NOTE_TAKER_TEAMS_BRIDGE_SECRET</c> of the Rust app (set via <c>TeamsBot__SharedSecret</c>; never logged).</summary>
    public string? SharedSecret { get; set; }

    /// <summary>Screen-share frames sampled per second.</summary>
    public double VideoFps { get; set; } = 1.0;

    /// <summary>JPEG quality of slides (1–100).</summary>
    public int VideoQuality { get; set; } = 80;

    /// <summary>Key required by <c>POST /api/join</c> (header <c>X-Api-Key</c>; at least 32 characters; never logged).</summary>
    public string? ApiKey { get; set; }

    /// <summary>Lists configuration problems.</summary>
    /// <returns>Human-readable errors (no secret values); empty when valid.</returns>
    public IReadOnlyList<string> Validate()
    {
        var errors = new List<string>();
        if (!WebSocketUrl.IsAbsoluteUri || (WebSocketUrl.Scheme != "ws" && WebSocketUrl.Scheme != "wss"))
        {
            errors.Add("TeamsBot:WebSocketUrl must be a ws:// or wss:// URL.");
        }
        else if (WebSocketUrl.Scheme == "ws" && !IsLoopback(WebSocketUrl))
        {
            errors.Add("TeamsBot:WebSocketUrl uses plaintext ws:// to a non-loopback host; use wss:// (e.g. through a TLS tunnel) or a loopback address.");
        }
        if (!(VideoFps > 0 && VideoFps <= VideoProcessingOptions.MaximumFps))
        {
            errors.Add($"TeamsBot:VideoFps must be greater than 0 and at most {VideoProcessingOptions.MaximumFps}.");
        }
        if (VideoQuality is < 1 or > 100)
        {
            errors.Add("TeamsBot:VideoQuality must be between 1 and 100.");
        }
        return errors;
    }

    /// <summary>Builds the WebSocket client settings (other values keep their defaults).</summary>
    /// <returns>The client options.</returns>
    public WebSocketClientOptions ToWebSocketClientOptions() => new() { Url = WebSocketUrl, SharedSecret = SharedSecret };

    /// <summary>Builds the screen-share settings.</summary>
    /// <returns>The video options.</returns>
    public VideoProcessingOptions ToVideoProcessingOptions() => new() { Fps = VideoFps, JpegQuality = VideoQuality };

    /// <summary>Whether the URL's host is the local machine.</summary>
    private static bool IsLoopback(Uri url) =>
        url.IsLoopback || (IPAddress.TryParse(url.IdnHost.Trim('[', ']'), out var address) && IPAddress.IsLoopback(address));
}
