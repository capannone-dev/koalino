<!-- CMake toolchain files that let whisper-rs-sys build whisper.cpp for the cross targets. -->

# CMake toolchain files

`whisper-rs-sys` builds whisper.cpp with CMake through the `cmake` crate. It forwards only
`WHISPER_*`, `GGML_*` and `CMAKE_*` variables. `../env.sh` selects these files through
`CMAKE_TOOLCHAIN_FILE_<target>`.

- `windows-gnu-zig.cmake` (`x86_64-pc-windows-gnu`):
  - **Compilers.** It uses cargo-zigbuild's compiler wrappers, because the `cmake` crate stops
    passing compilers once a toolchain file is set.
  - **SIMD.** It turns on AVX, AVX2, FMA and F16C, because a cross build cannot detect the CPU.
    Without them, transcription was about four times slower: 56.6 s against 13.3 s for an 11 s
    clip. Every CPU that Windows 11 supports has these extensions.
- `mingw-library-names.cmake` is included after ggml's `project()`. ggml removes the `lib` prefix
  from its static libraries on MinGW, producing `ggml.a`, but rustc's `-l static=ggml` looks for
  `libggml.a`. A deferred call gives the three ggml targets the prefix back.
- `android-arm64.cmake` (`aarch64-linux-android`) sets `ANDROID_ABI=arm64-v8a` and
  `ANDROID_PLATFORM=android-29`, then includes the NDK's toolchain. Without it, the NDK defaults
  to 32-bit ARM, and the link fails with "incompatible with aarch64linux".
