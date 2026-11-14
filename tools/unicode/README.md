<!-- Documents the generator of the Unicode tables the core needs. -->

# Unicode tables

Rust's standard library has no Unicode general-category lookup. Where the core must match a
pattern that uses categories, a small table is generated here instead of adding a library.

| File | Purpose |
| --- | --- |
| `letter_ranges.py` | Writes `core/adapters/ai/npu_whisper/unicode_letters.rs`, the code point ranges of the letters (`\p{L}`). The NPU tokenizer needs them for GPT-2's word split, which keeps combining marks (Thai, Indic and Arabic vowel signs, for example) apart from letters; `char::is_alphabetic` does not. |

Regenerate with the Unicode version you want (Python's `unicodedata` decides it):

```bash
python3 -I tools/unicode/letter_ranges.py > core/adapters/ai/npu_whisper/unicode_letters.rs
```
