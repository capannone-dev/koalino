<!-- Data compiled into the Rust core with include_bytes!. -->

# Core assets

- `speed_check_clip.wav`: 15 s of Italian speech, 16 kHz mono 16-bit PCM. `bin/speed_check.rs`
  transcribes it on the device to time Automatic's Whisper model (#109); it is never played. It is
  the first 15 s of utterance `20130522-0900-PLENARY-15-it_20130522-17:09:14_2` of the VoxPopuli
  Italian test set, released under CC0 1.0. Source: European Parliament.

Keep anything added here small, public and licensed compatibly with GPLv3, and list it in
`THIRD_PARTY_NOTICES.md` through `tools/licenses/third_party_notices.py`.
