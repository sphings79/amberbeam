/**
 * The whole surface the user interface has towards the core.
 *
 * Seam 2 (concept paper, section 09): every implementation of `AmberBeamApi`
 * has to satisfy this interface — the one going through Tauri's channel on the
 * desktop, and the one going through HTTP and WebSocket in the container build
 * of milestone M7. Nothing outside `src/lib/bridge/` may know which of them is
 * in use; `scripts/check-seams.mjs` enforces that.
 */

export type Protocol = "local" | "sftp" | "ftp" | "ftps";

export interface ProtocolInfo {
  protocol: Protocol;
  defaultPort: number | null;
  defaultConcurrency: number;
  encrypted: boolean;
}

export interface CoreInfo {
  version: string;
  arch: string;
  os: string;
  protocols: ProtocolInfo[];
}

/** Which shell the frontend is running in. */
export type Shell = "desktop" | "web";

export interface AmberBeamApi {
  readonly shell: Shell;
  /** Version and capabilities of the core behind this window. */
  coreInfo(): Promise<CoreInfo>;
}
