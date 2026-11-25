<!-- Builds and packages the macOS desktop app (#151). -->

# macOS build

`build-macos.sh` runs on a Mac with Xcode. It builds the Rust core for Intel and Apple silicon,
joins each into a universal binary with `lipo`, builds `Koalino.app` with Flutter, then puts the
core (`Contents/Frameworks/libnote_taker_core.dylib`), the MCP server and the browser extension's
host (`Contents/MacOS/`), and the Whisper and diarization models with their licences
(`Contents/Resources/models/`) into it. It signs the bundle ad hoc and zips it to
`dist/koalino-macos-universal-<version>.zip`.

The core is built with `local-ai,microphone,vad`, plus `diarization` on Apple silicon only:
diarization runs on ONNX Runtime, whose last Intel Mac build (1.23.2) is older than the API the
core needs, so on Intel Macs recordings are transcribed but not split by speaker. Capturing the computer's own sound
(`computer-sound`, PulseAudio on Linux) is not available on macOS yet.

The app is not notarized, since that needs a paid Apple Developer ID. Gatekeeper therefore blocks
its first start until the owner allows it in System Settings → Privacy & Security, as the main
README explains.

The models come from `$NOTE_TAKER_MODELS` (default `~/toolchains/models`), laid out as
`tools/cross/fetch.sh` makes them; copy them from a Linux build machine when the Mac has none.

GitHub Actions runs this script on a hosted macOS runner (`.github/workflows/macos-build.yml`,
#162), so the package no longer needs a local Mac or VM.
