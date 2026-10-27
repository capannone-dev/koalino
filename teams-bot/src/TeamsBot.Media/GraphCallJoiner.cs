// Owns the Graph communications client and the Real-Time Media platform: builds them from the
// app registration and media hosting settings, joins meetings from their Teams links with an
// app-hosted media session (unmixed 16 kHz audio + VBSS NV12 screen share, receive-only), and
// relays the calling service's callbacks (/api/calling) into the SDK.
//
// Windows-only at runtime: constructing it initializes the native media platform.

using System.Diagnostics;
using System.Net;
using Microsoft.Extensions.Logging;
using Microsoft.Graph.Communications.Calls;
using Microsoft.Graph.Communications.Calls.Media;
using Microsoft.Graph.Communications.Client;
using Microsoft.Graph.Communications.Common.Telemetry;
using Microsoft.Graph.Communications.Resources;
using Microsoft.Graph.Models;
using Microsoft.Skype.Bots.Media;
using TeamsBot.Core.Abstractions;
using TeamsBot.Core.Authentication;
using TeamsBot.Core.Meeting;
using LogLevel = Microsoft.Extensions.Logging.LogLevel;

namespace TeamsBot.Media;

/// <summary>Joins Teams meetings through Microsoft Graph with app-hosted media.</summary>
public sealed partial class GraphCallJoiner : ICallJoiner, IAsyncDisposable
{
    private readonly ICommunicationsClient client;
    private readonly GraphLogger graphLogger;
    private readonly IDisposable graphLogSubscription;
    private readonly AzureAdOptions azureAd;
    private readonly CallingOptions calling;
    private readonly ILoggerFactory loggerFactory;
    private readonly ILogger<GraphCallJoiner> logger;

    /// <summary>Builds the communications client and initializes the media platform.</summary>
    /// <param name="azureAd">The app registration.</param>
    /// <param name="calling">Callback URL and media hosting settings.</param>
    /// <param name="tokenProvider">Cached Graph tokens (also refreshed by the 401 retry handler).</param>
    /// <param name="loggerFactory">Creates adapter loggers; SDK warnings and errors are forwarded too.</param>
    /// <exception cref="InvalidOperationException">The settings are incomplete or the public IP cannot be resolved.</exception>
    /// <remarks>Side effects: Initializes the native media platform (binds the media port); may resolve DNS.</remarks>
    public GraphCallJoiner(AzureAdOptions azureAd, CallingOptions calling, IAccessTokenProvider tokenProvider, ILoggerFactory loggerFactory)
    {
        ArgumentNullException.ThrowIfNull(azureAd);
        ArgumentNullException.ThrowIfNull(calling);
        ArgumentNullException.ThrowIfNull(tokenProvider);
        ArgumentNullException.ThrowIfNull(loggerFactory);
        if (calling.NotificationUrl is null || calling.NotificationUrl.Scheme != Uri.UriSchemeHttps)
        {
            throw new InvalidOperationException("TeamsBot:Calling:NotificationUrl must be a public https URL.");
        }
        this.azureAd = azureAd;
        this.calling = calling;
        this.loggerFactory = loggerFactory;
        logger = loggerFactory.CreateLogger<GraphCallJoiner>();

        graphLogger = new GraphLogger("NoteTakerTeamsBot", redirectToTrace: false);
        graphLogSubscription = graphLogger.Subscribe(new GraphLogForwarder(loggerFactory.CreateLogger("Microsoft.Graph.Communications")));

#pragma warning disable CA2000 // The communications client owns the HttpClient and its handler chain.
        var httpClient = new HttpClient(new UnauthorizedRetryHandler(tokenProvider) { InnerHandler = new SocketsHttpHandler() });
#pragma warning restore CA2000

        var builder = new CommunicationsClientBuilder("NoteTakerTeamsBot", azureAd.ClientId, graphLogger);
        // The SDK's built-in provider also validates the JWT on every inbound callback.
        builder.SetAuthentication(azureAd.ClientId, new GraphTokenProvider(tokenProvider));
        builder.SetNotificationUrl(calling.NotificationUrl);
        builder.SetServiceBaseUrl(new Uri("https://graph.microsoft.com/v1.0"));
        builder.SetHttpClient(httpClient, null);
        builder.SetMediaPlatformSettings(new MediaPlatformSettings
        {
            ApplicationId = azureAd.ClientId,
            MediaPlatformInstanceSettings = new MediaPlatformInstanceSettings
            {
                CertificateThumbprint = calling.CertificateThumbprint,
                InstanceInternalPort = calling.MediaInternalPort,
                InstancePublicPort = calling.MediaPublicPort,
                InstancePublicIPAddress = ResolvePublicAddress(calling),
                ServiceFqdn = calling.ServiceFqdn,
            },
        });
        client = builder.Build();
        LogClientReady(calling.ServiceFqdn, calling.MediaPublicPort);
    }

    /// <inheritdoc />
    /// <exception cref="ArgumentException">The link is not a Teams meeting link.</exception>
    /// <exception cref="TimeoutException">The call was not established within the join timeout (e.g. nobody admitted the bot from the lobby).</exception>
    /// <exception cref="InvalidOperationException">Teams terminated the call before it was established.</exception>
    /// <remarks>Side effects: Creates a media session and a Graph call; the bot appears in the meeting.</remarks>
    public async Task<ICallSession> JoinAsync(Uri joinUrl, CancellationToken cancellationToken)
    {
        if (!TeamsJoinUrl.TryParse(joinUrl, out var link, out var error))
        {
            throw new ArgumentException(error, nameof(joinUrl));
        }

        var mediaSession = client.CreateMediaSession(
            new AudioSocketSettings
            {
                StreamDirections = StreamDirection.Recvonly,
                SupportedAudioFormat = AudioFormat.Pcm16K,
                ReceiveUnmixedMeetingAudio = true,
            },
            new List<VideoSocketSettings>(),
            new VideoSocketSettings
            {
                StreamDirections = StreamDirection.Recvonly,
                ReceiveColorFormat = VideoColorFormat.NV12,
                MediaType = MediaType.Vbss,
            },
            null,
            Guid.NewGuid());

        var parameters = link switch
        {
            TeamsJoinUrl.ThreadMeeting thread => new JoinMeetingParameters(
                new ChatInfo { ThreadId = thread.ThreadId, MessageId = thread.MessageId },
                new OrganizerMeetingInfo
                {
                    Organizer = new IdentitySet
                    {
                        User = new Identity
                        {
                            Id = thread.OrganizerId,
                            AdditionalData = new Dictionary<string, object> { ["tenantId"] = thread.TenantId },
                        },
                    },
                },
                mediaSession)
            { TenantId = thread.TenantId },
            TeamsJoinUrl.MeetingIdMeeting meetingId => new JoinMeetingParameters(
                new ChatInfo(),
                new JoinMeetingIdMeetingInfo { JoinMeetingId = meetingId.JoinMeetingId, Passcode = meetingId.Passcode },
                mediaSession)
            { TenantId = azureAd.TenantId },
            _ => throw new ArgumentException("Unsupported Teams link.", nameof(joinUrl)),
        };

        var established = new TaskCompletionSource<bool>(TaskCreationOptions.RunContinuationsAsynchronously);
        var call = await client.Calls().AddAsync(parameters, Guid.NewGuid(), cancellationToken).ConfigureAwait(false);
        void OnUpdated(ICall sender, ResourceEventArgs<Call> args)
        {
            switch (args.NewResource?.State)
            {
                case CallState.Established:
                    established.TrySetResult(true);
                    break;
                case CallState.Terminated:
                    established.TrySetResult(false);
                    break;
            }
        }

        call.OnUpdated += OnUpdated;
        try
        {
            if (call.Resource?.State is CallState.Established or CallState.Terminated)
            {
                established.TrySetResult(call.Resource.State == CallState.Established);
            }
            using var timeout = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
            timeout.CancelAfter(calling.JoinTimeout);
            bool isEstablished;
            try
            {
                isEstablished = await established.Task.WaitAsync(timeout.Token).ConfigureAwait(false);
            }
            catch (OperationCanceledException) when (!cancellationToken.IsCancellationRequested)
            {
                await DeleteQuietlyAsync(call).ConfigureAwait(false);
                throw new TimeoutException($"The call was not established within {calling.JoinTimeout.TotalSeconds} s (is the bot waiting in the lobby?).");
            }
            if (!isEstablished)
            {
                throw new InvalidOperationException($"Teams ended the call before it was established (code {call.Resource?.ResultInfo?.Code}).");
            }
        }
        finally
        {
            call.OnUpdated -= OnUpdated;
        }

        LogCallEstablished(call.Id);
        return new GraphCallSession(call, mediaSession, azureAd.ClientId, loggerFactory.CreateLogger<GraphCallSession>());
    }

    /// <summary>Hands a calling-service callback to the SDK (which validates its JWT first).</summary>
    /// <param name="request">The callback, converted from the ASP.NET Core request.</param>
    /// <returns>The SDK's response (202 when accepted, 401 when the token is invalid).</returns>
    /// <remarks>Side effects: Updates call, roster, and media state; may raise session events.</remarks>
    public Task<HttpResponseMessage> ProcessNotificationAsync(HttpRequestMessage request) => client.ProcessNotificationAsync(request);

    /// <summary>Terminates all calls and shuts the media platform down.</summary>
    /// <returns>A task that completes when the client stopped.</returns>
    /// <remarks>Side effects: Hangs up active calls; releases the media port.</remarks>
    public async ValueTask DisposeAsync()
    {
        try
        {
            await client.TerminateAsync(TimeSpan.FromSeconds(10)).ConfigureAwait(false);
        }
        catch (Exception failure)
        {
            LogShutdownFailed(failure.GetType().Name);
        }
        client.Dispose();
        graphLogSubscription.Dispose();
    }

    /// <summary>Uses the configured public IP or resolves the FQDN.</summary>
    private static IPAddress ResolvePublicAddress(CallingOptions calling)
    {
        if (!string.IsNullOrWhiteSpace(calling.MediaPublicIpAddress))
        {
            return IPAddress.Parse(calling.MediaPublicIpAddress);
        }
        var address = Dns.GetHostAddresses(calling.ServiceFqdn).FirstOrDefault(candidate => candidate.AddressFamily == System.Net.Sockets.AddressFamily.InterNetwork);
        return address ?? throw new InvalidOperationException("TeamsBot:Calling:ServiceFqdn does not resolve to an IPv4 address; set MediaPublicIpAddress.");
    }

    /// <summary>Deletes a call, logging instead of throwing.</summary>
    private async Task DeleteQuietlyAsync(ICall call)
    {
        try
        {
            await call.DeleteAsync(false, CancellationToken.None).ConfigureAwait(false);
        }
        catch (Exception failure)
        {
            LogShutdownFailed(failure.GetType().Name);
        }
    }

    [LoggerMessage(Level = LogLevel.Information, Message = "Graph calling client ready; media at {ServiceFqdn}:{MediaPort}")]
    private partial void LogClientReady(string serviceFqdn, int mediaPort);

    [LoggerMessage(Level = LogLevel.Information, Message = "Call {CallId} established")]
    private partial void LogCallEstablished(string callId);

    [LoggerMessage(Level = LogLevel.Warning, Message = "Graph cleanup failed ({ErrorType})")]
    private partial void LogShutdownFailed(string errorType);

    /// <summary>Forwards the SDK's warnings and errors (component and level only, never payloads) to the host's logger.</summary>
    private sealed partial class GraphLogForwarder(ILogger logger) : IObserver<LogEvent>
    {
        public void OnCompleted()
        {
        }

        public void OnError(Exception error)
        {
        }

        public void OnNext(LogEvent value)
        {
            if (value.Level is TraceLevel.Error or TraceLevel.Warning)
            {
                // Messages can contain request bodies with participant names, so only the source is logged.
                LogSdkEvent(value.Level, value.Component ?? "sdk");
            }
        }

        [LoggerMessage(Level = LogLevel.Debug, Message = "Graph SDK reported a {Level} in {Component}")]
        private partial void LogSdkEvent(TraceLevel level, string component);
    }
}
