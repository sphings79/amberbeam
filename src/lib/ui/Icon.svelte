<script lang="ts">
  /**
   * The toolbar icons, drawn rather than typed.
   *
   * Symbols like U+1F5C0 (folder) have no coverage in the system font and turn
   * into empty boxes, and the ones that do render are emoji — colourful, and
   * out of place next to monochrome text. Sixteen pixels of path each solves
   * both, and they follow the accent colour like everything else.
   */
  interface Props {
    name: string;
    size?: number;
  }

  let { name, size = 15 }: Props = $props();

  const PATHS: Record<string, string[]> = {
    reload: ["M13.5 8a5.5 5.5 0 1 1-1.6-3.9", "M13.5 2v3h-3"],
    "new-folder": ["M2 12.5v-9A1 1 0 0 1 3 2.5h3l1.6 2H13a1 1 0 0 1 1 1v7a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1z", "M8 7.5v4M6 9.5h4"],
    "new-file": ["M4 1.5h5l3 3v10a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1v-12a1 1 0 0 1 1-1z", "M9 1.5v3h3", "M7.5 8v4M5.5 10h4"],
    rename: ["M2 11.5 10.5 3l2.5 2.5L4.5 14H2z", "M9 4.5 11.5 7"],
    permissions: ["M8 1.5 3 3.5v4.2c0 3.1 2.1 5.6 5 6.8 2.9-1.2 5-3.7 5-6.8V3.5z", "M6 8l1.6 1.6L10.5 6.5"],
    delete: ["M3 4.5h10", "M6.5 4.5V3a1 1 0 0 1 1-1h1a1 1 0 0 1 1 1v1.5", "M4.5 4.5l.7 9a1 1 0 0 0 1 .9h3.6a1 1 0 0 0 1-.9l.7-9"],
    transfer: ["M2.5 6h9L9 3.5", "M13.5 10h-9L7 12.5"],
    "edit-remote": ["M8.5 2.5H3a1 1 0 0 0-1 1v9a1 1 0 0 0 1 1h9a1 1 0 0 0 1-1V7", "M6 10.5 13 3.5l1.5 1.5L7.5 12H6z"],
    folder: ["M2 12.5v-9A1 1 0 0 1 3 2.5h3l1.6 2H13a1 1 0 0 1 1 1v7a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1z"],
    // A page with the corner turned down, which is what a file has looked like
    // since before any of this. The fold is its own path so it reads at 13px.
    file: ["M4 1.5h5l3 3v10a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1v-12a1 1 0 0 1 1-1z", "M9 1.5v3h3"],
    // The same page, with the arrow that says it points somewhere else.
    symlink: [
      "M4 1.5h5l3 3v10a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1v-12a1 1 0 0 1 1-1z",
      "M9 1.5v3h3",
      "M5.5 11.5 10 7M7 7h3v3",
    ],
    sites: ["M2.5 2.5h11v4h-11zM2.5 9.5h11v4h-11z", "M4.5 4.5h.01M4.5 11.5h.01"],
    tree: ["M2 3.5h4v3H2zM10 2.5h4v3h-4zM10 10.5h4v3h-4z", "M4 6.5v5h6M4 4.5h6"],
    hidden: ["M1.5 8S4 3.5 8 3.5 14.5 8 14.5 8 12 12.5 8 12.5 1.5 8 1.5 8z", "M8 6a2 2 0 1 0 0 4 2 2 0 0 0 0-4z"],
    "hidden-off": ["M2 2l12 12", "M6.2 6.3A2 2 0 0 0 8 10c.5 0 1-.2 1.4-.5", "M4.2 4.6C2.6 5.9 1.5 8 1.5 8S4 12.5 8 12.5c1 0 1.9-.3 2.7-.7", "M11.6 11a10 10 0 0 0 2.9-3S12 3.5 8 3.5c-.5 0-1 .1-1.5.2"],
    connect: ["M8 3v10M3 8h10"],
    star: ["M8 1.8l1.9 3.9 4.3.6-3.1 3 .7 4.3L8 11.6l-3.8 2 .7-4.3-3.1-3 4.3-.6z"],
    coffee: [
      "M2.5 5.5h9v4a3.5 3.5 0 0 1-3.5 3.5h-2A3.5 3.5 0 0 1 2.5 9.5z",
      "M11.5 6.5h1.2a1.8 1.8 0 0 1 0 3.6h-1.2",
      "M5 1.5c0 1-.8 1-.8 2M8 1.5c0 1-.8 1-.8 2",
    ],
    disconnect: ["M8 2v6", "M4.6 4.6a5 5 0 1 0 6.8 0"],
    // Up, not a star: a star is for marking a favourite, and this is about
    // something being newer than what is installed.
    update: ["M8 13V3", "M4 7l4-4 4 4"],
    // Two paths side by side, stepping together.
    together: ["M4 2.5v11", "M12 2.5v11", "M2 6h4M2 10h4", "M10 6h4M10 10h4"],
  };
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 16 16"
  fill="none"
  stroke="currentColor"
  stroke-width="1.4"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  {#each PATHS[name] ?? [] as path (path)}
    <path d={path} />
  {/each}
</svg>

<style>
  svg {
    display: block;
    flex: none;
  }
</style>
