/**
 * Asking GitHub whether a newer release exists.
 *
 * The window makes the request: one address, named by the core and allowed by
 * the content security policy, which saves an HTTP client in Rust for a single
 * call. The judgement is not made here — comparing version numbers is written
 * once in the core and tested there, because a text comparison puts 0.10
 * before 0.9 and that is exactly the release somebody would miss.
 *
 * Nothing is sent: no version, no identifier, no count. And when the setting
 * says no, nothing is asked at all.
 */

import { api, type Release } from "../bridge";

let found = $state<Release | null>(null);
let dismissed = $state(false);

export function availableUpdate(): Release | null {
  return dismissed ? null : found;
}

export function dismissUpdate(): void {
  dismissed = true;
}

/** Runs the check once, if it is wanted. */
export async function checkForUpdate(): Promise<void> {
  try {
    const settings = await api.settings();
    if (!settings.checkForUpdates) return;

    const source = await api.updateSource();
    const response = await fetch(source, {
      headers: { accept: "application/vnd.github+json" },
    });
    if (!response.ok) return;

    found = await api.newerRelease(await response.text());
  } catch {
    // No network, a rate limit, a redesigned answer: none of that is worth
    // troubling the user with. An update check that fails silently is doing
    // its job; one that complains is not.
  }
}
