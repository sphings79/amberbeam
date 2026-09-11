/**
 * Keeps the tab key inside a dialog.
 *
 * A dialog here is a box drawn over the window, not something the browser knows
 * is modal — so without this, Tab walks straight out of it into the toolbar
 * behind, and the next space bar presses a button nobody can see. That is not
 * a nicety: it happened while testing the keyboard dialog, and what it pressed
 * was "connect".
 *
 * Listening on the document during capture, so it runs before the window's own
 * key handler and before anything inside the dialog.
 */
export function trap(node: HTMLElement) {
  const reachable = (): HTMLElement[] =>
    Array.from(
      node.querySelectorAll<HTMLElement>(
        'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
      ),
      // A hidden element is still in the tree and still matches the selector;
      // sending focus to one puts the cursor nowhere at all.
    ).filter((element) => element.offsetParent !== null || element === document.activeElement);

  function onKey(event: KeyboardEvent): void {
    if (event.key !== "Tab") return;
    const items = reachable();
    if (items.length === 0) {
      event.preventDefault();
      return;
    }

    const first = items[0];
    const last = items[items.length - 1];
    const active = document.activeElement as HTMLElement | null;

    if (!active || !node.contains(active)) {
      event.preventDefault();
      (event.shiftKey ? last : first)?.focus();
      return;
    }
    if (!event.shiftKey && active === last) {
      event.preventDefault();
      first?.focus();
    } else if (event.shiftKey && active === first) {
      event.preventDefault();
      last?.focus();
    }
  }

  document.addEventListener("keydown", onKey, true);
  return {
    destroy() {
      document.removeEventListener("keydown", onKey, true);
    },
  };
}
