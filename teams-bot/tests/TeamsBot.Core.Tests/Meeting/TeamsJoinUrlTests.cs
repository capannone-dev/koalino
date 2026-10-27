// Specifies how Teams "Join" links are parsed into Graph join parameters, and that links to
// anything other than a Teams meeting are refused before any Graph call is made.

using TeamsBot.Core.Meeting;
using Xunit;

namespace TeamsBot.Core.Tests.Meeting;

/// <summary>Tests for <see cref="TeamsJoinUrl"/>.</summary>
public sealed class TeamsJoinUrlTests
{
    /// <summary>A classic meetup-join link yields the thread, message, tenant, and organizer.</summary>
    [Fact]
    public void ParsesAClassicMeetupJoinLink()
    {
        var url = new Uri("https://teams.microsoft.com/l/meetup-join/19%3ameeting_NjA2YTE5%40thread.v2/0?context=%7b%22Tid%22%3a%2272f988bf-86f1-41af-91ab-2d7cd011db47%22%2c%22Oid%22%3a%22a1b2c3d4-0000-4000-8000-000000000001%22%7d");

        Assert.True(TeamsJoinUrl.TryParse(url, out var parsed, out var error), error);

        var meeting = Assert.IsType<TeamsJoinUrl.ThreadMeeting>(parsed);
        Assert.Equal("19:meeting_NjA2YTE5@thread.v2", meeting.ThreadId);
        Assert.Equal("0", meeting.MessageId);
        Assert.Equal("72f988bf-86f1-41af-91ab-2d7cd011db47", meeting.TenantId);
        Assert.Equal("a1b2c3d4-0000-4000-8000-000000000001", meeting.OrganizerId);
    }

    /// <summary>A short meeting-ID link yields the meeting ID and passcode.</summary>
    [Fact]
    public void ParsesAShortMeetingIdLink()
    {
        Assert.True(TeamsJoinUrl.TryParse(new Uri("https://teams.microsoft.com/meet/2345678901234?p=AbCd3fGh"), out var parsed, out var error), error);

        var meeting = Assert.IsType<TeamsJoinUrl.MeetingIdMeeting>(parsed);
        Assert.Equal("2345678901234", meeting.JoinMeetingId);
        Assert.Equal("AbCd3fGh", meeting.Passcode);
    }

    /// <summary>Links that are not Teams meetings, or lack the tenant, are refused with a reason.</summary>
    [Theory]
    [InlineData("http://teams.microsoft.com/l/meetup-join/19%3ameeting%40thread.v2/0?context=%7b%22Tid%22%3a%22t%22%7d", "https")]
    [InlineData("https://teams.microsoft.com.evil.example/l/meetup-join/19%3ameeting%40thread.v2/0?context=%7b%22Tid%22%3a%22t%22%7d", "host")]
    [InlineData("https://teams.microsoft.com/l/chat/19%3ameeting%40thread.v2/0", "meeting")]
    [InlineData("https://teams.microsoft.com/l/meetup-join/19%3ameeting%40thread.v2/0", "tenant")]
    [InlineData("https://teams.microsoft.com/l/meetup-join/19%3ameeting%40thread.v2/0?context=not-json", "context")]
    [InlineData("https://teams.microsoft.com/meet/12ab", "meeting ID")]
    public void RejectsOtherLinks(string link, string reasonFragment)
    {
        Assert.False(TeamsJoinUrl.TryParse(new Uri(link), out var parsed, out var error));

        Assert.Null(parsed);
        Assert.Contains(reasonFragment, error, StringComparison.OrdinalIgnoreCase);
    }
}
