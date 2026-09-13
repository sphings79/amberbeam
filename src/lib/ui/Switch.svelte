<script lang="ts">
  /**
   * One setting, on or off, as a switch rather than a tick.
   *
   * A tick means "this one is selected"; a switch means "this is how it is
   * set". The two look alike in a list and are not the same thing, which is
   * why the rows somebody picks from -- a comparison, an import, the bits of
   * a permission -- keep their ticks and only settings get this.
   *
   * A button with `role="switch"`, not a checkbox with a drawing over it: the
   * button already answers to the space bar and to Enter, already takes focus,
   * and a reader announces it as the thing it is.
   */
  interface Props {
    checked: boolean;
    label: string;
    /** The sentence under it, where there is one. */
    hint?: string | null;
    disabled?: boolean;
    /** Called with the new state. The parent may also bind `checked`. */
    onchange?: (value: boolean) => void;
  }

  let {
    checked = $bindable(false),
    label,
    hint = null,
    disabled = false,
    onchange = undefined,
  }: Props = $props();

  function toggle(): void {
    if (disabled) return;
    checked = !checked;
    onchange?.(checked);
  }
</script>

<button
  type="button"
  class="switch"
  role="switch"
  aria-checked={checked}
  {disabled}
  onclick={toggle}
>
  <span class="track" class:on={checked}>
    <span class="knob"></span>
  </span>
  <span class="words">
    <span class="label">{label}</span>
    {#if hint}<span class="hint">{hint}</span>{/if}
  </span>
</button>

<style>
  .switch {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    width: 100%;
    padding: 4px 2px;
    border: none;
    background: none;
    text-align: left;
    color: inherit;
    font: inherit;
    cursor: default;
    border-radius: 8px;
  }

  .switch:disabled {
    opacity: 0.5;
  }

  .switch:not(:disabled):hover {
    background: var(--surface-2);
  }

  .switch:focus-visible {
    outline: 2px solid var(--accent-ring);
    outline-offset: 1px;
  }

  .track {
    /* The shape everybody already knows, at the size it is everywhere else:
       a 36 by 20 track with a 16 wide knob and two points of air around it. */
    flex: none;
    width: 36px;
    height: 20px;
    margin-top: 1px;
    border-radius: 999px;
    background: var(--border-strong);
    padding: 2px;
    display: flex;
    transition: background 120ms ease;
  }

  .track.on {
    background: var(--accent);
  }

  .knob {
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: #ffffff;
    box-shadow: 0 1px 2px rgb(0 0 0 / 25%);
    transition: transform 120ms ease;
  }

  .track.on .knob {
    transform: translateX(16px);
  }

  @media (prefers-reduced-motion: reduce) {
    .track,
    .knob {
      transition: none;
    }
  }

  .words {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .label {
    font-size: 0.84rem;
  }

  .hint {
    font-size: 0.76rem;
    color: var(--text-faint);
    line-height: 1.45;
  }
</style>
