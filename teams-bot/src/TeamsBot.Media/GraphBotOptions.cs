// Settings for the Graph calling client and the Real-Time Media platform, bound from the
// "AzureAd" and "TeamsBot" configuration sections by the host. Secrets come from environment
// variables or a secret store, never from checked-in files.

using System.ComponentModel.DataAnnotations;

namespace TeamsBot.Media;

/// <summary>Azure AD app registration used for Graph calls (section <c>AzureAd</c>).</summary>
public sealed class AzureAdOptions
{
    /// <summary>The configuration section name.</summary>
    public const string SectionName = "AzureAd";

    /// <summary>The directory (tenant) ID of the app registration.</summary>
    [Required]
    public string TenantId { get; set; } = string.Empty;

    /// <summary>The application (client) ID; also the Azure Bot's Microsoft App ID.</summary>
    [Required]
    public string ClientId { get; set; } = string.Empty;

    /// <summary>The client secret (set through <c>AzureAd__ClientSecret</c>; never logged).</summary>
    [Required]
    public string ClientSecret { get; set; } = string.Empty;
}

/// <summary>Calling and media hosting settings (section <c>TeamsBot:Calling</c>).</summary>
public sealed class CallingOptions
{
    /// <summary>The configuration section name.</summary>
    public const string SectionName = "TeamsBot:Calling";

    /// <summary>Public https URL of the <c>/api/calling</c> endpoint, as registered on the Azure Bot's Teams channel.</summary>
    [Required]
    public Uri? NotificationUrl { get; set; }

    /// <summary>Public DNS name of this host; it must match the media certificate's subject.</summary>
    [Required]
    public string ServiceFqdn { get; set; } = string.Empty;

    /// <summary>Thumbprint of the media certificate in <c>LocalMachine\My</c>.</summary>
    [Required]
    public string CertificateThumbprint { get; set; } = string.Empty;

    /// <summary>Public TCP port Teams connects to for media.</summary>
    [Range(1, 65535)]
    public int MediaPublicPort { get; set; } = 8445;

    /// <summary>Local TCP port the media platform listens on (behind any NAT or load balancer).</summary>
    [Range(1, 65535)]
    public int MediaInternalPort { get; set; } = 8445;

    /// <summary>Public IP address of this host; resolved from <see cref="ServiceFqdn"/> when empty.</summary>
    public string? MediaPublicIpAddress { get; set; }

    /// <summary>How long a join may take, including time in the lobby.</summary>
    public TimeSpan JoinTimeout { get; set; } = TimeSpan.FromSeconds(90);
}
