/**
 * Appearance: theme and accent, the same pair AmberChest offers.
 *
 * `theme` is what the user picked — light, dark or system. What ends up on the
 * document element is always the resolved value, because the stylesheet knows
 * only light and dark. While "system" is selected, a change of the system
 * setting is followed live.
 *
 * Until the settings of milestone M5 exist, the choice lives in the browser
 * storage of the webview. From M5 it belongs in the configuration file the core
 * owns, so that the container build (M7) has it too.
 */

export const THEMES = ["system", "light", "dark"] as const;
export const ACCENTS = ["amber", "violet", "blue", "emerald", "rose"] as const;

export type Theme = (typeof THEMES)[number];
export type Accent = (typeof ACCENTS)[number];

const THEME_KEY = "amberbeam.theme";
const ACCENT_KEY = "amberbeam.accent";

function stored<T extends string>(key: string, allowed: readonly T[], fallback: T): T {
  const value = localStorage.getItem(key);
  return value !== null && (allowed as readonly string[]).includes(value) ? (value as T) : fallback;
}

/**
 * What somebody gets who has never chosen, and what "reset" means.
 *
 * Named rather than written twice. A default that appears once at startup and
 * again in a reset button is a default that will disagree with itself the
 * first time one of them is changed.
 */
export const DEFAULT_THEME: Theme = "system";
export const DEFAULT_ACCENT: Accent = "amber";

let theme = $state<Theme>(stored(THEME_KEY, THEMES, DEFAULT_THEME));
let accent = $state<Accent>(stored(ACCENT_KEY, ACCENTS, DEFAULT_ACCENT));

const darkQuery = window.matchMedia("(prefers-color-scheme: dark)");

// Tracked as state of its own: while "system" is selected, `theme` does not
// change when the system does, so anything reading the resolved theme would
// otherwise never hear about it.
let systemDark = $state(darkQuery.matches);

function resolve(choice: Theme): "light" | "dark" {
  if (choice === "system") {
    return systemDark ? "dark" : "light";
  }
  return choice;
}

function apply(): void {
  const root = document.documentElement;
  root.dataset.theme = resolve(theme);
  root.dataset.accent = accent;
}

darkQuery.addEventListener("change", (event) => {
  systemDark = event.matches;
  if (theme === "system") {
    apply();
  }
});

apply();

export function currentTheme(): Theme {
  return theme;
}

export function currentAccent(): Accent {
  return accent;
}

/** The theme actually in force, with "system" already resolved. */
export function resolvedTheme(): "light" | "dark" {
  return resolve(theme);
}

export function setTheme(next: Theme): void {
  theme = next;
  localStorage.setItem(THEME_KEY, next);
  apply();
}

export function setAccent(next: Accent): void {
  accent = next;
  localStorage.setItem(ACCENT_KEY, next);
  apply();
}
