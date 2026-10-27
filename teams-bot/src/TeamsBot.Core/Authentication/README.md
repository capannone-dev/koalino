<!-- Explains Graph token handling in the Teams bot core. -->

# Authentication

- `CachingAccessTokenProvider`: caches the Graph application token.
  - It refreshes the token 5 minutes before expiry.
  - It shares a single acquisition among concurrent callers.
  - It retries failed acquisitions with backoff.
  - It never logs token values.
- `UnauthorizedRetryHandler`: an `HttpClient` handler in front of the Graph SDK. On a `401` it
  invalidates the token and re-sends the request once with a fresh token.

The MSAL client-credentials call itself lives in `TeamsBot.Media/MsalTokenSource.cs`.
