/**
 * Bridge implementation for the container build (milestone M7): the same core,
 * reached over HTTP and WebSocket instead of over Tauri's channel.
 *
 * The service does not exist yet. What exists is this file, so that the
 * frontend can be built against it from the first day and nobody is tempted to
 * reach for Tauri from a component.
 */

import type { AmberBeamApi, CoreInfo } from "./types";

const base = import.meta.env.VITE_AMBERBEAM_SERVER ?? "";

async function get<T>(path: string): Promise<T> {
  const response = await fetch(`${base}${path}`, {
    headers: { accept: "application/json" },
    credentials: "same-origin",
  });
  if (!response.ok) {
    throw new Error(`${path} answered ${response.status}`);
  }
  return (await response.json()) as T;
}

export const api: AmberBeamApi = {
  shell: "web",
  coreInfo: () => get<CoreInfo>("/api/core-info"),
};
