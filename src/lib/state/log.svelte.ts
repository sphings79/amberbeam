/**
 * The server log: what the core said while it worked.
 *
 * Bounded on purpose. A listing of fifty thousand entries writes a handful of
 * lines, but a reconnect loop writes forever, and a window that grows until it
 * dies is worse than one that forgets the oldest line.
 */

import type { CoreEvent, LogDirection } from "../bridge";

export interface LogLine {
  id: number;
  endpoint: string;
  direction: LogDirection;
  text: string;
  at: Date;
}

const LIMIT = 2000;

let lines = $state<LogLine[]>([]);
let counter = 0;

export function logLines(): LogLine[] {
  return lines;
}

export function clearLog(): void {
  lines = [];
}

/** Takes what belongs in the log out of the core's event stream. */
export function recordEvent(event: CoreEvent): void {
  if (event.event !== "log") {
    return;
  }
  counter += 1;
  const line: LogLine = {
    id: counter,
    endpoint: event.endpoint,
    direction: event.direction,
    text: event.text,
    at: new Date(),
  };
  lines = lines.length >= LIMIT ? [...lines.slice(1), line] : [...lines, line];
}

/**
 * A line the window itself has to say.
 *
 * The log is where what happened to a server is written down, and a file
 * another program saved and this one sent up happened to a server. It belongs
 * with the rest of it rather than in a notice that disappears.
 */
export function note(text: string): void {
  counter += 1;
  const line: LogLine = {
    id: counter,
    endpoint: "",
    direction: "note",
    text,
    at: new Date(),
  };
  lines = lines.length >= LIMIT ? [...lines.slice(1), line] : [...lines, line];
}
