<!-- Developer tooling that produces the ONNX models for the local diarization adapter (#44). -->

# Diarization model export

`export_models.py` writes the two models the `diarization` Cargo feature loads, and checks them
with onnxruntime before it finishes:

| File | Model | Licence | Input | Output |
| --- | --- | --- | --- | --- |
| `segmentation.onnx` | pyannote segmentation-3.0 | MIT | waveform `[1, 1, 160000]` | powerset scores `[1, frames, 7]` |
| `embedding.onnx` | SpeechBrain ECAPA-TDNN (`spkrec-ecapa-voxceleb`) | Apache-2.0 | waveform `[1, samples]` | embedding `[1, 192]` |

The filterbank and mean normalization are inside the embedding graph, so the Rust core feeds it
plain samples. ONNX export has no complex tensors, so the script replaces SpeechBrain's STFT with an
equivalent real-valued convolution, after checking the two agree.

pyannote's weights are gated. With `HF_TOKEN` set (after accepting the model's conditions) or
`--segmentation-checkpoint`, the script exports them itself; otherwise it downloads the public
MIT-licensed export `onnx-community/pyannote-segmentation-3.0`, which has the same layout.

```sh
python -m venv .venv && . .venv/bin/activate
pip install torch torchaudio speechbrain pyannote.audio onnx onnxruntime huggingface_hub
python tools/diarization/export_models.py --output-dir models/diarization
```

Then run with:

```sh
cargo build --features local-ai,vad,diarization
export NOTE_TAKER_ENABLE_DIARIZATION=true
export NOTE_TAKER_DIARIZATION_SEGMENTATION_MODEL=models/diarization/segmentation.onnx
export NOTE_TAKER_DIARIZATION_EMBEDDING_MODEL=models/diarization/embedding.onnx
```

The models are not committed; `tools/cross/fetch.sh` exports them for the app builds. Tested with torch 2.15, speechbrain 1.1.2,
pyannote.audio 4.0.7, and onnxruntime 1.31.

## In the apps

The Linux and Windows apps and the server build include the `diarization` feature. The desktop
installers ship both models and their licences in `data/models/diarization/` beside the executable.
The apps turn diarization on by themselves, preferring models in
`<data directory>/models/diarization/`. The daemon still reads the variables above.

Windows loads Microsoft's `onnxruntime.dll` from beside the executable at run time, because `ort`
has no prebuilt library for `x86_64-pc-windows-gnu`. Windows 11 has an older copy in System32, which
`ort` refuses, so the DLL must be the one in the bundle. The Windows bundle also carries the Visual C++ runtime the
DLL needs. If the DLL cannot load, the core reports diarization as unavailable.

Android does not include the feature yet (#60): the library links, and the app can install the
models from its APK, but it has not been run on a device.

`licenses/` holds the licence texts shipped with the models.
