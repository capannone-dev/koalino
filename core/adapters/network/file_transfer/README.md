<!-- Documents the dedicated attachment file-transfer protocol and its security constraints. -->

# File transfer protocol

This directory documents the adapter-private `/note-taker/file-transfer/1` libp2p behaviour. It is
separate from `/note-taker/sync-message/1`: CRDT messages never carry attachment bytes.

Every binary frame contains `hash`, `total_size`, `chunk_size`, `offset`, and `is_final` metadata.
The sender uses ordered 64 KiB chunks, including one explicit final frame for a zero-byte file. The
receiver rejects non-contiguous, short, oversized, duplicate, or out-of-order streams and commits
only after the assembled SHA-256 matches its content address. The current adapter test uses Noise
over `MemoryTransport`, never a TCP or UDP socket.
