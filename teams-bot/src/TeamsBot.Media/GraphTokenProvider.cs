// Supplies the Graph calling SDK's built-in authentication provider with application tokens
// from TeamsBot.Core's token cache. The SDK attaches them to outbound requests and validates the
// signed JWT on every inbound /api/calling callback itself (issuer, audience = this app, keys
// from the calling service's OpenID metadata), rejecting forged callbacks with 401.

using Microsoft.Graph.Communications.Client.Authentication;
using TeamsBot.Core.Authentication;

namespace TeamsBot.Media;

/// <summary>Bridges <see cref="IAccessTokenProvider"/> to the SDK's <see cref="ITokenProvider"/>.</summary>
internal sealed class GraphTokenProvider(IAccessTokenProvider tokenProvider) : ITokenProvider
{
    /// <summary>Returns a cached or fresh Graph token.</summary>
    /// <param name="tenant">The tenant the SDK targets (the app is single-tenant, so it is ignored).</param>
    /// <returns>The bearer token (never logged).</returns>
    /// <exception cref="TokenAcquisitionException">No token could be acquired.</exception>
    /// <remarks>Side effects: May call Azure AD.</remarks>
    public Task<string> AcquireTokenAsync(string tenant) => tokenProvider.GetTokenAsync(CancellationToken.None);
}
