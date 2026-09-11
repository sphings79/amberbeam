import { svelte } from "@sveltejs/vite-plugin-svelte";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";

// Seam 2 (concept paper, section 09): the user interface never talks to Tauri
// directly. It imports `@bridge-impl`, and this is the one place that decides
// which implementation that name resolves to.
//
//   AMBERBEAM_SHELL=desktop  (default)  Tauri's internal channel
//   AMBERBEAM_SHELL=web                 HTTP and WebSocket, milestone M7
const implementations = {
  desktop: "./src/lib/bridge/tauri.ts",
  web: "./src/lib/bridge/http.ts",
} as const;

const shell = process.env.AMBERBEAM_SHELL ?? "desktop";
if (!(shell in implementations)) {
  throw new Error(
    `AMBERBEAM_SHELL must be one of ${Object.keys(implementations).join(", ")}, got "${shell}"`,
  );
}
const implementation = implementations[shell as keyof typeof implementations];

export default defineConfig({
  plugins: [svelte()],
  resolve: {
    alias: {
      "@bridge-impl": fileURLToPath(new URL(implementation, import.meta.url)),
    },
  },
  // Tauri expects a fixed port and shows Rust errors itself.
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**", "**/target/**"] },
  },
  build: {
    // Safari 16.4 ships with macOS 13, the oldest system AmberBeam supports.
    target: "safari16",
    sourcemap: process.env.NODE_ENV !== "production",
  },
});
