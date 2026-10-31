#!/usr/bin/env python3
"""Scores transcription, diarization and voice naming on the recordings in test_scripts/ (#61).

Run the `measure_transcription` and `measure_diarization` tests in
core/adapters/ai/diarization/recordings_measurement_tests.rs first, keeping each configuration's
output as <output dir>/<configuration>.jsonl and the diarization one as diar.jsonl. Then, with numpy
and scipy available:

    python3 tools/diarization/evaluate_pipeline.py --self-test
    python3 tools/diarization/evaluate_pipeline.py <output dir> tools/diarization/test_scripts

Inputs hold what people said and voice embeddings (biometric PII): keep them outside the repository
and delete them afterwards. This script prints only counts and rates, never text or embeddings.

The reference comes from one strong model's word timings (configuration REFERENCE) aligned to the
script, so its times are approximate; time within COLLAR_MS of a reference boundary is not scored.
Clustering and matching re-implement the Rust logic so that thresholds can be swept quickly.
"""

import itertools
import json
import re
import sys
import unicodedata
from pathlib import Path

import numpy as np
from scipy.cluster.hierarchy import fcluster, linkage
from scipy.optimize import linear_sum_assignment

REFERENCE = "ref_turbo_whole_it_b5"
COLLAR_MS = 500
FRAME_MS = 100
PEOPLE = ("ETTORE", "RENZO", "MARTA")
SOLO = {"01_ettore": "ETTORE", "02_renzo": "RENZO", "03_marta": "MARTA"}
STRONG, WEAK, CLUSTER = 0.80, 0.50, 0.40  # The app's cutoffs today.

UNITS = "zero uno due tre quattro cinque sei sette otto nove dieci undici dodici tredici quattordici quindici sedici diciassette diciotto diciannove".split()
TENS = "_ _ venti trenta quaranta cinquanta sessanta settanta ottanta novanta".split()


def number_words(value):
    """Spells a whole number in Italian, without spaces, up to 999,999."""
    if value < 20:
        return UNITS[value]
    if value < 100:
        tens, unit = divmod(value, 10)
        stem = TENS[tens][:-1] if unit in (1, 8) else TENS[tens]
        return stem + (UNITS[unit] if unit else "")
    if value < 1000:
        hundreds, rest = divmod(value, 100)
        return ("cento" if hundreds == 1 else UNITS[hundreds] + "cento") + (number_words(rest) if rest else "")
    thousands, rest = divmod(value, 1000)
    head = "mille" if thousands == 1 else number_words(thousands) + "mila"
    return head + (number_words(rest) if rest else "")


def words(text):
    """Normalizes text to comparable words: lowercase, no accents, digits spelled out."""
    text = text.replace("%", " per cento ").replace("€", " euro ")
    text = re.sub(r"(?<=\d)\.(?=\d{3}\b)", "", text.lower())
    text = "".join(c for c in unicodedata.normalize("NFD", text) if not unicodedata.combining(c))
    result = []
    for token in re.findall(r"[a-z]+|\d+", text):
        result.append(number_words(int(token)) if token.isdigit() and int(token) < 1_000_000 else token)
    return result


def read_script(path):
    """Returns the script's turns as (speaker, [words]) in reading order."""
    turns = []
    for line in path.read_text(encoding="utf-8").splitlines():
        speaker, separator, text = line.partition(": ")
        if separator and speaker in PEOPLE:
            turns.append((speaker, words(text)))
    return turns


def align(reference, hypothesis):
    """Levenshtein alignment. Returns the edit counts and (reference index, hypothesis index) matches."""
    n, m = len(reference), len(hypothesis)
    cost = np.zeros((n + 1, m + 1), dtype=np.int32)
    cost[:, 0], cost[0, :] = np.arange(n + 1), np.arange(m + 1)
    for i in range(1, n + 1):
        for j in range(1, m + 1):
            cost[i, j] = min(
                cost[i - 1, j - 1] + (reference[i - 1] != hypothesis[j - 1]),
                cost[i - 1, j] + 1,
                cost[i, j - 1] + 1,
            )
    pairs, i, j = [], n, m
    while i > 0 and j > 0:
        if cost[i, j] == cost[i - 1, j - 1] + (reference[i - 1] != hypothesis[j - 1]):
            if reference[i - 1] == hypothesis[j - 1]:
                pairs.append((i - 1, j - 1))
            i, j = i - 1, j - 1
        elif cost[i, j] == cost[i - 1, j] + 1:
            i -= 1
        else:
            j -= 1
    return int(cost[n, m]), pairs[::-1]


def hypothesis_words(recording):
    """Expands a run's timed words into (normalized word, start, end), splitting times evenly."""
    expanded = []
    for item in recording["words"]:
        parts = words(item["text"])
        span = max(item["end_ms"] - item["start_ms"], 1)
        for index, part in enumerate(parts):
            expanded.append((part,
                             item["start_ms"] + span * index / len(parts),
                             item["start_ms"] + span * (index + 1) / len(parts)))
    return expanded


def reference_turns(recording, turns):
    """Places each script turn in time from word matches. Returns dicts of speaker, start, end."""
    flat = [(word, index) for index, (_, turn_words) in enumerate(turns) for word in turn_words]
    heard = hypothesis_words(recording)
    _, pairs = align([w for w, _ in flat], [h[0] for h in heard])
    span = {}
    for script_index, heard_index in pairs:
        turn = flat[script_index][1]
        start, end = heard[heard_index][1:]
        low, high = span.get(turn, (start, end))
        span[turn] = (min(low, start), max(high, end))
    placed = [dict(speaker=s, start=span[i][0], end=span[i][1]) for i, (s, _) in enumerate(turns) if i in span]
    for left, right in zip(placed, placed[1:]):
        if left["end"] > right["start"]:
            left["end"] = right["start"] = (left["end"] + right["start"]) / 2
    return placed


def frames_from_regions(regions, duration_ms, key):
    """Returns an object array with the label active in each frame, or None."""
    labels = np.full(duration_ms // FRAME_MS + 1, None, dtype=object)
    for region in regions:
        labels[int(region["start"] // FRAME_MS):int(region["end"] // FRAME_MS)] = region[key]
    return labels


def diarization_error(reference, hypothesis, duration_ms):
    """Returns (scored, missed, false alarm, confusion, speaker map), mapping speakers optimally."""
    truth = frames_from_regions(reference, duration_ms, "speaker")
    guess = frames_from_regions(hypothesis, duration_ms, "label")
    keep = np.ones(len(truth), dtype=bool)
    collar = COLLAR_MS // FRAME_MS
    for region in reference:
        for edge in (region["start"], region["end"]):
            frame = int(edge // FRAME_MS)
            keep[max(frame - collar, 0):frame + collar] = False
    speakers = sorted({r["speaker"] for r in reference})
    labels = sorted({r["label"] for r in hypothesis})
    overlap = np.zeros((len(speakers), max(len(labels), 1)))
    for t, g in zip(truth[keep], guess[keep]):
        if t is not None and g is not None:
            overlap[speakers.index(t), labels.index(g)] += 1
    rows, cols = linear_sum_assignment(-overlap)
    mapping = {labels[c]: speakers[r] for r, c in zip(rows, cols) if labels}
    scored = missed = false_alarm = confused = 0
    for t, g in zip(truth[keep], guess[keep]):
        scored += t is not None
        missed += t is not None and g is None
        false_alarm += t is None and g is not None
        confused += t is not None and g is not None and mapping.get(g) != t
    return scored, missed, false_alarm, confused, mapping


def unit(vector):
    """Scales a vector to unit length."""
    vector = np.asarray(vector, dtype=np.float64)
    norm = np.linalg.norm(vector)
    return vector / norm if norm else vector


def online_cluster(turns, threshold=CLUSTER):
    """The app's `find_or_create_cluster`: joins the best cluster above the threshold, running mean."""
    centres, counts = [], []
    regions = []
    for turn in turns:
        embedding = np.asarray(turn["embedding"], dtype=np.float64)
        scores = [float(unit(c) @ unit(embedding)) for c in centres]
        best = int(np.argmax(scores)) if scores else -1
        if best >= 0 and scores[best] > threshold:
            centres[best] = (centres[best] * counts[best] + embedding) / (counts[best] + 1)
            counts[best] += 1
        else:
            best = len(centres)
            centres.append(embedding)
            counts.append(1)
        regions.append(dict(start=turn["start_ms"], end=turn["end_ms"], label=best, embedding=embedding))
    return regions, centres


def whole_meeting_regions(recording, threshold, min_speech_ms=1000):
    """Clusters every window's local speakers together, then lets windows vote on each moment.

    This is the common offline design: sliding windows, one clustering over the whole meeting.
    """
    items = []
    for window_index, window in enumerate(recording["windows"]):
        for speaker in window["speakers"]:
            if speaker["speech_ms"] >= min_speech_ms:
                items.append((window_index, speaker["local"], unit(speaker["embedding"]), speaker["speech_ms"]))
    if not items:
        return [], {}
    if len(items) == 1:
        assignment = np.array([1])
    else:
        vectors = np.stack([item[2] for item in items])
        assignment = fcluster(linkage(vectors, "average", metric="cosine"), 1 - threshold, "distance")
    link = {(item[0], item[1]): int(label) for item, label in zip(items, assignment)}
    labels = sorted(set(link.values()))
    cells = recording["duration_ms"] // FRAME_MS + 1
    votes = np.zeros((cells, len(labels)))
    covered = np.zeros(cells)
    for window_index, window in enumerate(recording["windows"]):
        for frame, bits in enumerate(window["frames"]):
            time_ms = window["start_ms"] + (frame + 0.5) * window["frame_ms"]
            if time_ms >= window["start_ms"] + window["length_ms"]:
                continue
            cell = int(time_ms // FRAME_MS)
            covered[cell] += 1
            for local in range(3):
                label = link.get((window_index, local))
                if bits >> local & 1 and label is not None:
                    votes[cell, labels.index(label)] += 1
    regions = []
    best = votes.argmax(axis=1)
    active = votes.max(axis=1) * 2 >= np.maximum(covered, 1)
    active &= votes.max(axis=1) > 0
    for cell in range(cells):
        label = int(best[cell]) if active[cell] else None
        if label is None:
            continue
        if regions and regions[-1]["label"] == label and regions[-1]["end"] >= cell * FRAME_MS - 500:
            regions[-1]["end"] = (cell + 1) * FRAME_MS
        else:
            regions.append(dict(start=cell * FRAME_MS, end=(cell + 1) * FRAME_MS, label=label))
    regions = [r for r in regions if r["end"] - r["start"] >= 300]
    centres = {}
    for index, label in enumerate(labels):
        members = [(item[2], item[3]) for item, value in zip(items, assignment) if value == label]
        centres[index] = sum(v * w for v, w in members)
    return regions, centres


def word_truth(recording, turns):
    """Aligns a run's words to the script. Returns heard words and each matched word's true speaker."""
    flat = [turn_speaker for turn_speaker, turn_words in turns for _ in turn_words]
    script = [word for _, turn_words in turns for word in turn_words]
    heard = hypothesis_words(recording)
    edits, pairs = align(script, [h[0] for h in heard])
    return heard, {j: flat[i] for i, j in pairs}, edits, len(script)


def proportional_labels(heard, pieces, regions):
    """The app's `align_chunk_regions`: words split in proportion to time within each chunk."""
    labels = {}
    for piece in pieces:
        indices = [i for i, h in enumerate(heard) if piece["start_ms"] <= (h[1] + h[2]) / 2 < piece["end_ms"]]
        inside = sorted(
            (dict(start=max(r["start"], piece["start_ms"]) - piece["start_ms"],
                  end=min(r["end"], piece["end_ms"]) - piece["start_ms"], label=r["label"])
             for r in regions if r["end"] > piece["start_ms"] and r["start"] < piece["end_ms"]),
            key=lambda r: r["start"])
        merged = []
        for region in inside:
            if merged and merged[-1]["label"] == region["label"]:
                merged[-1]["end"] = max(merged[-1]["end"], region["end"])
            elif merged and region["start"] < merged[-1]["end"]:
                region["start"] = merged[-1]["end"]
                if region["end"] > region["start"]:
                    merged.append(region)
            else:
                merged.append(region)
        if not merged:
            continue
        merged[0]["start"] = 0
        for left, right in zip(merged, merged[1:]):
            left["end"] = right["start"] = (left["end"] + right["start"]) / 2
        duration = piece["end_ms"] - piece["start_ms"]
        merged[-1]["end"] = duration
        next_index = 0
        for position, region in enumerate(merged):
            stop = len(indices) if position == len(merged) - 1 else max(round(len(indices) * region["end"] / duration), next_index)
            for index in indices[next_index:stop]:
                labels[index] = region["label"]
            next_index = stop
    return labels


def timestamp_labels(heard, regions):
    """Gives each word the region at its midpoint, or the nearest region if it falls in a gap."""
    labels = {}
    for index, (_, start, end) in enumerate(heard):
        middle = (start + end) / 2
        nearest = min(regions, default=None, key=lambda r: 0 if r["start"] <= middle < r["end"] else min(abs(r["start"] - middle), abs(r["end"] - middle)))
        if nearest is not None:
            labels[index] = nearest["label"]
    return labels


def word_accuracy(labels, truth, mapping):
    """Counts matched words under the right speaker. Returns (right, total)."""
    right = sum(mapping.get(labels.get(j)) == speaker for j, speaker in truth.items())
    return right, len(truth)


def self_test():
    """Checks the metrics on small cases with known answers."""
    assert number_words(21) == "ventuno" and number_words(38) == "trentotto"
    assert number_words(4200) == "quattromiladuecento" and number_words(1000) == "mille"
    assert words("È il 12% di Ciò") == ["e", "il", "dodici", "per", "cento", "di", "cio"]
    assert align("a b c d".split(), "a x c".split())[0] == 2
    assert align("a b c".split(), "a b c".split())[1] == [(0, 0), (1, 1), (2, 2)]
    reference = [dict(speaker="A", start=0, end=10000), dict(speaker="B", start=10000, end=20000)]
    right = [dict(label=7, start=0, end=10000), dict(label=9, start=10000, end=20000)]
    scored, missed, alarm, confused, mapping = diarization_error(reference, right, 20000)
    assert (missed, alarm, confused) == (0, 0, 0) and mapping == {7: "A", 9: "B"} and scored > 0
    merged = [dict(label=7, start=0, end=20000)]
    scored, missed, alarm, confused, _ = diarization_error(reference, merged, 20000)
    assert confused > 0.4 * scored and missed == 0
    silent = diarization_error(reference, [], 20000)
    assert silent[1] == silent[0]
    regions = [dict(start=0, end=1000, label="x"), dict(start=1000, end=2000, label="y")]
    heard = [("a", 100, 200), ("b", 300, 400), ("c", 500, 600), ("d", 1800, 1900)]
    assert timestamp_labels(heard, regions) == {0: "x", 1: "x", 2: "x", 3: "y"}
    piece = [dict(start_ms=0, end_ms=2000)]
    assert proportional_labels(heard, piece, regions) == {0: "x", 1: "x", 2: "y", 3: "y"}
    turn = lambda v, s, e: dict(embedding=v, start_ms=s, end_ms=e)
    clusters, _ = online_cluster([turn([1, 0], 0, 1), turn([0, 1], 1, 2), turn([1, 0.1], 2, 3)])
    assert [c["label"] for c in clusters] == [0, 1, 0]
    print("self-test passed")


def percent(part, whole):
    """Formats a share."""
    return f"{100 * part / whole:5.1f}%" if whole else "    -"


def load(path):
    """Reads one JSONL output, keyed by recording name."""
    return {r["name"]: r for r in map(json.loads, Path(path).read_text().splitlines())}


def transcription_section(folder, scripts):
    """E1: word error rate per configuration, against the script."""
    print("== E1 transcription, word error rate against the script")
    print(f"{'configuration':30} {'WER':>6} {'x real time':>11}  {'by person: N F V':>16}  worst")
    for path in sorted(folder.glob("*.jsonl")):
        if path.stem == "diar":
            continue
        runs = load(path)
        edits = total = elapsed = duration = 0
        by_person = {p: [0, 0] for p in PEOPLE}
        worst = 0.0
        for name, run in runs.items():
            e, pairs = align([w for _, t in scripts[name] for w in t], [h[0] for h in hypothesis_words(run)])
            n = sum(len(t) for _, t in scripts[name])
            edits, total = edits + e, total + n
            worst = max(worst, e / n)
            elapsed, duration = elapsed + run["elapsed_ms"], duration + run["duration_ms"]
            matched = {i for i, _ in pairs}
            position = 0
            for speaker, turn_words in scripts[name]:
                by_person[speaker][1] += len(turn_words)
                by_person[speaker][0] += sum((position + k) not in matched for k in range(len(turn_words)))
                position += len(turn_words)
        people = " ".join(f"{100 * by_person[p][0] / by_person[p][1]:3.0f}" for p in PEOPLE)
        print(f"{path.stem:30} {100 * edits / total:5.1f}% {elapsed / duration:11.3f}  {people:>16}  {100 * worst:3.0f}%")


def main(folder, scripts_folder):
    """Prints every section."""
    folder = Path(folder)
    scripts = {p.stem: read_script(p) for p in Path(scripts_folder).glob("*.txt")}
    transcription_section(folder, scripts)
    reference_run = load(folder / f"{REFERENCE}.jsonl")
    reference = {n: reference_turns(r, scripts[n]) for n, r in reference_run.items()}
    diar = load(folder / "diar.jsonl")
    names = sorted(scripts)

    print("\n== E3 diarization (error = missed + false alarm + confusion)")
    print(f"{'design':34} {'count ok':>8} {'missed':>7} {'alarm':>7} {'confus':>7} {'DER':>7}")
    def score(regions_of, label):
        totals, right = np.zeros(4), 0
        for name in names:
            regions = regions_of(name)
            s, m, a, c, _ = diarization_error(reference[name], regions, diar[name]["duration_ms"])
            totals += (s, m, a, c)
            right += len({r["label"] for r in regions}) == len({r["speaker"] for r in reference[name]})
        s, m, a, c = totals
        print(f"{label:34} {right:5}/{len(names)} {percent(m, s):>7} {percent(a, s):>7} {percent(c, s):>7} {percent(m + a + c, s):>7}")
        return (m + a + c) / s
    online = {n: online_cluster(diar[n]["vad_turns"])[0] for n in names}
    score(lambda n: online[n], "today: VAD chunks, online 0.40")
    whole_engine = {n: online_cluster(diar[n]["whole_turns"])[0] for n in names}
    score(lambda n: whole_engine[n], "same engine, whole file")
    errors = {}
    for threshold in (0.15, 0.2, 0.25, 0.3, 0.35, 0.4, 0.45, 0.5):
        errors[threshold] = score(lambda n: whole_meeting_regions(diar[n], threshold)[0], f"1 s windows, whole meeting t={threshold}")
    odd, even = names[::2], names[1::2]
    fold_errors = []
    for fit, test in ((odd, even), (even, odd)):
        def fold_error(group, threshold):
            total = np.zeros(4)
            for name in group:
                total += diarization_error(reference[name], whole_meeting_regions(diar[name], threshold)[0], diar[name]["duration_ms"])[:4]
            return (total[1] + total[2] + total[3]) / total[0]
        chosen = min(errors, key=lambda t: fold_error(fit, t))
        fold_errors.append((chosen, fold_error(test, chosen)))
    print("threshold chosen on half, error on the other half:", [(t, round(100 * e, 1)) for t, e in fold_errors])
    best_threshold = min(errors, key=errors.get)
    whole = {n: whole_meeting_regions(diar[n], best_threshold) for n in names}

    print("\n== E2 words to speakers (share of correctly heard words under the right speaker)")
    print(f"{'transcript':28} {'speaker times':24} {'split by time':>13} {'word timing':>11}")
    for config in ("base_vad_auto_g", "base_vad_it_g", "small_vad_it_g", "turbo_vad_it_g", "turbo_whole_it_b5"):
        path = folder / f"{config}.jsonl"
        if config == "turbo_whole_it_b5":
            path = folder / f"{REFERENCE}.jsonl"
        if not path.exists():
            continue
        runs = load(path)
        for design, regions_of in (
            ("reference", lambda n: [dict(start=r["start"], end=r["end"], label=r["speaker"]) for r in reference[n]]),
            ("today", lambda n: online[n]),
            (f"whole meeting t={best_threshold}", lambda n: whole[n][0]),
        ):
            proportional = np.zeros(2)
            timed = np.zeros(2)
            for name in names:
                regions = regions_of(name)
                _, _, _, _, mapping = diarization_error(reference[name], regions, diar[name]["duration_ms"]) if design != "reference" else (0, 0, 0, 0, {p: p for p in PEOPLE})
                heard, truth, _, _ = word_truth(runs[name], scripts[name])
                pieces = runs[name]["pieces"] if len(runs[name]["pieces"]) > 1 else diar[name]["vad_chunks"]
                proportional += word_accuracy(proportional_labels(heard, pieces, regions), truth, mapping)
                timed += word_accuracy(timestamp_labels(heard, regions), truth, mapping)
            print(f"{config:28} {design:24} {percent(*proportional):>13} {percent(*timed):>11}")

    print("\n== E4 naming")
    def speaker_of(region, name):
        best, overlap = None, 0
        for r in reference[name]:
            shared = min(region["end"], r["end"]) - max(region["start"], r["start"])
            if shared > overlap:
                best, overlap = r["speaker"], shared
        return best
    solo = {SOLO[n]: unit(sum(np.asarray(t["embedding"]) for t in diar[n]["vad_turns"])) for n in SOLO}
    print("similarity of the solo voices:", {a[0] + b[0]: round(float(solo[a] @ solo[b]), 2) for a, b in itertools.combinations(PEOPLE, 2)})
    enrolments = {"solo recording": lambda person, meeting: solo[person]}
    def cluster_enrolment(table):
        def enrol(person, meeting):
            candidates = []
            for other in names:
                if other == meeting or other in SOLO:
                    continue
                regions, centres = table[other]
                _, _, _, _, mapping = diarization_error(reference[other], regions, diar[other]["duration_ms"])
                for label, mapped in mapping.items():
                    if mapped == person:
                        candidates.append(unit(centres[label]))
            return candidates
        return enrol
    today_tables = {n: online_cluster(diar[n]["vad_turns"]) for n in names}
    enrolments["named cluster, today"] = cluster_enrolment(today_tables)
    enrolments["named cluster, whole"] = cluster_enrolment(whole)
    meetings = [n for n in names if n not in SOLO]
    for enrol_name, enrol in enrolments.items():
        print(f"-- enrolment: {enrol_name}")
        print(f"{'unit named':24} {'cutoff':>6} {'right':>7} {'wrong':>7} {'unnamed':>8}   right by person N F V")
        for unit_name, cutoff in itertools.product(("each turn (today)", "each speaker, today", "each speaker, whole"), (0.8, 0.7, 0.6, 0.5, 0.4)):
            right = wrong = none = 0.0
            per = {p: [0.0, 0.0] for p in PEOPLE}
            for meeting in meetings:
                if unit_name == "each turn (today)":
                    items = [(r["embedding"], speaker_of(r, meeting), r["end"] - r["start"]) for r in online[meeting]]
                else:
                    table = today_tables if unit_name.endswith("today") else whole
                    regions, centres = table[meeting]
                    _, _, _, _, mapping = diarization_error(reference[meeting], regions, diar[meeting]["duration_ms"])
                    items = []
                    for label, centre in (enumerate(centres) if isinstance(centres, list) else centres.items()):
                        length = sum(r["end"] - r["start"] for r in regions if r["label"] == label)
                        # A group that maps to nobody speaks little; score it against whoever it overlaps most.
                        truth = mapping.get(label) or max(((p, sum(max(0, min(r["end"], q["end"]) - max(r["start"], q["start"])) for r in regions if r["label"] == label for q in reference[meeting] if q["speaker"] == p)) for p in PEOPLE), key=lambda x: x[1])[0]
                        items.append((centre, truth, length))
                for embedding, truth, length in items:
                    if truth is None:
                        continue
                    vector = unit(embedding)
                    scores = {}
                    for person in PEOPLE:
                        voices = enrol(person, meeting)
                        voices = [voices] if isinstance(voices, np.ndarray) else voices
                        scores[person] = max((float(vector @ v) for v in voices), default=-1.0)
                    best = max(scores, key=scores.get)
                    per[truth][1] += length
                    if scores[best] <= cutoff:
                        none += length
                    elif best == truth:
                        right += length
                        per[truth][0] += length
                    else:
                        wrong += length
            total = right + wrong + none
            people = " ".join(f"{100 * per[p][0] / max(per[p][1], 1):3.0f}" for p in PEOPLE)
            print(f"{unit_name:24} {cutoff:6.2f} {percent(right, total):>7} {percent(wrong, total):>7} {percent(none, total):>8}   {people}")

    print("\n== Purity of a meeting's groups (share of a group's time from its main speaker)")
    for label, table in (("today", today_tables), ("whole", whole)):
        pure, length, groups = 0.0, 0.0, 0
        for name in names:
            regions = table[name][0]
            for group in {r["label"] for r in regions}:
                shares = {p: 0.0 for p in PEOPLE}
                for r in regions:
                    if r["label"] == group:
                        for q in reference[name]:
                            shares[q["speaker"]] += max(0, min(r["end"], q["end"]) - max(r["start"], q["start"]))
                pure += max(shares.values())
                length += sum(shares.values())
                groups += 1
        print(f"{label:8} groups {groups:3}  purity {percent(pure, length)}")

    print("\n== Reference sanity")
    print("turns placed:", sum(len(v) for v in reference.values()), "of", sum(len(v) for v in scripts.values()))
    chunk_lengths = [c["end_ms"] - c["start_ms"] for n in names for c in diar[n]["vad_chunks"]]
    print(f"VAD chunks: {len(chunk_lengths)}, median {np.median(chunk_lengths) / 1000:.1f} s")
    print(f"speaker changes per chunk: median {np.median([sum(reference_change(r, c) for r in [reference[n]]) for n in names for c in diar[n]['vad_chunks']]):.1f}")


def reference_change(turns, chunk):
    """Counts reference turn starts inside a chunk."""
    return sum(chunk["start_ms"] <= turn["start"] < chunk["end_ms"] for turn in turns[1:])


if __name__ == "__main__":
    if sys.argv[1:] == ["--self-test"]:
        self_test()
    else:
        main(sys.argv[1], sys.argv[2])
