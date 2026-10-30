<!-- Documents the interactive voice-gallery dashboard route. -->

# Voice gallery route

This server route reads `GET /api/voice-gallery` and passes every meeting's unresolved clusters and
the saved people to the `VoiceGallery` client component, which assigns clusters to new or saved
names. These DTOs carry no raw audio or embedding data; a clip's audio is fetched only when the
owner plays it.
