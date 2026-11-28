// Helpers for the dashboard's keyboard shortcuts (#215): whether a key press belongs to a field the
// owner is typing in, so single-key shortcuts never steal text.

/**
 * Whether `target` takes typed text (an input, a text area, a menu or editable content), or is a
 * button or link that Space and Enter already press.
 *
 * Inputs: the event's target. Outputs: true when single-key shortcuts must leave the key alone.
 */
export function isTypingTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target.isContentEditable) return true;
  return ["INPUT", "TEXTAREA", "SELECT", "BUTTON", "A"].includes(target.tagName);
}

/** Whether a key press holds Control, Command or Alt, which typing alone never does. */
export function hasCommandModifier(event: KeyboardEvent): boolean {
  return event.ctrlKey || event.metaKey || event.altKey;
}
