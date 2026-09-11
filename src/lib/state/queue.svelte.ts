/**
 * The transfer queue, as the window sees it.
 *
 * The core owns the queue; this is a copy that follows it. Two kinds of update
 * arrive: a full snapshot whenever the list changes, and progress on a timer
 * for the jobs that are moving. Redrawing the whole list sixty times a second
 * for a progress bar would be wasteful, and asking the core for a snapshot on
 * every chunk more so.
 */

import { api, type CoreEvent, type JobProgress, type Queue, type Totals } from "../bridge";

let queue = $state<Queue>({ jobs: [], paused: false });
let totals = $state<Totals>({
  waitingJobs: 0,
  askingJobs: 0,
  runningJobs: 0,
  pausedJobs: 0,
  doneJobs: 0,
  failedJobs: 0,
  doneBytes: 0,
  totalBytes: 0,
});
/** Live figures per running job, by id. */
let live = $state<Record<string, JobProgress>>({});

export function queueState(): Queue {
  return queue;
}

export function queueTotals(): Totals {
  return totals;
}

export function jobProgress(id: string): JobProgress | undefined {
  return live[id];
}

export async function refreshQueue(): Promise<void> {
  const [snapshot, sums] = await Promise.all([api.queueSnapshot(), api.queueTotals()]);
  queue = snapshot;
  totals = sums;
  // Anything no longer running has no live figure; keeping the last one would
  // leave a finished job showing a rate.
  const running = new Set(snapshot.jobs.filter((job) => job.state === "running").map((j) => j.id));
  live = Object.fromEntries(Object.entries(live).filter(([id]) => running.has(id)));
}

/** Takes what belongs to the queue out of the core's event stream. */
export function recordQueueEvent(event: CoreEvent): void {
  if (event.event === "queue") {
    void refreshQueue();
    return;
  }
  if (event.event === "progress") {
    const next: Record<string, JobProgress> = {};
    for (const job of event.jobs) {
      next[job.id] = job;
    }
    live = next;
  }
}

/** The percentage a row shows, preferring the live figure over the snapshot. */
export function percentOf(id: string, doneBytes: number, totalBytes: number | null): number | null {
  const current = live[id];
  const done = current?.doneBytes ?? doneBytes;
  const total = current?.totalBytes ?? totalBytes;
  if (!total || total === 0) return null;
  return Math.min(100, Math.round((done / total) * 100));
}
