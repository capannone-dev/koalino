<!-- Documents typed browser/server API clients for the local Axum daemon. -->

# API client

This directory centralizes typed calls to the local Axum server. Server pages use cached-disabled
fetch calls for current local state; client components reuse the same contract for assignments and
WebSocket URL construction.
