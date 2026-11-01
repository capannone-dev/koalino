# Source this after env.sh for a Linux build that runs on Debian 13 and newer: an explicit
# x86_64-unknown-linux-gnu target, built by clang against the sysroot from fetch-debian-sysroot.sh.
# Build scripts still link and run on this machine, but this target is also the machine's own, so
# their C dependencies see these compilers too. That works because newer glibc accepts older headers.
export NOTE_TAKER_TOOLCHAINS_DIR=$T
DEBIAN_SYSROOT=$T/debian-13-sysroot
DEBIAN_TARGET=x86_64-unknown-linux-gnu
if [ ! -f "$DEBIAN_SYSROOT/usr/include/stdio.h" ]; then
  echo "missing $DEBIAN_SYSROOT: run tools/cross/fetch-debian-sysroot.sh" >&2
  return 1
fi
# Apart from other builds, whose artifacts would not link against this sysroot.
export CARGO_TARGET_DIR=$T/target-debian
DEBIAN_CLANG_FLAGS="--target=x86_64-linux-gnu --sysroot=$DEBIAN_SYSROOT"
export CC_x86_64_unknown_linux_gnu=clang CXX_x86_64_unknown_linux_gnu=clang++
export AR_x86_64_unknown_linux_gnu=llvm-ar
export CFLAGS_x86_64_unknown_linux_gnu=$DEBIAN_CLANG_FLAGS
export CXXFLAGS_x86_64_unknown_linux_gnu=$DEBIAN_CLANG_FLAGS
export BINDGEN_EXTRA_CLANG_ARGS_x86_64_unknown_linux_gnu=$DEBIAN_CLANG_FLAGS
export CMAKE_TOOLCHAIN_FILE_x86_64_unknown_linux_gnu=$CROSS_DIR/cmake/debian-13.cmake
export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=clang
export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS="-C link-arg=--target=x86_64-linux-gnu -C link-arg=--sysroot=$DEBIAN_SYSROOT -C link-arg=-fuse-ld=lld -L native=$VULKAN_SDK/lib"
export PKG_CONFIG_LIBDIR_x86_64_unknown_linux_gnu=$DEBIAN_SYSROOT/usr/lib/x86_64-linux-gnu/pkgconfig:$DEBIAN_SYSROOT/usr/share/pkgconfig
export PKG_CONFIG_SYSROOT_DIR_x86_64_unknown_linux_gnu=$DEBIAN_SYSROOT
# ONNX Runtime's download step is a build script needing this machine's OpenSSL; the app does not.
export OPENSSL_INCLUDE_DIR=${OPENSSL_INCLUDE_DIR:-/usr/include} OPENSSL_LIB_DIR=${OPENSSL_LIB_DIR:-/usr/lib}

# Fails when a built file needs a newer C or C++ library than Debian 13 has.
# Inputs: The files to check. Side effects: Prints the offending file.
check_debian_13_compatible() {
  local file newest
  for file in "$@"; do
    for limit in GLIBC_2.41 GLIBCXX_3.4.33; do
      newest=$(objdump -T "$file" | grep -o "${limit%_*}_[0-9.]*" | sort -Vu | tail -1 || true)
      if [ -n "$newest" ] && [ "$(printf '%s\n%s\n' "$newest" "$limit" | sort -V | tail -1)" != "$limit" ]; then
        echo "$file needs $newest, newer than Debian 13's $limit" >&2
        return 1
      fi
    done
  done
}
