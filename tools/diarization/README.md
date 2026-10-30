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

The models are not committed or shipped with the app. Tested with torch 2.15, speechbrain 1.1.2,
pyannote.audio 4.0.7, and onnxruntime 1.31.

## In the apps

The Linux desktop and server builds include the `diarization` feature. The apps turn diarization on
by themselves when both files are in `<data directory>/models/diarization/`. The daemon still reads
the variables above. Windows and Android builds do not include the feature yet (#60): `ort` has no
prebuilt ONNX Runtime for `x86_64-pc-windows-gnu`, and the Android build links but has not been run
on a device.
