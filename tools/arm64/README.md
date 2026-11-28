<!-- This directory holds the scripts that build Koalino's 64-bit ARM packages for Linux and Windows. -->

# ARM64 builds (Linux and Windows)

Koalino's x86_64 desktop releases are built on this machine (`tools/cross/`). The ARM64 ones are
built natively on GitHub's hosted ARM runners by `.github/workflows/arm64-build.yml`, as the macOS
package is (#162):

- Flutter's Linux runner links GTK, GStreamer and their dependencies, which a cross build would
  need as a whole ARM64 sysroot.
- Flutter builds a Windows app only for the machine it runs on.

The workflow runs on a `v*` tag, attaching the packages to that release, or by hand
(**Actions → ARM64 build → Run workflow**), leaving them as the run's artifacts.

| File | What it does |
| --- | --- |
| `build-linux-arm64.sh` | On ARM64 Linux (`ubuntu-24.04-arm`): builds the core (whisper.cpp with Vulkan, microphone and computer sound, speech-pause chunking, diarization), the MCP server and the browser extension's host, then the Flutter app, and packs `dist/koalino-linux-arm64-VERSION.tar.gz` (#175) |
| `build-windows-arm64.sh` | On Windows on ARM (`windows-11-arm`), in Git Bash with the Visual Studio ARM64 environment: builds the core for `aarch64-pc-windows-msvc` (whisper.cpp through clang-cl, diarization with Microsoft's ARM64 `onnxruntime.dll` loaded at run time), the MCP server, the host and the app, and packs `dist/koalino-windows-arm64-VERSION.zip` (#176) |
| `fetch-models.sh` | Downloads Whisper base and the diarization models, checked against pinned hashes |
| `check-transcription.sh` | Transcribes whisper.cpp's `jfk.wav` sample with the build's CLI and fails unless the transcript holds the known sentence |

The packaging scripts in `tools/packaging/` read the architecture from the archive's name:

- `build-deb.sh` and `build-rpm.sh` turn the Linux tarball into an `arm64` `.deb` and an
  `aarch64` `.rpm`.
- `build-flatpak.sh` turns it into `koalino-arm64-VERSION.flatpak` (#190), on the ARM runner
  itself, since flatpak-builder builds for the machine it runs on. The workflow installs the
  bundle and checks that the app starts. `arm64-flatpak-for-release.yml` adds it to a release
  published before this, from that release's own tarball.
- `build-installer.sh` turns the Windows zip into `koalino-windows-arm64-VERSION-setup.exe`.

The ARM64 installer refuses x64 Windows. The x64 installer warns on Windows on ARM before
installing there under emulation.

## What differs from x86_64

- **CPU level:** the baseline ARMv8-A instruction set (`GGML_NATIVE=OFF`), so the Linux build also
  runs on a Raspberry Pi 4.
- **Linux libraries:** built on Ubuntu 24.04 (glibc 2.39). The script fails if a file needs more
  than Debian 13's glibc 2.41 or `GLIBCXX_3.4.33`, so it runs on Debian 13, Ubuntu 24.04 and newer.
- **Windows GPU:** no Vulkan on Windows on ARM yet. Transcription uses the CPU.
- **ONNX Runtime for Windows:** comes from Microsoft's NuGet package `Microsoft.ML.OnnxRuntime`,
  pinned by SHA-256 in the workflow. That package carries the `win-arm64` build. Its x64 DLL is
  byte-identical to the GitHub release's that `tools/cross/fetch.sh` fetches.
- **Checks:** on the ARM runner, the workflow transcribes a sample with the build's CLI and
  checks the text. It also starts each app (Linux under Xvfb) and fails if it does not stay up
  for a minute. This project has no ARM64 Windows machine of its own, so real-device
  testing of that build is still open.
