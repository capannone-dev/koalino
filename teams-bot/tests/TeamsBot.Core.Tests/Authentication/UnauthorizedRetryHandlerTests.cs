// Specifies the HTTP handler that recovers from an expired Graph token: on 401 it invalidates
// the cached token, re-authorizes, and resends the request once with the same body.

using System.Net;
using System.Net.Http.Headers;
using System.Text;
using NSubstitute;
using TeamsBot.Core.Authentication;
using Xunit;

namespace TeamsBot.Core.Tests.Authentication;

/// <summary>Tests for <see cref="UnauthorizedRetryHandler"/>.</summary>
public sealed class UnauthorizedRetryHandlerTests
{
    private readonly IAccessTokenProvider tokens = Substitute.For<IAccessTokenProvider>();

    /// <summary>Creates a client whose server answers with <paramref name="statuses"/> in turn and records each request.</summary>
    private HttpClient Client(List<(string? Authorization, string Body)> seen, params HttpStatusCode[] statuses)
    {
        var responses = new Queue<HttpStatusCode>(statuses);
        var server = new StubHandler(async request =>
        {
            var body = request.Content is null ? string.Empty : await request.Content.ReadAsStringAsync();
            seen.Add((request.Headers.Authorization?.ToString(), body));
            return new HttpResponseMessage(responses.Dequeue());
        });
#pragma warning disable CA2000 // The HttpClient owns and disposes the handler chain.
        return new HttpClient(new UnauthorizedRetryHandler(tokens) { InnerHandler = server });
#pragma warning restore CA2000
    }

    /// <summary>A 401 invalidates the token and resends the same body with a fresh one.</summary>
    [Fact]
    public async Task RetriesOnceWithAFreshToken()
    {
        tokens.GetTokenAsync(Arg.Any<CancellationToken>()).Returns("fresh");
        var seen = new List<(string? Authorization, string Body)>();
        using var client = Client(seen, HttpStatusCode.Unauthorized, HttpStatusCode.OK);
        using var request = new HttpRequestMessage(HttpMethod.Post, "https://graph.microsoft.com/v1.0/communications/calls")
        {
            Content = new StringContent("{\"x\":1}", Encoding.UTF8, "application/json"),
        };
        request.Headers.Authorization = new AuthenticationHeaderValue("Bearer", "expired");

        using var response = await client.SendAsync(request);

        Assert.Equal(HttpStatusCode.OK, response.StatusCode);
        tokens.Received(1).Invalidate();
        Assert.Equal(new (string?, string)[] { ("Bearer expired", "{\"x\":1}"), ("Bearer fresh", "{\"x\":1}") }, seen);
    }

    /// <summary>A second 401 is returned to the caller instead of looping.</summary>
    [Fact]
    public async Task ASecondUnauthorizedIsReturned()
    {
        tokens.GetTokenAsync(Arg.Any<CancellationToken>()).Returns("fresh");
        var seen = new List<(string? Authorization, string Body)>();
        using var client = Client(seen, HttpStatusCode.Unauthorized, HttpStatusCode.Unauthorized);

        using var response = await client.GetAsync(new Uri("https://graph.microsoft.com/v1.0/x"));

        Assert.Equal(HttpStatusCode.Unauthorized, response.StatusCode);
        Assert.Equal(2, seen.Count);
    }

    /// <summary>Other responses pass through untouched.</summary>
    [Fact]
    public async Task OtherResponsesPassThrough()
    {
        var seen = new List<(string? Authorization, string Body)>();
        using var client = Client(seen, HttpStatusCode.Forbidden);

        using var response = await client.GetAsync(new Uri("https://graph.microsoft.com/v1.0/x"));

        Assert.Equal(HttpStatusCode.Forbidden, response.StatusCode);
        Assert.Single(seen);
        tokens.DidNotReceive().Invalidate();
    }

    /// <summary>Answers requests with a delegate.</summary>
    private sealed class StubHandler(Func<HttpRequestMessage, Task<HttpResponseMessage>> respond) : HttpMessageHandler
    {
        protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken cancellationToken) => respond(request);
    }
}
