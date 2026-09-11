/**
 * Which key does what, right now.
 *
 * The table in `schemes.ts` says what the three schemes offer; this holds the
 * one in force, whatever the user changed on top of it, and whether they have
 * been asked at all. It is kept beside the rest of the window's state — the
 * core stores that as opaque JSON and deliberately does not look inside, which
 * is exactly right for something as much a matter of taste as a keyboard.
 */

import {
  actionFor,
  bindingOf,
  FALLBACK,
  resolve,
  schemeBindings,
  type Action,
  type Bindings,
  type SchemeName,
} from "./schemes";

let scheme = $state<SchemeName>(FALLBACK);
let own = $state<Bindings>({});
/** False until somebody has answered the dialog, which is what opens it. */
let answered = $state(false);

export function currentScheme(): SchemeName {
  return scheme;
}

export function hasAnswered(): boolean {
  return answered;
}

/** The scheme with the user's own changes on top. */
export function bindings(): Bindings {
  return resolve(scheme, own);
}

/** What a press means, or null when it means nothing here. */
export function actionOf(event: KeyboardEvent): Action | null {
  return actionFor(bindings(), bindingOf(event));
}

/** The keys an action answers to, for showing beside it. */
export function keysFor(action: Action): string[] {
  return bindings()[action] ?? [];
}

export function setScheme(name: SchemeName): void {
  scheme = name;
  answered = true;
}

/**
 * Points one action at one key, taking it off whatever else had it.
 *
 * Taking it away rather than refusing: somebody who just pressed a key meant
 * that key, and being told "no, it is taken" by a program that could simply
 * move it is the sort of politeness nobody asked for. What it did before is
 * shown, so the exchange is visible.
 */
export function assign(action: Action, binding: string): Action | null {
  const current = bindings();
  let displaced: Action | null = null;

  for (const other of Object.keys(current) as Action[]) {
    if (other === action) continue;
    const keys = current[other] ?? [];
    if (keys.includes(binding)) {
      displaced = other;
      own = { ...own, [other]: keys.filter((key) => key !== binding) };
    }
  }

  own = { ...own, [action]: [binding] };
  answered = true;
  return displaced;
}

/** Gives an action no key at all. */
export function unassign(action: Action): void {
  own = { ...own, [action]: [] };
  answered = true;
}

/** Puts an action back to what the scheme says. */
export function reset(action: Action): void {
  const { [action]: _dropped, ...rest } = own;
  own = rest;
}

/** Puts everything back, and forgets every change. */
export function resetAll(): void {
  own = {};
}

/** Whether anything has been moved off the scheme. */
export function changed(): boolean {
  return Object.keys(own).length > 0;
}

/** What goes into the saved window state. */
export interface SavedKeys {
  scheme: SchemeName;
  own: Bindings;
  answered: boolean;
}

export function saved(): SavedKeys {
  return { scheme, own, answered };
}

export function restore(state: unknown): void {
  const value = state as Partial<SavedKeys> | null | undefined;
  if (!value) return;
  if (value.scheme && schemeBindings(value.scheme)) scheme = value.scheme;
  if (value.own && typeof value.own === "object") own = value.own;
  answered = value.answered === true;
}

/** A whole scheme as one file, for taking it to another machine. */
export function toFile(): string {
  return JSON.stringify({ version: 1, scheme, own }, null, 2);
}

/**
 * Reads a scheme file back.
 *
 * Anything it does not understand is left alone rather than guessed at: a
 * keyboard that half applied would be worse than one that did not change.
 */
export function fromFile(text: string): boolean {
  try {
    const read = JSON.parse(text) as { version?: number; scheme?: SchemeName; own?: Bindings };
    if (read.version !== 1) return false;
    if (!read.scheme || !schemeBindings(read.scheme)) return false;
    scheme = read.scheme;
    own = read.own && typeof read.own === "object" ? read.own : {};
    answered = true;
    return true;
  } catch {
    return false;
  }
}

export { bindingOf, label } from "./schemes";
export { ACTIONS, SCHEMES, schemeBindings } from "./schemes";
export type { Action, Bindings, SchemeName } from "./schemes";
