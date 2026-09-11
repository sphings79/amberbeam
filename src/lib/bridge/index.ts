/**
 * The only door between user interface and core.
 *
 * `@bridge-impl` is resolved by `vite.config.ts` and points at `tauri.ts` or
 * `http.ts`, depending on which shell is being built.
 */

import { api } from "@bridge-impl";

export { api };
export type * from "./types";
