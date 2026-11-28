/**
 * The options page (#212): the extension's settings, grouped as "All sites" (watching every
 * website for calls, keeping shared slides, the per-site Always / Never choices) and "Teams".
 * They moved here from the popup, which now only shows the status and Record this tab.
 * Every switch saves as soon as it changes.
 */
import { ANY_SITE_PATTERNS } from "../lib/anySite";
import { msg } from "../lib/i18n";
import { translatePage } from "../lib/pageText";
import { ExtensionSettings } from "../lib/settings";
import { MEETING_SERVICES } from "../lib/sites";
import { loadSettings, saveSettings } from "../lib/storage";

/** A site key as the owner knows it: the service's name, else the host. */
function siteLabel(key: string): string {
  return MEETING_SERVICES.find((service) => service.id === key)?.name ?? key;
}

/** Shows the per-site choices, or hides the line when there are none. */
function renderSiteChoices(settings: ExtensionSettings): void {
  const line = document.getElementById("site-choices");
  const text = document.getElementById("site-choices-text");
  if (!line || !text) return;
  const list = (keys: string[]) => (keys.length > 0 ? keys.map(siteLabel).join(", ") : msg("noSites"));
  text.textContent = msg("siteChoices", { always: list(settings.alwaysRecordSites), never: list(settings.neverAskSites) });
  line.hidden = settings.alwaysRecordSites.length === 0 && settings.neverAskSites.length === 0;
}

/** The settings shown as checkboxes. */
type SwitchSetting = { [Key in keyof ExtensionSettings]: ExtensionSettings[Key] extends boolean ? Key : never }[keyof ExtensionSettings];

/**
 * Binds the checkboxes to the settings.
 * Side effects: Reads the settings; writes one whenever its checkbox changes.
 */
async function bindSettings(): Promise<void> {
  const settings = await loadSettings();
  for (const input of document.querySelectorAll<HTMLInputElement>("input[data-setting]")) {
    const key = input.dataset.setting as SwitchSetting;
    input.checked = settings[key];
    input.addEventListener("change", () => {
      void saveSettings({ [key]: input.checked });
    });
  }
  renderSiteChoices(settings);
  document.getElementById("forget-sites")?.addEventListener("click", () => {
    void saveSettings({ alwaysRecordSites: [], neverAskSites: [] }).then(renderSiteChoices);
  });
  await bindWatchAllSites(settings);
}

/**
 * Binds "Watch every website for calls": on, it asks the browser for access to all sites first.
 * Side effects: May show the browser's permission prompt; writes the setting.
 */
async function bindWatchAllSites(settings: ExtensionSettings): Promise<void> {
  const input = document.getElementById("watch-all-sites") as HTMLInputElement | null;
  if (!input) return;
  input.checked = settings.watchAllSites && (await chrome.permissions.contains({ origins: ANY_SITE_PATTERNS }));
  input.addEventListener("change", () => {
    if (!input.checked) {
      void saveSettings({ watchAllSites: false });
      return;
    }
    // The request must start in the click itself, before any other await.
    void chrome.permissions.request({ origins: ANY_SITE_PATTERNS }).then((granted) => {
      input.checked = granted;
      void saveSettings({ watchAllSites: granted });
    });
  });
}

translatePage();
void bindSettings();
