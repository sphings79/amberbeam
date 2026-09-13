<script lang="ts">
  /**
   * A name, asked for in a dialog of this program's own.
   *
   * Not out of taste: `window.prompt` is answered with nothing at all inside
   * the desktop webview — it has no panel for it and returns null the moment
   * it is called — so every command built on one silently did nothing. A
   * folder that refuses to be created without a word is a worse bug than an
   * extra box on screen.
   */
  import { untrack } from "svelte";
  import { t } from "../i18n/index.svelte";
  import { trap } from "./trap";

  interface Props {
    /** What is being asked for, as a sentence. */
    title: string;
    /** What the field starts with, for a rename or a second attempt. */
    value?: string;
    /** The word on the button that does it. */
    action?: string;
    onname: (name: string) => void;
    oncancel: () => void;
  }

  let { title, value = "", action, onname, oncancel }: Props = $props();

  /* The starting value, once. The dialog is thrown away and made again for
     each question, so there is nothing to follow afterwards. */
  let typed = $state(untrack(() => value));
</script>

<div class="backdrop" role="presentation">
  <div class="dialog" use:trap role="dialog" aria-modal="true" aria-label={title}>
    <form
      onsubmit={(event) => {
        event.preventDefault();
        const name = typed.trim();
        if (name) onname(name);
      }}
    >
      <h2>{title}</h2>
      <!-- svelte-ignore a11y_autofocus -->
      <input
        bind:value={typed}
        autofocus
        autocomplete="off"
        autocapitalize="off"
        autocorrect="off"
        spellcheck="false"
        onkeydown={(event) => {
          event.stopPropagation();
          if (event.key === "Escape") oncancel();
        }}
      />
      <div class="actions">
        <button type="button" onclick={oncancel}>{t("action.cancel")}</button>
        <button type="submit" class="primary" disabled={typed.trim() === ""}>
          {action ?? t("action.save")}
        </button>
      </div>
    </form>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgb(0 0 0 / 45%);
    display: grid;
    place-items: center;
    z-index: 50;
  }

  .dialog {
    width: min(420px, 92vw);
    background: var(--surface-1);
    border: 1px solid var(--border-strong);
    border-radius: 1rem;
    box-shadow: var(--shadow-lg);
    padding: 20px 22px;
  }

  h2 {
    margin: 0 0 8px;
    font-size: 1rem;
  }

  input {
    width: 100%;
    box-sizing: border-box;
    font: inherit;
    font-size: 0.88rem;
    padding: 7px 9px;
    margin-bottom: 14px;
    border: 1px solid var(--border-strong);
    border-radius: 0.5rem;
    background: var(--surface-2);
    color: var(--text);
  }

  input:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-ring);
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }

  button {
    font: inherit;
    font-size: 0.84rem;
    padding: 6px 12px;
    border: 1px solid var(--border-strong);
    border-radius: 0.5rem;
    background: var(--surface-2);
    color: var(--text);
    cursor: default;
  }

  button.primary {
    border-color: var(--accent);
    background: var(--accent);
    color: var(--accent-text);
  }

  button:disabled {
    opacity: 0.5;
  }
</style>
