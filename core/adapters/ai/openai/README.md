<!-- Documents secure setup and constraints for the OpenAI Whisper transcription adapter. -->

# OpenAI Whisper adapter

Set `OPENAI_API_KEY` only in the process environment or a secure launcher/keychain bridge. The
adapter never logs, serializes, or returns this value. `NOTE_TAKER_OPENAI_WHISPER_MODEL` defaults
to `whisper-1`; `NOTE_TAKER_OPENAI_LANGUAGE` may be `it` or `en`.

The adapter uploads adapter-owned WAV bytes to `/v1/audio/transcriptions` as `audio_chunk.wav`
with `audio/wav` content type. Requests over 25 MiB fail with `PayloadTooLarge` before HTTP. Raw
PCM must be wrapped into a valid WAV file by `AudioFileSource`; neither raw audio nor API tokens
cross the application boundary. HTTP 429 maps to a bounded, portable rate-limit error so a caller
can choose a configured fallback instead of blocking indefinitely.
