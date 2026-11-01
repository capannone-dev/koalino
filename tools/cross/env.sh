# Source this before a cross build (see README.md); it sets up the toolchains from fetch.sh.
T=${NOTE_TAKER_TOOLCHAINS:-$HOME/toolchains}
CROSS_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# A separate target directory keeps the host's target/ (built by the system rustc) untouched.
export RUSTUP_HOME=$T/rustup CARGO_HOME=$T/cargo CARGO_TARGET_DIR=$T/target
export PATH=$T/cargo/bin:$T/zig-x86_64-linux-0.14.1:$T/cmake-venv/bin:$PATH
export CMAKE_TOOLCHAIN_FILE_x86_64_pc_windows_gnu=$CROSS_DIR/cmake/windows-gnu-zig.cmake
export ANDROID_NDK=$T/android-ndk-r27c ANDROID_NDK_HOME=$T/android-ndk-r27c
NDK_BIN=$ANDROID_NDK/toolchains/llvm/prebuilt/linux-x86_64/bin
export CC_aarch64_linux_android=$NDK_BIN/aarch64-linux-android29-clang
export AR_aarch64_linux_android=$NDK_BIN/llvm-ar
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=$NDK_BIN/aarch64-linux-android29-clang
export CMAKE_TOOLCHAIN_FILE_aarch64_linux_android=$CROSS_DIR/cmake/android-arm64.cmake
export BINDGEN_EXTRA_CLANG_ARGS_aarch64_linux_android="--sysroot=$NDK_BIN/../sysroot"
# Flutter app builds (fetch-flutter.sh): Flutter SDK, JDK and Android SDK.
export JAVA_HOME=$T/jdk ANDROID_HOME=$T/android-sdk ANDROID_SDK_ROOT=$T/android-sdk
export PATH=$T/flutter/bin:$JAVA_HOME/bin:$ANDROID_HOME/platform-tools:$PATH
# Vulkan headers and an empty link library, for the GPU builds (fetch-vulkan.sh; Issue #71). The
# core defines the one Vulkan symbol ggml links, so nothing is really linked (vulkan_loader_shim.rs).
export VULKAN_SDK=$T/vulkan-sdk
# OpenCL headers for the Android build's efficient mode (fetch-opencl.sh; Issue #75). The library
# itself comes from the phone at run time (opencl_loader_shim.rs).
export OPENCL_HEADERS_DIR=$T/opencl-headers
