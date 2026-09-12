<script lang="ts">
  /**
   * The password, for a page served by a service.
   *
   * The container build and nothing else. Everything behind this needs a
   * session, so there is nothing useful to show until there is one — it fills
   * the window rather than sitting over a program nobody can use yet.
   *
   * There is no address to ask for: it is wherever this page came from. And
   * nothing is kept here, because the answer is a cookie the browser holds.
   */
  import { signInHere } from "../bridge";
  import { t } from "../i18n/index.svelte";

  interface Props {
    onin: () => void;
  }

  let { onin }: Props = $props();

  let password = $state("");
  let busy = $state(false);
  let wrong = $state(false);
  let field = $state<HTMLInputElement | null>(null);

  $effect(() => {
    field?.focus();
  });

  async function go(): Promise<void> {
    busy = true;
    wrong = false;
    const ok = await signInHere(password);
    busy = false;
    if (!ok) {
      wrong = true;
      password = "";
      field?.focus();
      return;
    }
    password = "";
    onin();
  }
</script>

<div class="page">
  <form onsubmit={(event) => (event.preventDefault(), void go())}>
    <h1>AmberBeam</h1>
    <p class="what">{t("signin.what")}</p>

    <label>
      <span>{t("service.password")}</span>
      <!-- svelte-ignore a11y_autofocus -->
      <input type="password" bind:this={field} bind:value={password} autocomplete="current-password" />
    </label>

    {#if wrong}
      <p class="bad">{t("service.problem.refused")}</p>
    {/if}

    <button type="submit" disabled={busy || password === ""}>
      {busy ? t("signin.going") : t("signin.go")}
    </button>
  </form>
</div>

<style>
  .page {
    position: fixed;
    inset: 0;
    display: grid;
    place-items: center;
    background: var(--surface-0);
  }

  form {
    width: min(320px, 88vw);
    display: flex;
    flex-direction: column;
    background: var(--surface-1);
    border: 1px solid var(--border);
    border-radius: 1rem;
    padding: 24px;
    box-shadow: var(--shadow-lg);
  }

  h1 {
    margin: 0;
    font-size: 1.1rem;
    color: var(--accent);
  }

  .what {
    margin: 4px 0 0;
    font-size: 0.76rem;
    color: var(--text-muted);
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-top: 18px;
    font-size: 0.74rem;
    color: var(--text-muted);
  }

  input {
    font: inherit;
    font-size: 0.86rem;
    padding: 6px 9px;
    border: 1px solid var(--border);
    border-radius: 0.45rem;
    background: var(--surface-0);
    color: var(--text);
  }

  input:focus {
    border-color: var(--accent);
    outline: none;
  }

  .bad {
    margin: 10px 0 0;
    font-size: 0.76rem;
    color: var(--danger);
  }

  button {
    margin-top: 16px;
    border: 1px solid var(--accent);
    border-radius: 0.5rem;
    padding: 7px 14px;
    background: var(--accent-soft);
    color: var(--accent);
    font: inherit;
    font-size: 0.84rem;
    font-weight: 600;
    cursor: pointer;
  }

  button:disabled {
    border-color: var(--border);
    background: transparent;
    color: var(--text-faint);
    cursor: default;
  }
</style>
