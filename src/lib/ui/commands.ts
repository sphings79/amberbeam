/**
 * The commands a file pane offers.
 *
 * One list, used by the toolbar and by the right-click menu. Two lists would
 * drift apart within a week, and then a command exists in one place and not in
 * the other — which is exactly how a program starts feeling unfinished.
 */

import type { DirEntry } from "../bridge";

export type CommandId =
  | "reload"
  | "new-folder"
  | "new-file"
  | "rename"
  | "permissions"
  | "delete"
  | "transfer"
  | "enqueue"
  | "download"
  | "edit-remote";

export interface Command {
  id: CommandId;
  /** Translation key for the label. */
  key: string;
  /** Name of the drawn icon, see Icon.svelte. */
  icon: string;
  /** Shown in the toolbar, or only in the menu. */
  inToolbar: boolean;
  /** Needs at least one row selected or under the cursor. */
  needsTarget: boolean;
  /** Only offered on a remote pane. */
  remoteOnly?: boolean;
  /**
   * Only where the window and the files are on different computers.
   *
   * Which is the container build and nothing else. In the desktop program the
   * local side already is your disk, and offering to download it would be
   * offering to copy a file onto itself.
   */
  awayOnly?: boolean;
  /** Only offered on a single row, not a selection. */
  singleOnly?: boolean;
  /**
   * Offered, but not built.
   *
   * It used to name the milestone that would bring it, and said "M2" long
   * after M2 had shipped without it — a promise in the interface that had
   * quietly become untrue. A date nobody is holding to is worse than no date:
   * it says the program knows something it does not.
   */
  notYet?: boolean;
}

export const COMMANDS: Command[] = [
  { id: "reload", key: "cmd.reload", icon: "reload", inToolbar: true, needsTarget: false },
  { id: "new-folder", key: "cmd.new-folder", icon: "new-folder", inToolbar: true, needsTarget: false },
  { id: "new-file", key: "cmd.new-file", icon: "new-file", inToolbar: true, needsTarget: false },
  { id: "rename", key: "cmd.rename", icon: "rename", inToolbar: true, needsTarget: true, singleOnly: true },
  { id: "permissions", key: "cmd.permissions", icon: "permissions", inToolbar: true, needsTarget: true },
  { id: "delete", key: "cmd.delete", icon: "delete", inToolbar: true, needsTarget: true },
  { id: "transfer", key: "cmd.transfer", icon: "transfer", inToolbar: true, needsTarget: true },
  {
    id: "enqueue",
    key: "cmd.enqueue",
    icon: "transfer",
    inToolbar: false,
    needsTarget: true,
  },
  {
    id: "download",
    key: "cmd.download",
    icon: "update",
    inToolbar: false,
    needsTarget: true,
    awayOnly: true,
  },
  {
    id: "edit-remote",
    key: "cmd.edit-remote",
    icon: "edit-remote",
    inToolbar: false,
    needsTarget: true,
    remoteOnly: true,
    singleOnly: true,
    notYet: true,
  },
];

/** Whether a command can be used right now, and why not when it cannot. */
export function availability(
  command: Command,
  options: { remote: boolean; targets: DirEntry[]; away?: boolean },
): { usable: boolean; reason: "coming" | "needs-target" | "single-only" | "remote-only" | null } {
  if (command.notYet) return { usable: false, reason: "coming" };
  if (command.remoteOnly && !options.remote) return { usable: false, reason: "remote-only" };
  if (command.awayOnly && !options.away) return { usable: false, reason: "remote-only" };
  if (command.needsTarget && options.targets.length === 0) {
    return { usable: false, reason: "needs-target" };
  }
  if (command.singleOnly && options.targets.length > 1) {
    return { usable: false, reason: "single-only" };
  }
  return { usable: true, reason: null };
}

/** The commands a right-click menu shows for this pane. */
export function menuFor(options: {
  remote: boolean;
  targets: DirEntry[];
  away?: boolean;
}): Command[] {
  return COMMANDS.filter(
    (command) =>
      (!command.remoteOnly || options.remote) &&
      // Hidden rather than greyed out where it can never apply. A permanently
      // disabled entry is a question the program keeps asking and answering
      // itself.
      (!command.awayOnly || options.away === true),
  );
}
