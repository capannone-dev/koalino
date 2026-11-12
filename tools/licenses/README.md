<!-- Explains the license tooling: how THIRD_PARTY_NOTICES.md is generated and checked. -->

# License tooling (`tools/licenses/`)

Koalino is licensed GPL-3.0-or-later (`LICENSE` at the repository root, #115). Everything it is
built on must be under a GPLv3-compatible license, and a release ships their notices.

- `third_party_notices.py` writes `THIRD_PARTY_NOTICES.md`. It lists the AI models, every Rust
  crate in `Cargo.lock` (all platforms and features), and the run-time npm packages of `web/` and
  `browser-extension/`. Flutter and Dart packages are left to the app's own license page
  (Settings → About Koalino → View licenses).

```sh
python3 -I tools/licenses/third_party_notices.py          # regenerate
python3 -I tools/licenses/third_party_notices.py --check  # exit 1 when out of date
```

Run `npm install` in `web/` and `browser-extension/` first, or their packages are listed without
a license. Regenerate after adding or updating a dependency, and check the new license is
GPLv3-compatible. Apache-2.0-only crates are fine under GPLv3, though not under GPLv2. A
proprietary library, such as Qualcomm's QNN runtime for the phone NPU (#75), needs a review
before it ships.
