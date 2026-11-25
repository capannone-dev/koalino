// Koalino's landing page (#173): points the main Download button straight at the package for the
// visitor's system and highlights that platform's card. Without JavaScript, or on an unknown
// system, the button simply scrolls to the list of all downloads. Reads only the browser's own
// user agent; sends nothing anywhere.

"use strict";

(() => {
  /** The platform the visitor's browser runs on, or null when it can't be told. */
  function detectPlatform() {
    const agent = navigator.userAgent || "";
    const hinted = (navigator.userAgentData && navigator.userAgentData.platform) || "";
    if (/android/i.test(agent)) return "android";
    if (/iphone|ipad|ipod/i.test(agent)) return "ios";
    if (/windows/i.test(hinted) || /windows/i.test(agent)) return "windows";
    if (/macos/i.test(hinted) || /macintosh|mac os x/i.test(agent)) return "mac";
    if (/linux/i.test(hinted) || /linux|x11/i.test(agent)) return "linux";
    return null;
  }

  /** The button labels for each platform; the link comes from that platform's card. */
  const labels = {
    android: "Download for Android",
    windows: "Download for Windows",
    mac: "Download for Mac",
    linux: "Download for Linux (.deb)",
  };

  const platform = detectPlatform();
  const button = document.getElementById("primary-download");
  const note = document.getElementById("primary-note");
  if (!button || !note) return;

  if (platform === "ios") {
    note.textContent = "Not on iPhone or iPad yet. Get it for Android, Windows, Linux or Mac below.";
    return;
  }
  const card = platform && document.querySelector(`[data-platform="${platform}"]`);
  const link = card && card.querySelector("a.button");
  if (!card || !link) return;

  button.href = link.href;
  button.textContent = labels[platform];
  card.classList.add("yours");
  note.textContent = "Version 0.2.0 preview · free · other systems below";
})();
