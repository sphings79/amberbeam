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
 * says no, nothing is asked on its own.
 */

import { api, type Release } from "../bridge";

/**
 * How far the check has got.
 *
 * Kept apart from the answer on purpose. A button that says "up to date"
 * because the request failed is not reporting, it is guessing, and the two
 * look identical from the outside — so "asked and heard nothing" has a state
 * of its own rather than being folded into "nothing newer".
 */
export type UpdateStatus = "idle" | "checking" | "current" | "available" | "unreachable";

let found = $state<Release | null>(null);
let status = $state<UpdateStatus>("idle");

export function availableUpdate(): Release | null {
  return found;
}

export function updateStatus(): UpdateStatus {
  return status;
}

/**
 * Runs the check.
 *
 * Without `onDemand` this is the automatic check at start, which the setting
 * can switch off entirely. A press of the button is `onDemand`: somebody
 * asking in as many words, which the setting about asking *unprompted* has no
 * business refusing.
 */
export async function checkForUpdate(onDemand = false): Promise<void> {
  if (status === "checking") return;
  try {
    if (!onDemand) {
      const settings = await api.settings();
      if (!settings.checkForUpdates) return;
    }

    status = "checking";
    const source = await api.updateSource();
    const response = await fetch(source, {
      headers: { accept: "application/vnd.github+json" },
    });
    if (!response.ok) {
      status = "unreachable";
      return;
    }

    found = await api.newerRelease(await response.text());
    status = found ? "available" : "current";
  } catch {
    // No network, a rate limit, a redesigned answer. None of it is worth a
    // dialog, but it is worth not claiming to be up to date: the button says
    // it could not ask, and pressing it again tries once more.
    status = "unreachable";
  }
}
