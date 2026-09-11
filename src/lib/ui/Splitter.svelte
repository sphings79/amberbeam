<script lang="ts">
  interface Props {
    /** "vertical" divides left from right, "horizontal" top from bottom. */
    direction: "vertical" | "horizontal";
    onmove: (delta: number) => void;
    label: string;
  }

  let { direction, onmove, label }: Props = $props();

  let dragging = $state(false);

  function start(event: PointerEvent): void {
    dragging = true;
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  }

  function move(event: PointerEvent): void {
    if (!dragging) return;
    onmove(direction === "vertical" ? event.movementX : event.movementY);
  }

  function stop(event: PointerEvent): void {
    dragging = false;
    (event.currentTarget as HTMLElement).releasePointerCapture(event.pointerId);
  }
</script>

<div
  class="splitter {direction}"
  class:dragging
  role="separator"
  aria-label={label}
  aria-orientation={direction === "vertical" ? "vertical" : "horizontal"}
  tabindex="-1"
  onpointerdown={start}
  onpointermove={move}
  onpointerup={stop}
  onpointercancel={stop}
></div>

<style>
  .splitter {
    flex: none;
    background: var(--border);
    transition: background 0.12s;
  }

  .splitter:hover,
  .splitter.dragging {
    background: var(--accent);
  }

  .vertical {
    width: 4px;
    cursor: col-resize;
  }

  .horizontal {
    height: 4px;
    cursor: row-resize;
  }
</style>
