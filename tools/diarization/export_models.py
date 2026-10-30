#!/usr/bin/env python3
"""Exports the two ONNX models the local diarization adapter loads (Issue #44).

* ``segmentation.onnx``: pyannote segmentation-3.0 (MIT). Input: float32 waveform
  ``[1, 1, 160000]`` (10 s at 16 kHz). Output: ``[1, frames, 7]`` powerset scores in pyannote's
  ``Powerset(3, 2)`` order. The weights are gated on Hugging Face: with ``HF_TOKEN`` set (after
  accepting the model's conditions) or ``--segmentation-checkpoint``, the script exports them
  itself; otherwise it downloads the public MIT-licensed export ``onnx-community/
  pyannote-segmentation-3.0``, which has the same layout.
* ``embedding.onnx``: SpeechBrain ECAPA-TDNN ``speechbrain/spkrec-ecapa-voxceleb`` (Apache-2.0)
  with the 80-bin filterbank and the sentence mean normalization inside the graph. Input: float32
  waveform ``[1, samples]`` at 16 kHz. Output: ``[1, 192]`` speaker embedding.

Both models are checked with onnxruntime before the script finishes: the segmentation output
shape, and the embedding's cosine similarity to SpeechBrain's own ``encode_batch`` on test audio.

Usage (in a virtual environment with torch, torchaudio, speechbrain, pyannote.audio, onnx,
onnxruntime and huggingface_hub)::

    python tools/diarization/export_models.py --output-dir models/diarization

Point the daemon at the results with ``NOTE_TAKER_DIARIZATION_SEGMENTATION_MODEL`` and
``NOTE_TAKER_DIARIZATION_EMBEDDING_MODEL``. Nothing here is shipped with the app, and no audio or
embedding is written anywhere but the temporary checks.
"""

import argparse
import os
import shutil
import sys
from pathlib import Path

import numpy as np
import onnxruntime
import torch

SAMPLE_RATE_HZ = 16_000
WINDOW_SAMPLES = 10 * SAMPLE_RATE_HZ
POWERSET_CLASSES = 7
EMBEDDING_DIMENSIONS = 192
# Least cosine similarity between the ONNX embedding and SpeechBrain's own.
EMBEDDING_PARITY = 0.999
PUBLIC_SEGMENTATION_REPO = "onnx-community/pyannote-segmentation-3.0"
GATED_SEGMENTATION_REPO = "pyannote/segmentation-3.0"
EMBEDDING_REPO = "speechbrain/spkrec-ecapa-voxceleb"
ONNX_OPSET = 18


def parse_arguments() -> argparse.Namespace:
    """Reads the command line: the output directory and an optional local checkpoint."""
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument(
        "--segmentation-checkpoint",
        type=Path,
        help="a local pyannote segmentation-3.0 checkpoint (pytorch_model.bin)",
    )
    return parser.parse_args()


def export_segmentation(output_path: Path, checkpoint: Path | None) -> None:
    """Writes segmentation.onnx from pyannote's weights, or fetches the public export.

    Side effects: downloads from Hugging Face; writes ``output_path``.
    """
    token = os.environ.get("HF_TOKEN")
    if checkpoint is None and not token:
        from huggingface_hub import hf_hub_download

        print(f"no HF_TOKEN or checkpoint: using {PUBLIC_SEGMENTATION_REPO}")
        downloaded = hf_hub_download(PUBLIC_SEGMENTATION_REPO, "onnx/model.onnx")
        shutil.copyfile(downloaded, output_path)
        return

    from pyannote.audio import Model

    source = str(checkpoint) if checkpoint is not None else GATED_SEGMENTATION_REPO
    model = Model.from_pretrained(source, token=token)
    model.eval()
    torch.onnx.export(
        model,
        (torch.zeros(1, 1, WINDOW_SAMPLES),),
        str(output_path),
        input_names=["input_values"],
        output_names=["logits"],
        opset_version=ONNX_OPSET,
        dynamo=False,
    )


class RealValuedStft(torch.nn.Module):
    """SpeechBrain's STFT as a convolution with a windowed DFT basis.

    ONNX export does not support complex tensors, so the transform is computed as real and
    imaginary convolutions. The output layout matches ``speechbrain.processing.features.STFT``:
    ``[batch, frames, n_fft // 2 + 1, 2]``.
    """

    def __init__(self, stft) -> None:
        super().__init__()
        if stft.win_length != stft.n_fft or not stft.center or stft.normalized_stft:
            raise ValueError("only SpeechBrain's default ECAPA STFT is supported")
        self.n_fft = stft.n_fft
        self.hop_length = stft.hop_length
        bins = self.n_fft // 2 + 1
        sample_index = torch.arange(self.n_fft, dtype=torch.float64)
        angles = 2 * torch.pi * torch.arange(bins, dtype=torch.float64)[:, None] * sample_index / self.n_fft
        window = stft.window.to(torch.float64)
        basis = torch.cat([torch.cos(angles) * window, -torch.sin(angles) * window])
        self.register_buffer("basis", basis.to(torch.float32).unsqueeze(1))

    def forward(self, waveform: torch.Tensor) -> torch.Tensor:
        """Transforms ``[batch, samples]`` into ``[batch, frames, bins, 2]``."""
        padded = torch.nn.functional.pad(waveform, (self.n_fft // 2, self.n_fft // 2))
        spectrum = torch.nn.functional.conv1d(padded.unsqueeze(1), self.basis, stride=self.hop_length)
        batch, channels, frames = spectrum.shape
        return spectrum.reshape(batch, 2, channels // 2, frames).permute(0, 3, 2, 1)


def replace_stft(compute_features) -> None:
    """Swaps the filterbank's complex STFT for ``RealValuedStft`` after checking they agree."""
    real_valued = RealValuedStft(compute_features.compute_STFT)
    probe = torch.randn(2, SAMPLE_RATE_HZ)
    with torch.no_grad():
        difference = (real_valued(probe) - compute_features.compute_STFT(probe)).abs().max().item()
    if difference > 1e-3:
        sys.exit(f"the real-valued STFT differs from SpeechBrain's by {difference}")
    compute_features.compute_STFT = real_valued


class EcapaFromWaveform(torch.nn.Module):
    """ECAPA with its feature extraction: waveform in, embedding out."""

    def __init__(self, classifier) -> None:
        super().__init__()
        self.compute_features = classifier.mods.compute_features
        replace_stft(self.compute_features)
        self.embedding_model = classifier.mods.embedding_model

    def forward(self, waveform: torch.Tensor) -> torch.Tensor:
        """Embeds ``[1, samples]`` at 16 kHz as ``[1, 192]``."""
        features = self.compute_features(waveform)
        # SpeechBrain's InputNormalization(norm_type="sentence", std_norm=False) for one sentence.
        features = features - features.mean(dim=1, keepdim=True)
        return self.embedding_model(features).squeeze(1)


def load_classifier(cache_dir: Path):
    """Downloads and loads SpeechBrain's ECAPA classifier into ``cache_dir``."""
    from speechbrain.inference.speaker import EncoderClassifier

    return EncoderClassifier.from_hparams(
        source=EMBEDDING_REPO, savedir=str(cache_dir), run_opts={"device": "cpu"}
    )


def export_embedding(output_path: Path, classifier) -> None:
    """Writes embedding.onnx with the filterbank inside the graph.

    Side effects: writes ``output_path``.
    """
    module = EcapaFromWaveform(classifier).eval()
    with torch.no_grad():
        torch.onnx.export(
            module,
            (torch.randn(1, 3 * SAMPLE_RATE_HZ) * 0.1,),
            str(output_path),
            input_names=["waveform"],
            output_names=["embedding"],
            dynamic_axes={"waveform": {1: "samples"}},
            opset_version=ONNX_OPSET,
            dynamo=False,
        )


def test_audio(seconds: float, seed: int) -> np.ndarray:
    """A voice-like test signal: a pitched harmonic tone with noise, in [-1, 1]."""
    generator = np.random.default_rng(seed)
    time = np.arange(int(seconds * SAMPLE_RATE_HZ)) / SAMPLE_RATE_HZ
    pitch = 110 + 60 * seed
    signal = sum(np.sin(2 * np.pi * pitch * harmonic * time) / harmonic for harmonic in range(1, 8))
    signal = signal * (0.5 + 0.5 * np.sin(2 * np.pi * 3 * time))
    signal = signal + 0.02 * generator.standard_normal(time.size)
    return (0.3 * signal / np.abs(signal).max()).astype(np.float32)


def check_segmentation(model_path: Path) -> None:
    """Fails unless the segmentation model maps one window to ``[1, frames, 7]``."""
    session = onnxruntime.InferenceSession(str(model_path))
    window = test_audio(10.0, seed=0).reshape(1, 1, WINDOW_SAMPLES)
    (scores,) = session.run(None, {session.get_inputs()[0].name: window})
    if scores.ndim != 3 or scores.shape[0] != 1 or scores.shape[2] != POWERSET_CLASSES:
        sys.exit(f"segmentation output has shape {scores.shape}, expected [1, frames, 7]")
    print(f"segmentation ok: {scores.shape[1]} frames per window")


def check_embedding(model_path: Path, classifier) -> None:
    """Fails unless the ONNX embedding matches SpeechBrain's on test audio of several lengths."""
    session = onnxruntime.InferenceSession(str(model_path))
    for seconds, seed in ((0.5, 1), (3.0, 2), (14.0, 3)):
        audio = test_audio(seconds, seed)
        (onnx_embedding,) = session.run(None, {"waveform": audio.reshape(1, -1)})
        with torch.no_grad():
            reference = classifier.encode_batch(torch.from_numpy(audio).unsqueeze(0))
        reference = reference.reshape(-1).numpy()
        onnx_embedding = onnx_embedding.reshape(-1)
        if onnx_embedding.size != EMBEDDING_DIMENSIONS:
            sys.exit(f"embedding has {onnx_embedding.size} dimensions, expected 192")
        cosine = float(
            onnx_embedding @ reference
            / (np.linalg.norm(onnx_embedding) * np.linalg.norm(reference))
        )
        if cosine < EMBEDDING_PARITY:
            sys.exit(f"ONNX embedding diverges from SpeechBrain on {seconds} s: cosine {cosine}")
        print(f"embedding ok on {seconds} s: cosine {cosine:.6f}")


def main() -> None:
    """Exports and checks both models."""
    arguments = parse_arguments()
    arguments.output_dir.mkdir(parents=True, exist_ok=True)
    segmentation_path = arguments.output_dir / "segmentation.onnx"
    embedding_path = arguments.output_dir / "embedding.onnx"

    export_segmentation(segmentation_path, arguments.segmentation_checkpoint)
    check_segmentation(segmentation_path)

    classifier = load_classifier(arguments.output_dir / "speechbrain-ecapa")
    export_embedding(embedding_path, classifier)
    check_embedding(embedding_path, classifier)
    print(f"wrote {segmentation_path} and {embedding_path}")


if __name__ == "__main__":
    main()
