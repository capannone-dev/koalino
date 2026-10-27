// Thread-safe map from media source IDs (MSIs) to roster participants, updated from roster
// events on the Graph notification thread and read by the audio processor on media threads.

using System.Diagnostics.CodeAnalysis;
using TeamsBot.Core.Abstractions;
using TeamsBot.Core.Model;

namespace TeamsBot.Core.Meeting;

/// <summary>The live roster of one meeting.</summary>
internal sealed class RosterDirectory : IParticipantDirectory
{
    private readonly object gate = new();

    /// <summary>Participants keyed by Teams user ID, in join order.</summary>
    private readonly List<RosterParticipant> participants = new();

    /// <summary>Audio MSI → participant.</summary>
    private readonly Dictionary<uint, ParticipantInfo> participantsBySource = new();

    /// <summary>Adds or replaces entries (matched by Teams user ID).</summary>
    /// <param name="joined">New or updated entries.</param>
    /// <returns>The entries that were not present before (for join logging).</returns>
    /// <remarks>Side effects: Rebuilds the source map.</remarks>
    public IReadOnlyList<RosterParticipant> AddOrUpdate(IEnumerable<RosterParticipant> joined)
    {
        var added = new List<RosterParticipant>();
        lock (gate)
        {
            foreach (var entry in joined)
            {
                var index = participants.FindIndex(existing => existing.Participant.TeamsUserId == entry.Participant.TeamsUserId);
                if (index >= 0)
                {
                    participants[index] = entry;
                }
                else
                {
                    participants.Add(entry);
                    added.Add(entry);
                }
            }
            RebuildSources();
        }
        return added;
    }

    /// <summary>Removes entries (matched by Teams user ID).</summary>
    /// <param name="left">Departed entries.</param>
    /// <returns>The entries that were actually removed.</returns>
    /// <remarks>Side effects: Rebuilds the source map.</remarks>
    public IReadOnlyList<RosterParticipant> Remove(IEnumerable<RosterParticipant> left)
    {
        var removed = new List<RosterParticipant>();
        lock (gate)
        {
            foreach (var entry in left)
            {
                var index = participants.FindIndex(existing => existing.Participant.TeamsUserId == entry.Participant.TeamsUserId);
                if (index >= 0)
                {
                    removed.Add(participants[index]);
                    participants.RemoveAt(index);
                }
            }
            RebuildSources();
        }
        return removed;
    }

    /// <summary>A snapshot of the current participants, in join order.</summary>
    /// <returns>The participants.</returns>
    public IReadOnlyList<ParticipantInfo> Snapshot()
    {
        lock (gate)
        {
            return participants.Select(entry => entry.Participant).ToArray();
        }
    }

    /// <inheritdoc />
    public bool TryResolve(uint mediaSourceId, [NotNullWhen(true)] out ParticipantInfo? participant)
    {
        lock (gate)
        {
            return participantsBySource.TryGetValue(mediaSourceId, out participant);
        }
    }

    /// <summary>Recomputes the MSI map; caller holds the lock.</summary>
    private void RebuildSources()
    {
        participantsBySource.Clear();
        foreach (var entry in participants)
        {
            foreach (var sourceId in entry.AudioSourceIds)
            {
                participantsBySource[sourceId] = entry.Participant;
            }
        }
    }
}
