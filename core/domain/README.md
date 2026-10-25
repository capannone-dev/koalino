<!-- This directory contains pure business concepts with no dependencies on infrastructure or external services. -->

# Domain layer

Domain types encode meetings, recordings, transcripts, device identities, voice identities, and domain-specific failures. It must never import ports, application services, or adapters.

Voice-signature values retain only UUID metadata and model-derived floating-point embeddings. They
never retain raw audio; persistence and similarity calculation remain outside this layer.

Attachment values retain only a user-selected local path and declared metadata. Metadata extraction,
file access, and attachment persistence remain behind ports; meeting suggestions are immutable
domain values based on portable UTC meeting windows.
