<!-- This directory declares interfaces implemented by infrastructure adapters and consumed by application use cases. -->

# Ports layer

Ports define the stable contracts for audio capture, AI processing, P2P networking, and persistence. They prevent domain and application code from depending on OS audio APIs, libp2p, databases, or AI SDKs.

The P2P port exposes `NetworkEvent` values for discovery, expiry, and future opaque payload
delivery. Its nonblocking `poll_event` operation lets application tests inject a mock network
without starting libp2p or opening sockets.

`broadcast_encrypted` provides application-level fan-out of opaque protocol envelopes only through
authenticated encrypted peers. It never gives application services access to libp2p topics,
connections, or socket handles.

`VoiceVectorDB` accepts and returns `VoiceEmbedding` values only. Its adapter owns all vector
similarity calculations and biometric persistence details, while the application receives just
candidate scores through the port.
