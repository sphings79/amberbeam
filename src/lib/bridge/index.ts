/**
 * The only door between user interface and core — and which core it opens on.
 *
 * `@bridge-impl` is resolved by `vite.config.ts` and points at `tauri.ts` or
 * `http.ts`, depending on which shell is being built. That is this machine's
 * own core, and it is where everything starts.
 *
 * The desktop program can also point at a service running somewhere else: the
 * same window, the same keys, but the transfers happen over there and carry on
 * after the laptop is shut. That is a runtime choice, not a build-time one, so
 * what leaves here is a stand-in that forwards each call to whichever side is
 * in use.
 *
 * The stand-in is what keeps this from being a rewrite. Every call site in the
 * interface says `api.something()` and goes on saying it; none of them knows
 * or asks which machine answered.
 */

import { api as here } from "@bridge-impl";
import { isNotSignedIn, makeApi } from "./http";
import type { AmberBeamApi } from "./types";

/**
 * Which core is being used.
 *
 * `null` is this machine. Anything else is a service, and the address is kept
 * so the window can say whose files are on screen — a program that can show
 * two machines must never leave you guessing which one you are looking at.
 *
 * A plain variable and not a rune: this is the bridge, not a store, and a
 * rune here would be a rune in a file where they do not run. What is on
 * screen follows from the window's own state, set by whoever called the
 * switch.
 */
let elsewhere: string | null = null;

let current: AmberBeamApi = here;

/**
 * Everything the interface calls, forwarded to whichever core is in use.
 *
 * Bound on the way out: an implementation is free to write a method that uses
 * `this`, and a stand-in that quietly broke those would be a trap nobody would
 * look for.
 */
export const api: AmberBeamApi = new Proxy({} as AmberBeamApi, {
  get(_target, key) {
    const value = (current as unknown as Record<string | symbol, unknown>)[key];
    return typeof value === "function" ? value.bind(current) : value;
  },
  has: (_target, key) => key in (current as object),
});

/** The service in use, or null for this machine. */
export function connectedTo(): string | null {
  return elsewhere;
}

/**
 * Points the window at a service.
 *
 * The token stays in this window's memory and is never written down — the same
 * rule as every other password in this program. Closing the window means
 * signing in again, which is the correct amount of inconvenience.
 */
export function useService(address: string, token: string): void {
  current = makeApi({ base: address.replace(/\/+$/, ""), token });
  elsewhere = address;
}

/** Back to the core in this program. */
export function useThisMachine(): void {
  current = here;
  elsewhere = null;
}

/**
 * Asks a service whether it is one, and trades the password for a token.
 *
 * Deliberately not part of the bridge: it is the one thing that happens before
 * there is a connection to speak of, and putting it behind the same door would
 * mean a door that answers questions about itself.
 */
export async function signIn(
  address: string,
  password: string,
): Promise<{ token: string } | { problem: string }> {
  const base = address.replace(/\/+$/, "");
  try {
    const hello = await fetch(`${base}/api/hello`, { headers: { accept: "application/json" } });
    if (!hello.ok) return { problem: "not-amberbeam" };
    const greeting = (await hello.json()) as { guarded?: boolean; version?: string };
    if (greeting.version === undefined) return { problem: "not-amberbeam" };
    // A service started without a password lets nobody in, and saying "wrong
    // password" for ever would send somebody looking in the wrong place.
    if (greeting.guarded === false) return { problem: "unguarded" };

    const answer = await fetch(`${base}/api/login`, {
      method: "POST",
      headers: { "content-type": "application/json", accept: "application/json" },
      body: JSON.stringify({ password }),
    });
    if (!answer.ok) return { problem: "refused" };
    const { token } = (await answer.json()) as { token: string };
    return { token };
  } catch {
    // No network, a name that resolves to nothing, a certificate the machine
    // will not accept. From here they are one thing: it did not answer.
    return { problem: "unreachable" };
  }
}

/**
 * Signs in to the service that served this page.
 *
 * The container build only. There is no address to give — it is wherever the
 * page came from — and the answer is a cookie the browser keeps, so nothing
 * has to be held here.
 */
export async function signInHere(password: string): Promise<boolean> {
  try {
    const answer = await fetch("/api/login", {
      method: "POST",
      headers: { "content-type": "application/json" },
      credentials: "same-origin",
      body: JSON.stringify({ password }),
    });
    return answer.ok;
  } catch {
    return false;
  }
}

export { isNotSignedIn };
export { LOCAL } from "./types";
export type * from "./types";
