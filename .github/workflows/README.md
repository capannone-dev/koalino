<!-- Documents repository-hosted automation configuration. -->

# GitHub automation

Workflow definitions live in `workflows/`. They provide cloud verification for runtimes that are
not available in the local coding sandbox.

`macos-build.yml` (#162) builds and packages the macOS app on a hosted macOS runner, on `v*` tags
(attaching the zip to that release) and on demand (Actions → macOS build → Run workflow; the zip
is the run's artifact). It downloads Whisper base from whisper.cpp's source and the diarization
models from the `models-v1` release, each checked against a pinned checksum, then runs
`tools/macos/build-macos.sh`. The package is ad hoc signed and untested on real Macs (#151).
