# CMake toolchain for whisper.cpp on aarch64-linux-android: the NDK toolchain with the ABI and API
# level set here, because whisper-rs-sys forwards only WHISPER_*, GGML_* and CMAKE_* variables.
set(ANDROID_ABI arm64-v8a)
set(ANDROID_PLATFORM android-29)
include($ENV{ANDROID_NDK}/build/cmake/android.toolchain.cmake)
# Cross builds cannot detect the CPU, which leaves whisper.cpp with baseline ARMv8 only. Every
# supported phone needs these, available since the 2018 Cortex-A55 and A75 (Snapdragon 845). The core
# checks for them at run time and turns local transcription off, instead of crashing, without them.
set(GGML_CPU_ARM_ARCH armv8.2-a+dotprod+fp16 CACHE STRING "" FORCE)
