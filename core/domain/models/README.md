<!-- This directory defines typed domain entities used independently of databases, networks, or AI SDKs. -->

# Domain models

`models.rs` contains pure values for meetings, recordings, transcripts, portable audio-capture data, and non-sensitive compute manifests. `AudioConfig`, `AudioChunk`, and `AudioState` intentionally avoid operating-system stream types so local and P2P capture adapters can be exchanged without changing application consumers. `DeviceManifest` records only a device ID, abstract capacity, and advertised model names; a future encrypted P2P adapter can broadcast it without exposing raw audio, weights, credentials, or hardware serial numbers.

The models here are immutable value objects representing meeting recordings, generated transcripts, and device identity metadata.
