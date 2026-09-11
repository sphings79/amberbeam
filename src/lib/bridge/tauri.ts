/** Bridge implementation for the desktop build: Tauri's internal channel. */

import { invoke } from "@tauri-apps/api/core";

import type { AmberBeamApi, CoreInfo } from "./types";

export const api: AmberBeamApi = {
  shell: "desktop",
  coreInfo: () => invoke<CoreInfo>("core_info"),
};
