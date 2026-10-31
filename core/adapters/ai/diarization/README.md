<!-- Documents the local speaker diarization adapter (Issue #44). -->

# Local speaker diarization

This directory implements the `DiarizationEngine` port on the device, with no cloud service. For
each recorder chunk (a 16 kHz mono PCM16 WAV of at most ~15 s) it finds who speaks when and returns
`SpeakerSegment`s, each with the speaker's voice embedding. The application layer then matches the
embeddings against enrolled voices and clusters the unknown ones (`diarization_processor`).

| File | Purpose |
| --- | --- |
| `wav_input.rs` | Decodes the recorder's WAV; any other format is refused, never resampled. |
| `powerset.rs` | Decodes pyannote segmentation-3.0's seven powerset classes into up to three active local speakers per frame. |
| `windowed_engine.rs` | Runs the segmentation model over 10 s windows (5 s step), embeds each local speaker, links speakers across windows by cosine similarity, and assembles turns (drops blips under `min_turn_ms`, bridges pauses up to `max_merge_gap_ms`). |
| `onnx_models.rs` | `diarization` feature: runs the models exported by `tools/diarization/export_models.py` with ONNX Runtime. |
| `recordings_measurement_tests.rs` | `#[ignore]`d: runs the real models over local recordings for `tools/diarization/evaluate_pipeline.py`. |
| `*_tests.rs` | Unit tests with fake models; no weights are loaded, except one `#[ignore]`d test that takes exported models from the environment. |

The models sit behind the `SegmentationModel` and `SpeakerEmbeddingModel` traits. Inference runs on
its own thread, never on the async runtime (the pipeline worker has no Tokio runtime, so
`spawn_blocking` is not an option). Audio and embeddings (biometric
PII) are never logged; embeddings are only stored encrypted by the storage adapter.
