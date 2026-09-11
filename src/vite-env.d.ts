/// <reference types="svelte" />
/// <reference types="vite/client" />

// `@bridge-impl` is not declared here on purpose: `tsconfig.json` maps it onto
// the desktop implementation, which gives real types instead of an ambient
// stand-in. Which file it resolves to at build time is decided in
// `vite.config.ts`.

interface ImportMetaEnv {
  /** Base address of the AmberBeam service in the container build (M7). */
  readonly VITE_AMBERBEAM_SERVER?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
