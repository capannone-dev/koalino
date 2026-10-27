// Parses a Teams meeting "Join" link into what Graph needs to join it: either the chat thread,
// tenant, and organizer of a classic meetup-join link, or the numeric meeting ID and passcode of
// a short /meet/ link. Only https links on Teams hosts are accepted, so the join endpoint
// cannot be pointed at arbitrary URLs. Links are never logged (they grant meeting access).

using System.Diagnostics.CodeAnalysis;
using System.Text.Json;

namespace TeamsBot.Core.Meeting;

/// <summary>A parsed Teams join link.</summary>
public abstract record TeamsJoinUrl
{
    /// <summary>Hosts that serve Teams meeting links (commercial and US government clouds).</summary>
    private static readonly string[] TeamsHosts =
    {
        "teams.microsoft.com",
        "teams.microsoft.us",
        "gov.teams.microsoft.us",
        "dod.teams.microsoft.us",
    };

    /// <summary>Prevents subclasses outside this file.</summary>
    private protected TeamsJoinUrl()
    {
    }

    /// <summary>Parses a join link.</summary>
    /// <param name="joinUrl">The link from the meeting invitation.</param>
    /// <param name="parsed">The parsed link, or <c>null</c>.</param>
    /// <param name="error">Why the link was refused (no link content), or empty.</param>
    /// <returns><c>true</c> when the link is a Teams meeting link.</returns>
    public static bool TryParse(Uri joinUrl, [NotNullWhen(true)] out TeamsJoinUrl? parsed, out string error)
    {
        ArgumentNullException.ThrowIfNull(joinUrl);
        parsed = null;
        if (!joinUrl.IsAbsoluteUri || joinUrl.Scheme != Uri.UriSchemeHttps)
        {
            error = "Join links must use https.";
            return false;
        }
        if (!TeamsHosts.Contains(joinUrl.IdnHost, StringComparer.OrdinalIgnoreCase))
        {
            error = "The link's host is not a Teams host.";
            return false;
        }

        var segments = joinUrl.AbsolutePath.Split('/', StringSplitOptions.RemoveEmptyEntries);
        var query = ParseQuery(joinUrl.Query);
        if (segments is ["l", "meetup-join", var thread, var message, ..])
        {
            return TryParseThreadMeeting(Uri.UnescapeDataString(thread), Uri.UnescapeDataString(message), query, out parsed, out error);
        }
        if (segments is ["meet", var meetingId])
        {
            if (meetingId.Length is < 6 or > 20 || !meetingId.All(char.IsAsciiDigit))
            {
                error = "The meeting ID must be 6 to 20 digits.";
                return false;
            }
            parsed = new MeetingIdMeeting(meetingId, query.GetValueOrDefault("p"));
            error = string.Empty;
            return true;
        }

        error = "The link is not a Teams meeting link.";
        return false;
    }

    /// <summary>Reads the tenant and organizer from the <c>context</c> JSON of a meetup-join link.</summary>
    private static bool TryParseThreadMeeting(
        string threadId,
        string messageId,
        Dictionary<string, string> query,
        [NotNullWhen(true)] out TeamsJoinUrl? parsed,
        out string error)
    {
        parsed = null;
        if (!query.TryGetValue("context", out var context))
        {
            error = "The link is missing the tenant (no context parameter).";
            return false;
        }

        string? tenantId;
        string? organizerId;
        try
        {
            using var document = JsonDocument.Parse(context);
            if (document.RootElement.ValueKind != JsonValueKind.Object)
            {
                error = "The link's context is not a JSON object.";
                return false;
            }
            tenantId = ReadString(document.RootElement, "Tid");
            organizerId = ReadString(document.RootElement, "Oid");
        }
        catch (JsonException)
        {
            error = "The link's context is not valid JSON.";
            return false;
        }

        if (string.IsNullOrWhiteSpace(tenantId))
        {
            error = "The link is missing the tenant (Tid).";
            return false;
        }
        parsed = new ThreadMeeting(threadId, messageId, tenantId, organizerId);
        error = string.Empty;
        return true;
    }

    /// <summary>Reads a string property, or <c>null</c>.</summary>
    private static string? ReadString(JsonElement element, string name) =>
        element.TryGetProperty(name, out var value) && value.ValueKind == JsonValueKind.String ? value.GetString() : null;

    /// <summary>Splits a query string into unescaped key/value pairs (first value wins).</summary>
    private static Dictionary<string, string> ParseQuery(string query)
    {
        var values = new Dictionary<string, string>(StringComparer.Ordinal);
        foreach (var pair in query.TrimStart('?').Split('&', StringSplitOptions.RemoveEmptyEntries))
        {
            var separator = pair.IndexOf('=', StringComparison.Ordinal);
            var key = Uri.UnescapeDataString(separator < 0 ? pair : pair[..separator]);
            var value = separator < 0 ? string.Empty : Uri.UnescapeDataString(pair[(separator + 1)..].Replace('+', ' '));
            values.TryAdd(key, value);
        }
        return values;
    }

    /// <summary>A classic meetup-join link.</summary>
    /// <param name="ThreadId">The meeting chat thread (e.g. <c>19:meeting_…@thread.v2</c>).</param>
    /// <param name="MessageId">The chat message ID (usually <c>0</c>).</param>
    /// <param name="TenantId">The organizer's Azure AD tenant.</param>
    /// <param name="OrganizerId">The organizer's Azure AD object ID, when present.</param>
    public sealed record ThreadMeeting(string ThreadId, string MessageId, string TenantId, string? OrganizerId) : TeamsJoinUrl;

    /// <summary>A short <c>/meet/</c> link.</summary>
    /// <param name="JoinMeetingId">The numeric meeting ID.</param>
    /// <param name="Passcode">The passcode (<c>p</c> parameter), when present.</param>
    public sealed record MeetingIdMeeting(string JoinMeetingId, string? Passcode) : TeamsJoinUrl
    {
        /// <summary>Describes the link without its passcode.</summary>
        /// <returns>A redacted description.</returns>
        public override string ToString() => $"MeetingIdMeeting {{ JoinMeetingId = {JoinMeetingId} }}";
    }
}
