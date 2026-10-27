// Pure mapping from Graph participant resources to TeamsBot.Core participant records: the Teams
// user ID (Azure AD object ID, else the call-scoped participant ID), the display name, the
// audio media source IDs used by unmixed audio, and the VBSS source of an active screen share.

using System.Globalization;
using Microsoft.Graph.Models;
using TeamsBot.Core.Model;
using CoreParticipantInfo = TeamsBot.Core.Model.ParticipantInfo;

namespace TeamsBot.Media;

/// <summary>Maps Graph participants to bridge identities.</summary>
internal static class ParticipantMapping
{
    /// <summary>Builds the bridge identity of a participant.</summary>
    /// <param name="participantId">The call-scoped Graph participant ID (fallback identity).</param>
    /// <param name="participant">The Graph participant.</param>
    /// <returns>The identity; guests and phone callers get a <c>teams-participant-…</c> ID.</returns>
    public static CoreParticipantInfo ToParticipantInfo(string participantId, Participant participant)
    {
        var identity = participant.Info?.Identity;
        var user = identity?.User;
        var teamsUserId = !string.IsNullOrWhiteSpace(user?.Id) ? user.Id : $"teams-participant-{participantId}";
        var displayName = user?.DisplayName
            ?? identity?.Application?.DisplayName
            ?? identity?.Device?.DisplayName
            ?? (identity?.AdditionalData is { } extra && extra.TryGetValue("guest", out var guest) && guest is Identity guestIdentity ? guestIdentity.DisplayName : null)
            ?? "Guest";
        return new CoreParticipantInfo(teamsUserId, displayName);
    }

    /// <summary>The media source IDs that tag this participant's audio.</summary>
    /// <param name="participant">The Graph participant.</param>
    /// <returns>Zero or more MSIs.</returns>
    public static IReadOnlyCollection<uint> AudioSourceIds(Participant participant) =>
        (participant.MediaStreams ?? new List<MediaStream>())
            .Where(stream => stream.MediaType == Modality.Audio)
            .Select(stream => ParseSourceId(stream.SourceId))
            .OfType<uint>()
            .ToArray();

    /// <summary>The VBSS source of a participant who is currently sharing their screen.</summary>
    /// <param name="participant">The Graph participant.</param>
    /// <returns>The MSI, or <c>null</c> when not sharing.</returns>
    public static uint? ScreenShareSourceId(Participant participant) =>
        (participant.MediaStreams ?? new List<MediaStream>())
            .Where(stream => stream.MediaType == Modality.VideoBasedScreenSharing
                && stream.Direction is MediaDirection.SendOnly or MediaDirection.SendReceive)
            .Select(stream => ParseSourceId(stream.SourceId))
            .FirstOrDefault(sourceId => sourceId is not null);

    /// <summary>Parses a Graph source ID string.</summary>
    private static uint? ParseSourceId(string? sourceId) =>
        uint.TryParse(sourceId, NumberStyles.None, CultureInfo.InvariantCulture, out var value) ? value : null;
}
