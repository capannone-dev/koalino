<!-- Documents repository-hosted automation configuration. -->

# GitHub automation

Workflow definitions live in `workflows/`. They provide cloud verification for runtimes that are
not available in the local coding sandbox.

`macos-build.yml` (#162) builds and packages the macOS app on a hosted macOS runner, on `v*` tags
(attaching the zip to that release) and on demand (Actions → macOS build → Run workflow; the zip
is the run's artifact). It downloads Whisper base from whisper.cpp's source and the diarization
models from the `models-v1` release, each checked against a pinned checksum, then runs
`tools/macos/build-macos.sh`. The package is ad hoc signed and untested on real Macs (#151).

`arm64-build.yml` (#175, #176) builds Koalino natively on GitHub's hosted ARM runners. It runs on
`v*` tags, attaching the packages to that release, and on demand (Actions → ARM64 build → Run
workflow), leaving them as the run's artifacts.

- **Linux** (`ubuntu-24.04-arm`): a tarball, an `arm64` `.deb` and an `aarch64` `.rpm`.
- **Windows on ARM** (`windows-11-arm`): a zip, then an installer, which a Linux job builds with
  NSIS.

Each job transcribes a sample on the ARM runner with the build's CLI and checks the text. It
also starts the built app and fails if it does not stay up for a minute. The scripts are in
`tools/arm64/`.

`pages.yml` (#166) publishes `site/` on GitHub Pages when it changes on `main`. It is skipped while
the repository is private, since the free plan has no Pages for private repositories.
