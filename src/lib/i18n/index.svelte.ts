/**
 * Seam 3 (concept paper, section 11): no text stands in the code.
 *
 * Every string of the user interface goes through `t()`. Catalogues are flat
 * JSON files with named keys, readable and editable by hand. English is the
 * fallback: if a key is missing from a translation, the English text appears —
 * never the raw key. `scripts/check-lang.mjs` reports missing and surplus keys
 * on every build, so a half finished translation cannot break the window.
 *
 * Reading further language files from
 * `~/Library/Application Support/AmberBeam/lang/` needs the core and arrives
 * with milestone M5. The shape here already allows for it: a catalogue is
 * nothing but a flat record of strings.
 */

import de from "./de.json";
import en from "./en.json";

export const LOCALES = ["en", "de"] as const;
export type Locale = (typeof LOCALES)[number];

type Catalogue = Record<string, string>;

const catalogues: Record<Locale, Catalogue> = { en, de };

/** English is the fallback and therefore has to be complete. */
const FALLBACK: Locale = "en";

function isLocale(value: string): value is Locale {
  return (LOCALES as readonly string[]).includes(value);
}

/** The language the system asks for, as far as AmberBeam carries it. */
function detect(): Locale {
  for (const tag of navigator.languages ?? [navigator.language]) {
    const primary = tag.split("-")[0]?.toLowerCase();
    if (primary && isLocale(primary)) {
      return primary;
    }
  }
  return FALLBACK;
}

let current = $state<Locale>(detect());

export function locale(): Locale {
  return current;
}

export function setLocale(next: Locale): void {
  current = next;
  document.documentElement.lang = next;
}

/**
 * The text behind a key, with `{name}` placeholders filled in.
 *
 * Reading `current` inside this function is what makes every call site update
 * when the language changes — no store subscription, no reload.
 */
export function t(key: string, params?: Record<string, string | number>): string {
  const text = catalogues[current][key] ?? catalogues[FALLBACK][key];
  if (text === undefined) {
    // Cannot happen once check-lang.mjs has run, and must not crash the window
    // if it ever does.
    console.error(`missing translation key: ${key}`);
    return key;
  }
  if (!params) {
    return text;
  }
  return text.replace(/\{(\w+)\}/g, (whole, name: string) =>
    name in params ? String(params[name]) : whole,
  );
}
