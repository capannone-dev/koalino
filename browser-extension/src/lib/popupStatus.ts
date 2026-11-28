/**
 * Words the popup's connection line (#212): what the owner should know about the Koalino helper
 * and app, as a message and a tone. Kept apart from the popup so it can be tested without a page.
 */
import type { MessageKey } from "./i18n";
import type { ConnectionState } from "./messages";

/** How the line looks: fine, a warning, a problem, or still being checked. */
export type StatusTone = "ok" | "warn" | "bad" | "checking";

/**
 * Returns the message and tone for `connection`.
 *
 * Inputs: The background's connection state. Outputs: The message key and the tone; an unknown
 * state reads "Checking…", since the popup checks as soon as it opens.
 */
export function connectionStatus(connection: ConnectionState): { key: MessageKey; tone: StatusTone } {
  if (connection.host === "missing") return { key: "helperMissing", tone: "bad" };
  if (connection.bridge === "connected") return { key: "connectedToApp", tone: "ok" };
  if (connection.bridge === "unavailable") return { key: "appNotRunning", tone: "warn" };
  return { key: "checking", tone: "checking" };
}
