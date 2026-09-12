<script lang="ts">
  /**
   * Whose core this window is driving.
   *
   * This machine, or a service running somewhere else — the same window, the
   * same keys, but the transfers happen over there and carry on after the
   * laptop is shut. That is the whole reason the second seam exists, and this
   * is the one place a person decides it.
   *
   * The password is traded for a token before anything switches. Switching
   * first and finding out afterwards would mean a window showing nothing and
   * no way to tell a wrong password from an unreachable machine.
   */
  import { connectedTo, signIn } from "../bridge";
  import { t } from "../i18n/index.svelte";
  import { trap } from "./trap";

  interface Props {
    onuse: (address: string, token: string) => void;
    onhere: () => void;
    onclose: () => void;
  }

  let { onuse, onhere, onclose }: Props = $props();

  /** Remembered between openings; it is an address, not a secret. */
  const REMEMBERED = "amberbeam.service.address";

  let address = $state(read());
  let password = $state("");
  let busy = $state(false);
  let problem = $state<string | null>(null);

  function read(): string {
    try {
      return localStorage.getItem(REMEMBERED) ?? connectedTo() ?? "";
    } catch {
      // A window with no storage is not a window that cannot connect.
      return connectedTo() ?? "";
    }
  }

  /**
   * What somebody typed, as an address a fetch can use.
   *
   * Somebody types "hausserver:2122" and means a machine. Without a scheme
   * that is a relative path, and the attempt fails in a way that says nothing
   * about what went wrong.
   */
  function tidy(typed: string): string {
    const text = typed.trim().replace(/\/+$/, "");
    if (text === "") return "";
    return /^https?:\/\//i.test(text) ? text : `http://${text}`;
  }

  async function use(): Promise<void> {
    const where = tidy(address);
    if (where === "") return;
    busy = true;
    problem = null;

    const answer = await signIn(where, password);
    busy = false;
    if ("problem" in answer) {
      problem = answer.problem;
      return;
    }

    try {
      localStorage.setItem(REMEMBERED, where);
    } catch {
      // Not being able to remember the address is not a reason to refuse.
    }
    password = "";
    onuse(where, answer.token);
  }
</script>

<div class="backdrop" role="presentation">
  <div class="dialog" use:trap role="dialog" aria-modal="true" aria-label={t("service.title")}>
    <header>
      <h2>{t("service.title")}</h2>
      <button
        type="button"
        class="close"
        onclick={onclose}
        title={t("action.close")}
        aria-label={t("action.close")}
      >×</button>
    </header>

    <div class="body">
      <button type="button" class="choice" class:active={connectedTo() === null} onclick={onhere}>
        <span class="name">{t("service.here")}</span>
        <span class="what">{t("service.here.what")}</span>
      </button>

      <form
        class="choice form"
        class:active={connectedTo() !== null}
        onsubmit={(event) => (event.preventDefault(), void use())}
      >
        <span class="name">{t("service.there")}</span>
        <span class="what">{t("service.there.what")}</span>

        <label>
          <span>{t("service.address")}</span>
          <input
            bind:value={address}
            placeholder="hausserver:2122"
            spellcheck="false"
            autocomplete="off"
            autocapitalize="off"
          />
        </label>
        <label>
          <span>{t("service.password")}</span>
          <input type="password" bind:value={password} autocomplete="off" />
        </label>

        {#if problem}
          <p class="bad">{t(`service.problem.${problem}`)}</p>
        {/if}

        <div class="actions">
          <button type="submit" class="go" disabled={busy || address.trim() === ""}>
            {busy ? t("service.connecting") : t("service.connect")}
          </button>
        </div>
      </form>

      <p class="hint">{t("service.hint")}</p>
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgb(0 0 0 / 45%);
    display: grid;
    place-items: center;
    z-index: 48;
  }

  .dialog {
    width: min(460px, 92vw);
    background: var(--surface-1);
    border: 1px solid var(--border-strong);
    border-radius: 1rem;
    box-shadow: var(--shadow-lg);
    outline: none;
  }

  header {
    display: flex;
    align-items: center;
    padding: 14px 18px;
    border-bottom: 1px solid var(--border);
  }

  h2 {
    margin: 0;
    flex: 1;
    font-size: 1rem;
  }

  .close {
    border: none;
    background: none;
    font-size: 1.2rem;
    color: var(--text-faint);
    cursor: default;
    padding: 0 6px;
  }

  .body {
    padding: 14px 18px 18px;
  }

  .choice {
    display: flex;
    flex-direction: column;
    gap: 2px;
    width: 100%;
    text-align: left;
    border: 1px solid var(--border);
    border-radius: 0.6rem;
    padding: 10px 12px;
    background: transparent;
    color: var(--text);
    font: inherit;
    cursor: pointer;
  }

  .choice + .choice {
    margin-top: 10px;
  }

  .choice.active {
    border-color: var(--accent);
    background: var(--accent-soft);
  }

  .choice.form {
    cursor: default;
  }

  .name {
    font-size: 0.86rem;
    font-weight: 600;
  }

  .what {
    font-size: 0.74rem;
    color: var(--text-muted);
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 3px;
    margin-top: 10px;
    font-size: 0.74rem;
    color: var(--text-muted);
  }

  input {
    font: inherit;
    font-size: 0.82rem;
    padding: 5px 8px;
    border: 1px solid var(--border);
    border-radius: 0.45rem;
    background: var(--surface-0);
    color: var(--text);
  }

  input:focus {
    border-color: var(--accent);
    outline: none;
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    margin-top: 12px;
  }

  .go {
    border: 1px solid var(--accent);
    border-radius: 0.5rem;
    padding: 5px 14px;
    background: transparent;
    color: var(--accent);
    font: inherit;
    font-size: 0.8rem;
    font-weight: 600;
    cursor: pointer;
  }

  .go:disabled {
    border-color: var(--border);
    color: var(--text-faint);
    cursor: default;
  }

  .bad {
    margin: 10px 0 0;
    font-size: 0.76rem;
    color: var(--danger);
  }

  .hint {
    margin: 14px 0 0;
    font-size: 0.72rem;
    color: var(--text-faint);
  }
</style>
