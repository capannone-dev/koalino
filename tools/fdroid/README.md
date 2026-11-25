<!-- This directory holds what Koalino's F-Droid build needs: the recipe draft and the scripts it runs. -->

# F-Droid build

F-Droid builds every app from source on its own servers and signs it with its own key (#163). The
regular Android build cannot pass that as it is, so the F-Droid build is a variant of it:

| Left out | Why | In the F-Droid build |
| --- | --- | --- |
| Speaker separation (`diarization`) | needs Microsoft's ONNX Runtime, which comes prebuilt | turns stay unattributed; voices can still be named by hand |
| Whisper on the NPU (`npu-whisper`) | needs ONNX Runtime and Qualcomm's proprietary QNN libraries | the option does not appear |
| Bundled Whisper `base` and diarization models | built or exported outside the source tree | Settings → Transcription downloads Whisper models from whisper.cpp's source, checked by hash |
| The PDF view (`pdfrx`) | its build downloads a prebuilt PDFium library | PDFs say they cannot be shown in the app yet, like Office files |

Whisper (whisper.cpp, built from the vendored source in `third_party/`), efficient mode (OpenCL
headers only; the phone's own OpenCL library is opened at run time), speech-pause chunking,
sync, backups and everything else stay.

| File | What it does |
| --- | --- |
| `prepare.sh` | Run before `flutter pub get`: swaps in `pdf_view.dart`, drops `pdfrx`, and leaves the release APK unsigned |
| `pdf_view.dart` | The PDF view without PDFium |
| `build-core.sh` | Builds the Rust core from source with F-Droid's NDK into `jniLibs` (`arm64`; `x86_64` to test on the emulator) |
| `io.github.capannone_dev.koalino.yml` | Draft recipe for [fdroiddata](https://gitlab.com/fdroid/fdroiddata) |

The listing (title, descriptions, icon) is in `fastlane/metadata/android/en-US/`.

## Submitting

1. The repository must be public (#161) and the release tag the recipe names must exist.
2. With a GitLab account, fork fdroiddata, add the recipe as `metadata/io.github.capannone_dev.koalino.yml`,
   check it with `fdroid lint` and `fdroid build io.github.capannone_dev.koalino` (fdroidserver's
   Docker image), and open a merge request.
3. F-Droid signs with its own key, so its APK cannot update one from GitHub releases (or the
   reverse): moving between them needs a backup and restore (#167).

## Testing locally

On a throwaway copy of the repository: `tools/fdroid/prepare.sh`, then
`ANDROID_NDK=… tools/fdroid/build-core.sh x86_64` and
`flutter build apk --release --target-platform android-x64 --dart-define=KOALINO_NO_BUNDLED_MODELS=true`
in `ui/`; sign the APK with a debug key to install it on the emulator.
