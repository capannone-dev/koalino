#!/usr/bin/env python3
"""Tests two diarization questions that evaluate_pipeline.py leaves open, on the same recordings.

`cut` writes WAV clips for the Rust `measure_diarization` test:
  turns: each script turn, cut at the reference times, to check who is really speaking.
  slide: 2 s windows every 1 s, to try voice embeddings from short windows.

`score` compares each turn's voice with the solo recordings, then clusters the 2 s windows over the
whole meeting and reports diarization error. Clusters under 8 windows join their nearest neighbour.

Usage, with OUT holding the evaluate_pipeline.py outputs and wav/ the recordings:
  python evaluate_short_windows.py cut OUT tools/diarization/test_scripts
  run measure_diarization on OUT/turns and OUT/slide, writing OUT/turns.jsonl and OUT/slide.jsonl
  python evaluate_short_windows.py score OUT tools/diarization/test_scripts

Clips and embeddings are personal data: keep OUT outside the repository and delete it afterwards.
Only counts and rates are printed.
"""

import collections
import sys
import wave
from pathlib import Path

import numpy as np
from scipy.cluster.hierarchy import fcluster, linkage

import evaluate_pipeline as ev

WINDOW_S, STEP_S = 2, 1
MIN_CLUSTER_WINDOWS = 8
THRESHOLDS = (0.15, 0.18, 0.2, 0.25, 0.3)


def load_all(out, scripts_folder):
    """Reads the scripts and the reference run. Returns (scripts, reference turns per recording)."""
    scripts = {p.stem: ev.read_script(p) for p in Path(scripts_folder).glob("*.txt")}
    runs = ev.load(Path(out) / "o" / f"{ev.REFERENCE}.jsonl")
    return scripts, {name: ev.reference_turns(run, scripts[name]) for name, run in runs.items()}


def cut(out, scripts_folder):
    """Writes the turn clips and the sliding clips under `out`. Side effects: creates WAV files."""
    _, reference = load_all(out, scripts_folder)
    out = Path(out)
    (out / "turns").mkdir(exist_ok=True)
    (out / "slide").mkdir(exist_ok=True)
    for name, turns in reference.items():
        with wave.open(str(out / "wav" / f"{name}.wav")) as source:
            parameters, rate = source.getparams(), source.getframerate()
            width = source.getsampwidth() * source.getnchannels()
            audio = source.readframes(source.getnframes())

        def write(path, start_ms, end_ms):
            with wave.open(str(path), "wb") as clip:
                clip.setparams(parameters)
                clip.writeframes(audio[int(start_ms * rate / 1000) * width:int(end_ms * rate / 1000) * width])

        for index, turn in enumerate(turns):
            write(out / "turns" / f"{name}__{index:03d}__{turn['speaker']}.wav", turn["start"], turn["end"])
        for start_ms in range(0, len(audio) // width * 1000 // rate, STEP_S * 1000):
            write(out / "slide" / f"{name}__{start_ms:07d}.wav", start_ms, start_ms + WINDOW_S * 1000)


def voice(record):
    """Returns a clip's speech-weighted unit embedding, or None if it holds no measured speech."""
    parts = [(np.asarray(s["embedding"]), s["speech_ms"]) for w in record["windows"] for s in w["speakers"]]
    return ev.unit(sum(v * ms for v, ms in parts)) if parts else None


def cluster(vectors, threshold):
    """Average-linkage clustering, then joins small clusters to the nearest. Returns one label per vector."""
    labels = fcluster(linkage(vectors, "average", metric="cosine"), 1 - threshold, "distance")
    while True:
        sizes = collections.Counter(labels.tolist())
        small = [label for label, size in sizes.items() if size < MIN_CLUSTER_WINDOWS]
        if not small or len(sizes) == 1:
            return labels
        smallest = min(small, key=sizes.get)
        centres = {k: ev.unit(vectors[labels == k].mean(0)) for k in sizes if k != smallest}
        for index in np.where(labels == smallest)[0]:
            labels[index] = max(centres, key=lambda k: float(centres[k] @ vectors[index]))


def score(out, scripts_folder):
    """Prints turn identity and short-window diarization error. Side effects: none."""
    scripts, reference = load_all(out, scripts_folder)
    out = Path(out)
    diar = ev.load(out / "o" / "diar.jsonl")
    solo = {ev.SOLO[n]: ev.unit(sum(np.asarray(t["embedding"]) for t in diar[n]["vad_turns"])) for n in ev.SOLO}
    right = total = 0
    for key, record in ev.load(out / "turns.jsonl").items():
        vector = voice(record)
        if vector is not None:
            total += 1
            right += max(solo, key=lambda p: float(solo[p] @ vector)) == key.split("__")[2]
    print(f"turns whose voice is nearest the expected reader: {right}/{total}")

    windows = collections.defaultdict(list)
    for key, record in ev.load(out / "slide.jsonl").items():
        name, start_ms = key.split("__")
        vector = voice(record)
        if vector is not None:
            windows[name].append((int(start_ms), vector))
    for threshold in THRESHOLDS:
        totals, count_right = np.zeros(4), 0
        for name in sorted(scripts):
            items = sorted(windows[name], key=lambda item: item[0])
            labels = cluster(np.array([v for _, v in items]), threshold)
            regions = [dict(label=int(k), start=t + 500, end=t + 1500) for (t, _), k in zip(items, labels)]
            totals += ev.diarization_error(reference[name], regions, diar[name]["duration_ms"])[:4]
            count_right += len(set(labels.tolist())) == len({r["speaker"] for r in reference[name]})
        s, m, a, c = totals
        print(f"t={threshold}: count {count_right}/{len(scripts)}, missed {100*m/s:.1f}%, alarm {100*a/s:.1f}%, "
              f"confusion {100*c/s:.1f}%, error {100*(m+a+c)/s:.1f}%")


if __name__ == "__main__":
    {"cut": cut, "score": score}[sys.argv[1]](sys.argv[2], sys.argv[3])
