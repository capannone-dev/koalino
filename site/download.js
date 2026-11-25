// Koalino's landing page (#173): points the main Download button straight at the right package
// for the visitor's system, lists that system's other packages under it, and highlights the
// matching platform card and browser-extension button. On an ARM64 computer (#175, #176) the
// Windows and Linux links point at the ARM64 packages instead. Without JavaScript, or on an
// unknown system, the button simply scrolls to the list of all downloads. Reads only what the
// browser says about itself (its user agent and, in Chromium browsers, the processor
// architecture); sends nothing anywhere.

"use strict";

(() => {
  const agent = navigator.userAgent || "";
  const hinted = (navigator.userAgentData && navigator.userAgentData.platform) || "";

  /** The visitor's system: android, ios, windows, mac, linux, or null when it can't be told. */
  function detectPlatform() {
    if (/android/i.test(agent)) return "android";
    if (/iphone|ipad|ipod/i.test(agent)) return "ios";
    if (/windows/i.test(hinted) || /windows/i.test(agent)) return "windows";
    if (/macos/i.test(hinted) || /macintosh|mac os x/i.test(agent)) return "mac";
    if (/linux|chrome os/i.test(hinted) || /linux|x11|cros/i.test(agent)) return "linux";
    return null;
  }

  /**
   * Whether the computer has an ARM processor. Linux browsers name it in the user agent; Windows
   * ones don't (they say x64 even on ARM), but Chromium browsers report it on request.
   * Output: a promise of true or false; false when it can't be told.
   */
  function detectArm() {
    if (/aarch64|arm64|armv8/i.test(agent)) return Promise.resolve(true);
    const data = navigator.userAgentData;
    if (!data || typeof data.getHighEntropyValues !== "function") return Promise.resolve(false);
    return data
      .getHighEntropyValues(["architecture"])
      .then((values) => values.architecture === "arm")
      .catch(() => false);
  }

  /**
   * The Linux package to offer first. Browsers rarely say which distribution they run on; some
   * (Firefox on Ubuntu and Fedora, for example) do. Otherwise the Flatpak, which runs anywhere,
   * or on ARM64, which has no Flatpak, the tarball.
   * Input: arm, whether the computer has an ARM processor.
   */
  function linuxPackage(arm) {
    if (/ubuntu|debian|mint|pop!?_?os|elementary/i.test(agent)) return "deb";
    if (/fedora|opensuse|suse|red ?hat|centos|rocky|alma/i.test(agent)) return "rpm";
    return arm ? "tarball" : "flatpak";
  }

  /** The browser, for the extension card: firefox, chromium, or null. */
  function detectBrowser() {
    if (/firefox|fxios/i.test(agent)) return "firefox";
    if (/chrome|chromium|edg\//i.test(agent)) return "chromium";
    return null;
  }

  const labels = {
    android: "Download for Android",
    windows: "Download for Windows",
    mac: "Download for Mac",
    deb: "Download for Linux (.deb)",
    rpm: "Download for Linux (.rpm)",
    flatpak: "Download for Linux (Flatpak)",
    tarball: "Download for Linux (tarball)",
  };

  const button = document.getElementById("primary-download");
  const note = document.getElementById("primary-note");
  const alternatives = document.getElementById("primary-alternatives");
  const platform = detectPlatform();

  // The browser extension: highlight the button for this browser.
  const browser = detectBrowser();
  const browserButton = browser && document.querySelector(`[data-browser="${browser}"]`);
  if (browserButton) browserButton.classList.replace("secondary", "primary-choice");

  if (!button || !note || !alternatives) return;
  if (platform === "ios") {
    note.textContent = "Not on iPhone or iPad yet. Get it for Android, Windows, Linux or Mac below.";
    return;
  }
  const card = platform && document.querySelector(`[data-platform="${platform}"]`);
  if (!card) return;
  detectArm().then((arm) => offer(card, arm));

  /**
   * Points the main button at the card's package for this computer and highlights the card.
   * Inputs: card, the platform's card; arm, whether the computer has an ARM processor.
   * Side effects: on ARM, rewrites the card's links to their ARM64 packages (data-arm64) and hides
   * the Flatpak, which has no ARM64 build.
   */
  function offer(card, arm) {
    if (arm) {
      for (const armLink of card.querySelectorAll("[data-arm64]")) armLink.href = armLink.dataset.arm64;
      const flatpak = card.querySelector('[data-package="flatpak"]');
      if (flatpak) flatpak.remove();
    }
    let link = card.querySelector("a.button");
    let label = labels[platform] + (arm ? " (ARM64)" : "");
    if (platform === "linux") {
      const chosen = linuxPackage(arm);
      link = card.querySelector(`[data-package="${chosen}"]`) || link;
      label = labels[chosen] + (arm ? " (ARM64)" : "");
      // Show the chosen package as the card's main button too.
      for (const other of card.querySelectorAll(".choices .button")) {
        other.classList.toggle("secondary", other !== link);
      }
      // The other Linux packages, as links under the main button.
      alternatives.replaceChildren(document.createTextNode("Also for Linux: "));
      const others = [...card.querySelectorAll("[data-package]")].filter((a) => a !== link);
      others.forEach((other, index) => {
        if (index > 0) alternatives.append(" · ");
        const copy = document.createElement("a");
        copy.href = other.href;
        copy.textContent = other.textContent.replace(/^Any distribution \((.*)\)$/, "$1");
        alternatives.append(copy);
      });
      alternatives.hidden = false;
    }
    if (!link) return;

    button.href = link.href;
    button.textContent = label;
    card.classList.add("yours");
    note.textContent = "Version 0.2.0 preview · free · other systems below";
  }
})();
