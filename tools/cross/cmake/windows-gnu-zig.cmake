# CMake toolchain for whisper.cpp under cargo-zigbuild (x86_64-pc-windows-gnu); set by ../env.sh.
set(CMAKE_SYSTEM_NAME Windows)
set(CMAKE_SYSTEM_PROCESSOR AMD64)
# With a toolchain file the cmake crate no longer passes compilers; use cargo-zigbuild's wrappers.
set(CMAKE_C_COMPILER $ENV{CC_x86_64_pc_windows_gnu})
set(CMAKE_CXX_COMPILER $ENV{CXX_x86_64_pc_windows_gnu})
set(CMAKE_AR $ENV{AR_x86_64_pc_windows_gnu})
set(CMAKE_RC_COMPILER $ENV{RC_x86_64_pc_windows_gnu})
# ggml removes the "lib" prefix on MinGW (ggml.a), but rustc's -l static=ggml looks for
# libggml.a. This runs after ggml's project() and renames its libraries back.
set(CMAKE_PROJECT_ggml_INCLUDE ${CMAKE_CURRENT_LIST_DIR}/mingw-library-names.cmake)
# Cross builds cannot detect the CPU (GGML_NATIVE is off), which leaves whisper.cpp without SIMD
# and several times slower. Every CPU that Windows 11 supports has AVX2, FMA and F16C.
set(GGML_AVX ON CACHE BOOL "" FORCE)
set(GGML_AVX2 ON CACHE BOOL "" FORCE)
set(GGML_FMA ON CACHE BOOL "" FORCE)
set(GGML_F16C ON CACHE BOOL "" FORCE)
