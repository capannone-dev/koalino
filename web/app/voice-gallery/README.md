<!-- Documents the interactive voice-gallery dashboard route. -->

# Voice gallery route

This server route lists meetings, fetches each meeting's unresolved unknown-speaker clusters, and
passes them to the `VoiceGallery` client component, which assigns clusters to typed names. Cluster
DTOs carry no raw audio or embedding data.
