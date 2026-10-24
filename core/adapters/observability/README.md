<!-- This directory provides the tracing-backed observability adapter for JSON console and rotating-file logs. -->

# Observability adapter

The adapter configures `tracing` JSON output to stdout and a daily rolling local file. Application events must include `device_id` and must never include raw audio, voice embeddings, encryption keys, or other sensitive values.
