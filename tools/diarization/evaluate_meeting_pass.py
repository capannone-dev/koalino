#!/usr/bin/env python3
"""Scores the real Rust final pass (#73, #74) on the evaluation recordings.

Needs, in OUT/o: ref_turbo_whole_it_b5.jsonl, turbo_chunks_it.jsonl and base_chunks_it.jsonl from
`measure_transcription`, and pass.jsonl from `measure_meeting_pass`. The output holds voice embeddings
and what people said: keep OUT outside the repository and delete it afterwards.

Usage: python evaluate_meeting_pass.py OUT tools/diarization/test_scripts
Only counts and rates are printed.
"""

import collections
import sys
from pathlib import Path

import numpy as np

import evaluate_pipeline as ev

STRONG, WEAK = 0.60, 0.50


def regions_of(record):
    """Returns each window's central second, which is what a voice window speaks for."""
    return [dict(label=w["label"], start=w["start_ms"] + 500, end=w["end_ms"] - 500) for w in record["windows"]
            if w["end_ms"] - w["start_ms"] >= 1000] or [
        dict(label=w["label"], start=w["start_ms"], end=w["end_ms"]) for w in record["windows"]]


def tiled_regions(record):
    """Stretches windows across each VAD chunk, as word assignment does: nearest window wins."""
    regions = []
    for chunk in record["chunks"]:
        inside = [w for w in record["windows"] if chunk["start_ms"] <= w["start_ms"] < chunk["end_ms"]]
        for index, window in enumerate(inside):
            middle = (window["start_ms"] + window["end_ms"]) / 2
            low = chunk["start_ms"] if index == 0 else (middle + (inside[index - 1]["start_ms"] + inside[index - 1]["end_ms"]) / 2) / 2
            high = chunk["end_ms"] if index + 1 == len(inside) else (middle + (inside[index + 1]["start_ms"] + inside[index + 1]["end_ms"]) / 2) / 2
            regions.append(dict(label=window["label"], start=low, end=high))
    return regions


def missed_causes(reference, record):
    """Splits reference speech into covered, outside any chunk, and in a chunk without a window. In ms."""
    covered = outside = no_window = 0
    for turn in reference:
        for t in np.arange(turn["start"], turn["end"], ev.FRAME_MS):
            if any(r["start"] <= t < r["end"] for r in regions_of(record)):
                covered += ev.FRAME_MS
            elif any(c["start_ms"] <= t < c["end_ms"] for c in record["chunks"]):
                no_window += ev.FRAME_MS
            else:
                outside += ev.FRAME_MS
    return covered, outside, no_window


def main(out, scripts_folder):
    """Prints speaker error, word accuracy, naming and clean-group figures. Side effects: none."""
    out = Path(out) / "o"
    scripts = {p.stem: ev.read_script(p) for p in Path(scripts_folder).glob("*.txt")}
    reference_run = ev.load(out / f"{ev.REFERENCE}.jsonl")
    passes = ev.load(out / "pass.jsonl")
    reference = {n: ev.reference_turns(reference_run[n], scripts[n]) for n in scripts}
    for regions_name, regions_for in (("voice windows only", regions_of), ("stretched across chunks", tiled_regions)):
        totals, right_count = np.zeros(4), 0
        for name in sorted(scripts):
            record = passes[name]
            regions = regions_for(record)
            totals += ev.diarization_error(reference[name], regions, record["duration_ms"])[:4]
            right_count += len({w["label"] for w in record["windows"]}) == len({r["speaker"] for r in reference[name]})
        s, m, a, c = totals
        print(f"{regions_name}: count {right_count}/{len(scripts)}, missed {100*m/s:.1f}%, alarm {100*a/s:.1f}%, "
              f"confusion {100*c/s:.1f}%, error {100*(m+a+c)/s:.1f}%")

    print("missed speech per file (outside chunks / in a chunk with no window):")
    for name in sorted(scripts):
        covered, outside, no_window = missed_causes(reference[name], passes[name])
        total = covered + outside + no_window
        if outside + no_window > total * 0.05:
            print(f"  {name}: {100*outside/total:.1f}% / {100*no_window/total:.1f}%")

    for run_name in ("turbo_chunks_it", "base_chunks_it"):
        runs = ev.load(out / f"{run_name}.jsonl")
        right = total = 0
        for name in sorted(scripts):
            heard, truth, _, _ = ev.word_truth(runs[name], scripts[name])
            regions = tiled_regions(passes[name])
            mapping = ev.diarization_error(reference[name], regions, passes[name]["duration_ms"])[4]
            r, t = ev.word_accuracy(ev.timestamp_labels(heard, regions), truth, mapping)
            right += r
            total += t
        print(f"words under the right speaker, {run_name}: {100*right/total:.1f}% of {total}")

    solo = {}
    for name, person in ev.SOLO.items():
        solo[person] = ev.unit(sum(np.asarray(w["embedding"]) for w in passes[name]["windows"]))
    outcome = collections.Counter()
    groups = []
    for name in sorted(scripts):
        if name in ev.SOLO:
            continue
        record = passes[name]
        mapping = ev.diarization_error(reference[name], tiled_regions(record), record["duration_ms"])[4]
        for label in sorted({w["label"] for w in record["windows"]}):
            members = [w for w in record["windows"] if w["label"] == label]
            vectors = np.array([ev.unit(np.asarray(w["embedding"])) for w in members])
            centre = ev.unit(vectors.mean(0))
            person = max(solo, key=lambda p: float(solo[p] @ centre))
            similarity = float(solo[person] @ centre)
            truth_share = collections.Counter()
            for w in members:
                middle = (w["start_ms"] + w["end_ms"]) / 2
                for turn in reference[name]:
                    if turn["start"] <= middle < turn["end"]:
                        truth_share[turn["speaker"]] += 1
            top = truth_share.most_common(1)[0][1] / max(sum(truth_share.values()), 1) if truth_share else 0
            true_person = mapping.get(label)
            state = "named" if similarity >= STRONG else "review" if similarity >= WEAK else "unknown"
            outcome[state, person == true_person] += len(members)
            groups.append((round(float((vectors @ centre).mean()), 3), round(top, 3), len(members), round(similarity, 3), person == true_person))
    total = sum(outcome.values())
    for key, value in sorted(outcome.items()):
        print(f"naming {key[0]} {'right' if key[1] else 'wrong'}: {100*value/total:.1f}% of windows")
    print("groups (consistency, purity, windows, similarity, nearest is right), lowest purity first:")
    for group in sorted(groups, key=lambda g: g[1])[:40]:
        print(" ", group)
    print("seconds of work per second of audio:",
          round(sum(p["embed_ms"] for p in passes.values()) / sum(p["duration_ms"] for p in passes.values()), 3),
          "grouping ms, largest:", max(p["group_ms"] for p in passes.values()))


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
