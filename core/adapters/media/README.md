<!-- Documents local metadata adapters that satisfy attachment application ports. -->

# Attachment metadata adapters

This directory contains filesystem-facing implementations of `MetadataExtractor`. These adapters
are composed outside the application layer: `AttachmentManager` receives only the port and never
uses filesystem or file-format APIs directly.

`local_file_metadata.rs` currently returns the local file's modification time as its UTC capture
time. Platform EXIF/PDF-specific adapters can replace it later without changing attachment
suggestion policy or Flutter's JSON boundary.
