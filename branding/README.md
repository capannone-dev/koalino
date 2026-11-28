<!-- This directory holds Koalino's logo and the script that turns it into every app icon. -->

# Branding

- `koalino.svg`: the logo, a koala holding a notepad on a purple (`#571775`) rounded square. It
  is the single source of every icon (#165).
- `make-icons.sh`: renders the SVG with `rsvg-convert` and ImageMagick into:
  - Android: `ic_launcher.png` per density, plus the adaptive icon (`mipmap-anydpi-v26`, a
    koala-only foreground on the purple background colour, small enough for circular masks);
  - Windows: `ui/windows/runner/resources/app_icon.ico` (16–256 px);
  - macOS: the `AppIcon.appiconset` PNGs, inset to Apple's 824-of-1024 grid;
  - Linux: the window icon `ui/linux/runner/resources/koalino.png`, and the packages' icons in
    `tools/packaging/linux/`;
  - the browser extension's `static/icon-{16,32,48,128}.png`;
  - the server web UI's favicon, `web/app/icon.svg`, and the website's logo, `site/koalino.svg`;
  - the Teams app's `color.png` and `outline.png`;
  - the F-Droid and store listing icon, `fastlane/metadata/android/en-US/images/icon.png`.

After changing the SVG, run `branding/make-icons.sh` and commit the regenerated files.

## Colour tokens (#206)

`tokens.json` holds Koalino's shared colours: the brand colour (the logo's purple, `#571775`) and the accent values
for light and dark schemes, each text pair at 4.5:1 contrast or better. The app seeds its Material 3 scheme from the
brand colour (`ui/lib/presentation/app_theme.dart`); the web dashboard (`web/app/styles.css`) and the browser
extension (`browser-extension/static/koalino.css`, the in-page banner) use the accent values. A test in each project
fails when one drifts from this file.
