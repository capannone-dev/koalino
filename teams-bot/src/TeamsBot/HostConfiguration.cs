// Startup validation across all configuration sections, so a missing secret or an unsafe URL
// is reported once, clearly, before anything connects to Teams or the Rust app.

using TeamsBot.Core.Hosting;
using TeamsBot.Media;

namespace TeamsBot;

/// <summary>Validates the bot's configuration at startup.</summary>
internal static class HostConfiguration
{
    /// <summary>Collects every configuration error (names of settings only, never their values).</summary>
    /// <param name="bot">The <c>TeamsBot</c> section.</param>
    /// <param name="azureAd">The <c>AzureAd</c> section.</param>
    /// <param name="calling">The <c>TeamsBot:Calling</c> section.</param>
    /// <returns>The errors; empty when the host can start.</returns>
    public static IReadOnlyList<string> Validate(TeamsBotOptions bot, AzureAdOptions azureAd, CallingOptions calling)
    {
        var errors = new List<string>(bot.Validate());
        Require(errors, azureAd.TenantId, "AzureAd:TenantId");
        Require(errors, azureAd.ClientId, "AzureAd:ClientId");
        Require(errors, azureAd.ClientSecret, "AzureAd:ClientSecret (set AzureAd__ClientSecret)");
        Require(errors, calling.ServiceFqdn, "TeamsBot:Calling:ServiceFqdn");
        Require(errors, calling.CertificateThumbprint, "TeamsBot:Calling:CertificateThumbprint");
        if (calling.NotificationUrl is null || calling.NotificationUrl.Scheme != Uri.UriSchemeHttps)
        {
            errors.Add("TeamsBot:Calling:NotificationUrl must be the public https URL of /api/calling.");
        }
        if (calling.JoinTimeout <= TimeSpan.Zero)
        {
            errors.Add("TeamsBot:Calling:JoinTimeout must be positive.");
        }
        return errors;
    }

    /// <summary>Adds an error when a required value is empty.</summary>
    private static void Require(List<string> errors, string? value, string name)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            errors.Add($"{name} is required.");
        }
    }
}
