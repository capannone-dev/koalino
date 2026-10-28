# CMake toolchain for whisper.cpp on aarch64-linux-android: the NDK toolchain with the ABI and API
# level set here, because whisper-rs-sys forwards only WHISPER_*, GGML_* and CMAKE_* variables.
set(ANDROID_ABI arm64-v8a)
set(ANDROID_PLATFORM android-29)
include($ENV{ANDROID_NDK}/build/cmake/android.toolchain.cmake)
