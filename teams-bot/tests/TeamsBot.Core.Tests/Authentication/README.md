<!-- Explains the authentication tests. -->

# Authentication tests

These tests specify the Graph token cache (refresh margin, invalidation, retries, a single
shared acquisition) and the handler that recovers from a `401` by retrying once with a fresh
token.
