/**
 * The little label that appears when the pointer rests on something.
 *
 * The browser has one of these built in, and relying on it was a mistake: in
 * the webview this program runs in it does not appear at all, which made every
 * `title` in the project decoration. This draws its own, so a button explains
 * itself on all three systems in the same way, after the same pause, in the
 * program's own colours.
 *
 * A `title` attribute is still where the text comes from — that is where
 * anybody would write it, and it keeps working in a browser — but it is taken
 * off the element the first time it is hovered. Leaving it would mean two
 * labels on the systems where the built-in one does work.
 */

/** How long somebody has to rest before a label is wanted rather than in the way. */
const PAUSE = 550;

/** Kept clear of the pointer, and clear of the edges of the window. */
const GAP = 8;

interface Held {
  box: HTMLDivElement;
  timer: number | null;
}

/**
 * Moves a `title` out of the way, once, keeping what it said.
 *
 * If the element has no other name, the title becomes its label for a screen
 * reader first: a `title` is doing two jobs, and only one of them is being
 * taken over here.
 */
function claim(element: HTMLElement): string | null {
  const existing = element.dataset.tip;
  if (existing !== undefined) return existing === "" ? null : existing;

  const title = element.getAttribute("title");
  if (title === null) return null;

  if (!element.hasAttribute("aria-label") && element.textContent?.trim() === "") {
    element.setAttribute("aria-label", title);
  }
  element.removeAttribute("title");
  element.dataset.tip = title;
  return title === "" ? null : title;
}

/**
 * Watches one window for anything worth explaining.
 *
 * Applied to the root of each window rather than to every button: the rule is
 * the same everywhere, and a project where each button has to remember to ask
 * for a tooltip is a project where some of them will not.
 */
export function tips(root: HTMLElement) {
  const held: Held = { box: document.createElement("div"), timer: null };
  held.box.className = "amberbeam-tip";
  held.box.setAttribute("role", "tooltip");
  held.box.hidden = true;
  document.body.appendChild(held.box);

  function hide(): void {
    if (held.timer !== null) {
      clearTimeout(held.timer);
      held.timer = null;
    }
    held.box.hidden = true;
  }

  function show(element: HTMLElement, text: string): void {
    held.box.textContent = text;
    held.box.hidden = false;

    // Measured after it is in the document, because its width depends on the
    // text and guessing it puts long labels off the edge of the window.
    const at = element.getBoundingClientRect();
    const box = held.box.getBoundingClientRect();
    const left = Math.min(
      Math.max(GAP, at.left + at.width / 2 - box.width / 2),
      window.innerWidth - box.width - GAP,
    );
    const below = at.bottom + GAP;
    const top = below + box.height < window.innerHeight ? below : at.top - box.height - GAP;
    held.box.style.left = `${Math.round(left)}px`;
    held.box.style.top = `${Math.round(Math.max(GAP, top))}px`;
  }

  function consider(event: Event): void {
    const target = event.target;
    if (!(target instanceof Element)) return;
    const element = target.closest<HTMLElement>("[title], [data-tip]");
    if (!element) {
      hide();
      return;
    }
    const text = claim(element);
    if (text === null) {
      hide();
      return;
    }
    hide();
    held.timer = window.setTimeout(() => show(element, text), PAUSE);
  }

  root.addEventListener("mouseover", consider);
  root.addEventListener("focusin", consider);
  root.addEventListener("mouseleave", hide);
  // Anything that moves or changes what is under the pointer takes the label
  // with it. A label left behind over a window that has scrolled is pointing
  // at something that is no longer there.
  root.addEventListener("mousedown", hide);
  window.addEventListener("keydown", hide);
  window.addEventListener("scroll", hide, true);
  window.addEventListener("blur", hide);

  return {
    destroy() {
      hide();
      root.removeEventListener("mouseover", consider);
      root.removeEventListener("focusin", consider);
      root.removeEventListener("mouseleave", hide);
      root.removeEventListener("mousedown", hide);
      window.removeEventListener("keydown", hide);
      window.removeEventListener("scroll", hide, true);
      window.removeEventListener("blur", hide);
      held.box.remove();
    },
  };
}
