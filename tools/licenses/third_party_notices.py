#!/usr/bin/env python3
"""Writes THIRD_PARTY_NOTICES.md: the licenses of everything Koalino builds on (#115).

Koalino is GPL-3.0-or-later. This lists each third-party component and its declared license so
a release can check they are all GPLv3-compatible and ship the notices. It reads:

- Rust crates from `cargo metadata --offline --all-features` (every platform and feature, so a superset of any
  one build);
- the npm packages the web dashboard and the browser extension depend on at run time, from their
  installed `node_modules` (run `npm install` in each first, or they are listed without a license);
- the AI models the app ships or downloads (fixed below; keep in step with `tools/diarization/`).

Flutter and Dart packages are not listed: Flutter gathers their LICENSE files into the app's own
license page (Settings -> About Koalino -> View licenses).

Usage: python3 -I tools/licenses/third_party_notices.py [--check]
  --check  exits with 1 when THIRD_PARTY_NOTICES.md is out of date, writing nothing.
Side effects: writes THIRD_PARTY_NOTICES.md at the repository root unless --check.
"""

import json
import subprocess
import sys
from pathlib import Path

REPOSITORY = Path(__file__).resolve().parents[2]
OUTPUT = REPOSITORY / "THIRD_PARTY_NOTICES.md"

# Models are data, not code dependencies, so no package manager lists them.
MODELS = [
    ("Whisper (ggml conversions by whisper.cpp)", "OpenAI; ggerganov/whisper.cpp", "MIT"),
    ("pyannote segmentation-3.0 (`segmentation.onnx`)", "pyannote", "MIT"),
    ("SpeechBrain ECAPA-TDNN spkrec-ecapa-voxceleb (`embedding.onnx`)", "SpeechBrain", "Apache-2.0"),
]


# Data compiled into the core (`core/assets`).
DATA = [
    (
        "`core/assets/speed_check_clip.wav` (15 s of VoxPopuli Italian test speech)",
        "VoxPopuli (Meta AI); source: European Parliament",
        "CC0-1.0",
    ),
]


# Prebuilt native libraries shipped with the app, which no package manager lists.
NATIVE_LIBRARIES = [
    (
        "ONNX Runtime (`onnxruntime.dll` on Windows, `libonnxruntime.so` on Android)",
        "Microsoft",
        "MIT",
    ),
]

# Files the app downloads only when the owner asks, under their publishers' terms. They are not
# part of Koalino and are never shipped with it (#75).
ON_REQUEST = [
    (
        "QNN runtime 2.50.0 (`libQnnHtp.so` and its companions), for Whisper on a Snapdragon NPU",
        "Qualcomm",
        "Qualcomm AI Stack License",
    ),
    (
        "Whisper Small, precompiled for the Snapdragon 8 Gen 2 NPU (w8a16)",
        "Qualcomm AI Hub; model by OpenAI",
        "Qualcomm's terms for AI Hub models",
    ),
]


# Code ported from other projects, with the notice their license asks to keep.
PORTED = [
    (
        "`core/domain/echo_lines.rs`, `core/domain/echo_detect.rs` and their tests (#108)",
        "Yogurt (FrigadeHQ/yogurt, `app/src/main/asr/echo.ts`, `echoDetect.ts`)",
        "MIT",
    ),
]

# The MIT notice of Yogurt, kept with the ported code as its license requires.
YOGURT_NOTICE = """MIT License

Copyright (c) 2026 Frigade

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE."""


def rust_crates() -> list[tuple[str, str, str]]:
    """Returns (name, version, license) for every crate in the workspace's lock file but ours.

    Side effects: runs `cargo metadata --offline`.
    """
    metadata = json.loads(
        subprocess.run(
            ["cargo", "metadata", "--format-version", "1", "--offline", "--all-features"],
            cwd=REPOSITORY,
            check=True,
            capture_output=True,
            text=True,
        ).stdout
    )
    workspace = set(metadata["workspace_members"])
    crates = {
        (package["name"], package["version"], package.get("license") or "see the crate")
        for package in metadata["packages"]
        if package["id"] not in workspace
    }
    return sorted(crates)


def npm_packages(project: str) -> list[tuple[str, str, str]]:
    """Returns (name, version, license) for a JavaScript project's run-time dependencies.

    Inputs: the project folder relative to the repository. Side effects: reads files only.
    """
    folder = REPOSITORY / project
    manifest = json.loads((folder / "package.json").read_text(encoding="utf-8"))
    packages = []
    for name, wanted in sorted(manifest.get("dependencies", {}).items()):
        installed = folder / "node_modules" / name / "package.json"
        if installed.is_file():
            details = json.loads(installed.read_text(encoding="utf-8"))
            license_name = details.get("license") or "see the package"
            if isinstance(license_name, dict):
                license_name = license_name.get("type", "see the package")
            packages.append((name, details.get("version", wanted), license_name))
        else:
            packages.append((name, wanted, "not installed; see the package"))
    return packages


def table(rows: list[tuple[str, str, str]], headers: tuple[str, str, str]) -> str:
    """Formats rows as a Markdown table."""
    lines = [f"| {headers[0]} | {headers[1]} | {headers[2]} |", "| --- | --- | --- |"]
    lines += [f"| {first} | {second} | {third} |" for first, second, third in rows]
    return "\n".join(lines)


def notices() -> str:
    """Builds the whole file. Side effects: as `rust_crates` and `npm_packages`."""
    sections = [
        "<!-- Generated by tools/licenses/third_party_notices.py; do not edit by hand. Lists the "
        "third-party components Koalino builds on and their licenses. -->",
        "",
        "# Third-party notices",
        "",
        "Koalino is free software under the GNU General Public License, version 3 or (at your "
        "option) any later version; see `LICENSE`. It is built on the components below, each "
        "under its own license. Every license listed is compatible with GPLv3. Where a component "
        "offers a choice (`MIT OR Apache-2.0`), Koalino uses it under any of the options.",
        "",
        "## AI models",
        "",
        table(MODELS, ("Model", "From", "License")),
        "",
        "## Native libraries",
        "",
        table(NATIVE_LIBRARIES, ("Library", "From", "License")),
        "",
        "## Downloaded only on request",
        "",
        "These are not part of Koalino and are not shipped with it. The Android app downloads them "
        "from their publishers only when the owner turns on Whisper on the NPU and accepts the "
        "publisher's terms; each archive is checked against a pinned SHA-256.",
        "",
        table(ON_REQUEST, ("Component", "From", "Terms")),
        "",
        "## Data",
        "",
        table(DATA, ("Data", "From", "License")),
        "",
        "## Ported code",
        "",
        table(PORTED, ("Code", "From", "License")),
        "",
        "Yogurt's notice:",
        "",
        "```text",
        YOGURT_NOTICE,
        "```",
        "",
        "## Rust crates",
        "",
        "Every crate in `Cargo.lock`, for all platforms and features.",
        "",
        table(rust_crates(), ("Crate", "Version", "License")),
    ]
    for project, title in (("web", "Web dashboard"), ("browser-extension", "Browser extension")):
        sections += [
            "",
            f"## {title} (`{project}/`) npm packages",
            "",
            "Run-time dependencies only; their own dependencies come with their licenses in "
            "`node_modules`.",
            "",
            table(npm_packages(project), ("Package", "Version", "License")),
        ]
    sections += [
        "",
        "## Flutter and Dart packages",
        "",
        "The app shows them, with their full license texts, under Settings -> About Koalino -> "
        "View licenses.",
        "",
    ]
    return "\n".join(sections)


def main() -> int:
    """Writes or checks the notices. Outputs: the process exit code."""
    text = notices()
    if "--check" in sys.argv[1:]:
        current = OUTPUT.read_text(encoding="utf-8") if OUTPUT.is_file() else ""
        if current != text:
            print("THIRD_PARTY_NOTICES.md is out of date; run tools/licenses/third_party_notices.py")
            return 1
        return 0
    OUTPUT.write_text(text, encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
