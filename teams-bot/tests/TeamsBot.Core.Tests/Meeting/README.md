<!-- Explains the meeting lifecycle tests. -->

# Meeting tests

`BotServiceTests` mocks `ICallJoiner` and `ICallSession` with NSubstitute. It specifies the
recording notice before metadata, join retries, roster tagging, screen-share routing, resilient
meeting ends, and failure isolation. `TeamsJoinUrlTests` specifies which join links are
accepted.
