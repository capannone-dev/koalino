<!-- This directory is Koalino's website, published on GitHub Pages. -->

# Website

Koalino's website (#166, #173), published by `.github/workflows/pages.yml` at
<https://capannone-dev.github.io/koalino/>.

- `index.html`: the landing page: the main features, screenshots on a computer and a phone, and
  direct downloads for every platform.
- `download.js`: points the main Download button at the visitor's own system (on Linux, the
  `.deb` or `.rpm` when the browser names the distribution, otherwise the Flatpak) and highlights its
  card. It reads only the browser's user agent and sends nothing.
- `privacy.html`: the privacy policy the app stores and F-Droid link to (#127, #163). It must
  match what the apps store and send: update it with any change to that.
- `style.css`: the shared styles, light and dark.
- `koalino.svg`: the logo, copied by `branding/make-icons.sh`.
- `icons/`: platform and browser logos, copied from `docs/icons/` (see its README for their source and
  licence).
- `screens/`: `desktop.png` is `docs/screenshots/desktop-meeting.png`; `phone.png` is the same
  app in its phone layout. Both show an excerpt of AMI meeting ES2004a (CC BY 4.0), never a real
  meeting.

## New release

The download links name the release tag (`v0.2.0-preview.2`) and the package files. For a new
release, replace them in `index.html`, and the version in `download.js`'s note.

Plain HTML, CSS and a little JavaScript, with no build step and nothing loaded from other sites.
Preview it by opening `index.html` in a browser.
