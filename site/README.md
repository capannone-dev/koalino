<!-- This directory is Koalino's website, published on GitHub Pages. -->

# Website

Koalino's website (#166, #173), published by `.github/workflows/pages.yml` at
<https://capannone-dev.github.io/koalino/>.

- `index.html`: the landing page: the main features, screenshots on a computer and a phone, and
  direct downloads for every platform.
- `download.js`: points the main Download button at the visitor's own system (on Linux, the
  `.deb` or `.rpm` when the browser names the distribution, otherwise the Flatpak) and highlights its
  card. It reads only the browser's user agent and sends nothing.
- `guide.html`: how to use Koalino (#178): installing it on a computer and a phone, the browser
  extension, pairing devices, and recording meetings in the room and online, with screenshots.
- `privacy.html`: the privacy policy the app stores and F-Droid link to (#127, #163). It must
  match what the apps store and send: update it with any change to that.
- `style.css`: the shared styles, light and dark.
- `koalino.svg`: the logo, copied by `branding/make-icons.sh`.
- `icons/`: platform and browser logos, copied from `docs/icons/` (see its README for their source and
  licence).
- `screens/`: the landing page's `desktop.png` and `phone.png`, and `guide/`, the guide's
  screenshots. All come from 0.2.0 (#178) and show only test data:
  - **Meetings:** excerpts of AMI meetings ES2004a and ES2004b (CC BY 4.0), never a real meeting.
    ES2004a was imported with `cli record start --input`. ES2004b was played into a virtual
    microphone on the test VM while the app recorded it live.
  - **Computer:** the Linux app on Debian 13 (`deb-gui`, 1280×800), with its own throwaway data
    folders; the window's content, cropped to 1280×722.
  - **Phone:** the Android app on the emulator (Pixel 6), with the status bar in demo mode. The
    phone and the computer were paired with an invite, so the phone shows the synced meeting.
  - **Extension and installer:** the extension in Chromium during a test call between two VMs.
    Each picture is cropped so that no real name or photo shows. The Windows installer is from
    `win11`.

## New release

The download links name the release tag (`v0.2.0-preview.2`) and the package files. For a new
release, replace them in `index.html`, and the version in `download.js`'s note.

Plain HTML, CSS and a little JavaScript, with no build step and nothing loaded from other sites.
Preview it by opening `index.html` in a browser.
