/**
 * Numbers, sizes and dates in the language the window is set to.
 *
 * Section 11 of the concept paper is explicit about this: formatting comes from
 * the locale, never from hand written code. Otherwise the German window ends up
 * showing `1,024.5 KB`, which is neither German nor right.
 */

import { locale } from "../i18n/index.svelte";

/** Binary prefixes, the way a file manager counts. */
export function formatSize(bytes: number | null): string {
  if (bytes === null) return "—";
  if (bytes < 1024) {
    return `${new Intl.NumberFormat(locale()).format(bytes)} B`;
  }
  const units = ["KB", "MB", "GB", "TB", "PB"];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const digits = value < 10 ? 1 : 0;
  return `${new Intl.NumberFormat(locale(), {
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
  }).format(value)} ${units[unit]}`;
}

/** Short for this year, with the year for anything older. */
export function formatDate(seconds: number | null): string {
  if (seconds === null) return "—";
  const when = new Date(seconds * 1000);
  const thisYear = when.getFullYear() === new Date().getFullYear();
  return new Intl.DateTimeFormat(locale(), {
    day: "2-digit",
    month: "short",
    year: thisYear ? undefined : "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(when);
}

/** The nine permission bits the way `ls` prints them. */
export function formatPermissions(bits: number | null): string {
  if (bits === null) return "";
  const flags = "rwxrwxrwx";
  let out = "";
  for (let index = 0; index < 9; index += 1) {
    out += bits & (1 << (8 - index)) ? flags[index] : "-";
  }
  return out;
}

export function formatTime(at: Date): string {
  return new Intl.DateTimeFormat(locale(), {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  }).format(at);
}
