<!-- Developer tooling that is not part of any shipped component. -->

# Tools

- `arm64/`: builds the 64-bit ARM Linux and Windows packages on GitHub's hosted ARM runners
  (`.github/workflows/arm64-build.yml`).
- `cross/`: installs and configures the toolchains for building the daemon and CLI for Windows
  and Android from a Linux host, including whisper.cpp (`local-ai`).
- `diarization/`: exports the ONNX models the `diarization` feature loads.
- `licenses/`: generates `THIRD_PARTY_NOTICES.md`, the licenses of everything Koalino builds on.
  `diarization/test_scripts/` holds Italian reading scripts for recording test audio.
