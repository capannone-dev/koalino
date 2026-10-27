// Acquires Microsoft Graph application tokens with MSAL client credentials. The token cache and
// refresh policy live in TeamsBot.Core's CachingAccessTokenProvider; MSAL's own in-memory cache
// additionally avoids redundant calls to the token endpoint.

using Microsoft.Identity.Client;
using TeamsBot.Core.Authentication;

namespace TeamsBot.Media;

/// <summary>Client-credentials token acquisition for Graph.</summary>
public sealed class MsalTokenSource
{
    /// <summary>Requests every application permission granted to the app (Calls.*).</summary>
    private static readonly string[] GraphScopes = { "https://graph.microsoft.com/.default" };

    private readonly IConfidentialClientApplication application;

    /// <summary>Creates the source for an app registration.</summary>
    /// <param name="options">Tenant, client ID, and secret.</param>
    /// <remarks>Side effects: None until the first acquisition.</remarks>
    public MsalTokenSource(AzureAdOptions options)
    {
        ArgumentNullException.ThrowIfNull(options);
        application = ConfidentialClientApplicationBuilder
            .Create(options.ClientId)
            .WithClientSecret(options.ClientSecret)
            .WithAuthority(AzureCloudInstance.AzurePublic, options.TenantId)
            .Build();
    }

    /// <summary>Acquires a Graph token.</summary>
    /// <param name="cancellationToken">Cancels the request.</param>
    /// <returns>The token and its expiry.</returns>
    /// <exception cref="MsalException">Azure AD refused the request (wrong secret, missing consent, …).</exception>
    /// <remarks>Side effects: Calls the Azure AD token endpoint unless MSAL's cache has a valid token.</remarks>
    public async Task<AccessToken> AcquireAsync(CancellationToken cancellationToken)
    {
        var result = await application.AcquireTokenForClient(GraphScopes).ExecuteAsync(cancellationToken).ConfigureAwait(false);
        return new AccessToken(result.AccessToken, result.ExpiresOn);
    }
}
