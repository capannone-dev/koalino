// HTTP handler placed in front of the Graph SDK's HttpClient: when Graph answers 401 (typically
// an access token that expired or was revoked mid-call), it invalidates the cached token,
// acquires a fresh one, and resends the request once with the same body.

using System.Net;
using System.Net.Http.Headers;

namespace TeamsBot.Core.Authentication;

/// <summary>Retries a request once with a fresh bearer token after a 401.</summary>
public sealed class UnauthorizedRetryHandler : DelegatingHandler
{
    private readonly IAccessTokenProvider tokenProvider;

    /// <summary>Creates the handler; set <see cref="DelegatingHandler.InnerHandler"/> (or use it with an HttpClient factory).</summary>
    /// <param name="tokenProvider">The token cache to invalidate and refresh.</param>
    public UnauthorizedRetryHandler(IAccessTokenProvider tokenProvider)
    {
        ArgumentNullException.ThrowIfNull(tokenProvider);
        this.tokenProvider = tokenProvider;
    }

    /// <summary>Sends the request; on 401, refreshes the token and resends a copy once.</summary>
    /// <param name="request">The outgoing request (already authorized by the SDK).</param>
    /// <param name="cancellationToken">Cancels the request.</param>
    /// <returns>The response of the last attempt.</returns>
    /// <exception cref="TokenAcquisitionException">The fresh token could not be acquired.</exception>
    /// <remarks>Side effects: Buffers the request body; may invalidate and refresh the token cache.</remarks>
    protected override async Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(request);
        var body = request.Content is null ? null : await request.Content.ReadAsByteArrayAsync(cancellationToken).ConfigureAwait(false);

        var response = await base.SendAsync(request, cancellationToken).ConfigureAwait(false);
        if (response.StatusCode != HttpStatusCode.Unauthorized)
        {
            return response;
        }

        response.Dispose();
        tokenProvider.Invalidate();
        var freshToken = await tokenProvider.GetTokenAsync(cancellationToken).ConfigureAwait(false);
        using var retry = Copy(request, body);
        retry.Headers.Authorization = new AuthenticationHeaderValue("Bearer", freshToken);
        return await base.SendAsync(retry, cancellationToken).ConfigureAwait(false);
    }

    /// <summary>Copies the request line, headers, options, and buffered body (a sent request cannot be resent).</summary>
    private static HttpRequestMessage Copy(HttpRequestMessage request, byte[]? body)
    {
        var copy = new HttpRequestMessage(request.Method, request.RequestUri) { Version = request.Version, VersionPolicy = request.VersionPolicy };
        foreach (var header in request.Headers)
        {
            copy.Headers.TryAddWithoutValidation(header.Key, header.Value);
        }
        foreach (var option in request.Options)
        {
            ((IDictionary<string, object?>)copy.Options)[option.Key] = option.Value;
        }
        if (body is not null && request.Content is not null)
        {
            copy.Content = new ByteArrayContent(body);
            foreach (var header in request.Content.Headers)
            {
                copy.Content.Headers.TryAddWithoutValidation(header.Key, header.Value);
            }
        }
        return copy;
    }
}
