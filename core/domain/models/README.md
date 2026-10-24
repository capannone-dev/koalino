<!-- This directory defines typed domain entities used independently of databases, networks, or AI SDKs. -->

# Domain models

`models.rs` contains pure values for meetings, recordings, transcripts, and portable audio-capture data. `AudioConfig`, `AudioChunk`, and `AudioState` intentionally avoid operating-system stream types so local and P2P capture adapters can be exchanged without changing application consumers.

The models here are immutable value objects representing meeting recordings, generated transcripts, and device identity metadata.
