# whisper-rs-sys, vendored

`whisper-rs-sys` 0.15.0 (whisper.cpp 1.8.3), MIT licensed, used through `[patch.crates-io]` in the root
`Cargo.toml`. It builds whisper.cpp for the core's `local-ai` feature. The folders below this one are
upstream's and keep upstream's layout, so they don't carry a README of their own.

The first commit that adds this folder is an unmodified copy, so `git log -p third_party/whisper-rs-sys`
shows everything we changed.

## What we changed, and why (`DECISION_LOG.md`, #75)

| File | Change | Why |
| --- | --- | --- |
| `Cargo.toml`, `build.rs` | An `opencl` feature, active only for Android, using `OPENCL_HEADERS_DIR` | Builds ggml's OpenCL backend without linking the library; `opencl_loader_shim.rs` opens it at run time |
| `build.rs` | Takes a fresh copy of the sources on each run, and watches `ggml-opencl` | Upstream copies them once, so our source edits were silently never built |
| `ggml-opencl.cpp` | Decoder matrix products with one output row go to the single-row kernel | They went to tile kernels, taking `medium` from 172 ms to 61 ms a step |
| `ggml-opencl.cpp` | Adreno matrix kernels honour tensor view offsets | They read the wrong part of a buffer |
| `ggml-opencl.cpp` | No kernel compilation while listing devices | It took about 9 s, even for a model running on the CPU |
| `flash_attn_f32_f16.cl` | Guards against `0 × NaN` and overflow in `exp` | Flash Attention produced wrong text on Adreno |

To update whisper.cpp: replace this folder with the new crate, commit that alone, and reapply the table.
