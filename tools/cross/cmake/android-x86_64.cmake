# CMake toolchain for whisper.cpp on x86_64-linux-android, the Android emulator on a PC (not a
# shipped target): the NDK toolchain with the ABI and API level set here, because whisper-rs-sys
# forwards only WHISPER_*, GGML_* and CMAKE_* variables. The CPU level is the x86-64 baseline the
# emulator guarantees; whisper.cpp still picks faster paths at run time where it can.
set(ANDROID_ABI x86_64)
set(ANDROID_PLATFORM android-29)
include($ENV{ANDROID_NDK}/build/cmake/android.toolchain.cmake)
