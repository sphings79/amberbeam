<script lang="ts">
  /**
   * How the program looks, and where its regions sit.
   *
   * This used to unfold as two rows along the bottom of the window, under the
   * footer: four groups of buttons in a strip, each labelled by a word to its
   * left, with nothing to separate one group from the next. It was legible
   * only to somebody who already knew what was in it.
   *
   * A dialog rather than a window of its own. The server list earns a window
   * because it is a place to work in beside the panes; choosing a colour is
   * not — it is a decision, taken once, in front of the thing it changes.
   */
  import { t, locale, LOCALES, setLocale, type Locale } from "../i18n/index.svelte";
  import {
    ACCENTS,
    currentAccent,
    currentSize,
    currentTheme,
    setAccent,
    setSize,
    setTheme,
    SIZES,
    THEMES,
  } from "../theme/index.svelte";
  import { trap } from "./trap";

  type Where = "top" | "bottom" | "off";

  interface Props {
    logWhere: Where;
    queueWhere: Where;
    onlog: (where: Where) => void;
    onqueue: (where: Where) => void;
    onreset: () => void;
    onclose: () => void;
  }

  let { logWhere, queueWhere, onlog, onqueue, onreset, onclose }: Props = $props();

  /**
   * Asked twice, without a dialog on top of a dialog.
   *
   * Resetting throws away every deliberate choice somebody has made about the
   * window, and four splitters dragged back into place is a real cost. A
   * second click is enough of a pause; a second window to click through would
   * be more ceremony than the thing deserves.
   */
  let sure = $state(false);
  let asked: number | null = null;

  function reset(): void {
    if (!sure) {
      sure = true;
      if (asked !== null) clearTimeout(asked);
      // Long enough to read the question, short enough that a stray click
      // minutes later is not taken as the answer to it.
      asked = window.setTimeout(() => (sure = false), 5000);
      return;
    }
    if (asked !== null) clearTimeout(asked);
    sure = false;
    onreset();
  }

  /**
   * Off last, after the two places.
   *
   * Putting it away is a different kind of answer from where to put it, and it
   * belongs at the end of the row rather than in the middle of the choice.
   */
  const PLACES: Where[] = ["top", "bottom", "off"];
</script>

<svelte:window onkeydown={(event) => event.key === "Escape" && onclose()} />

<div class="backdrop" role="presentation">
  <div
    class="dialog"
    use:trap
    role="dialog"
    aria-modal="true"
    aria-label={t("appearance.title")}
  >
    <header>
      <h2>{t("appearance.title")}</h2>
      <button
        type="button"
        class="close"
        onclick={onclose}
        title={t("action.close")}
        aria-label={t("action.close")}
      >×</button>
    </header>

    <div class="body">
      <div class="row">
        <span class="label">{t("appearance.theme")}</span>
        <div class="choice">
          {#each THEMES as candidate (candidate)}
            <button
              type="button"
              class:active={currentTheme() === candidate}
              onclick={() => setTheme(candidate)}
            >
              {t(`theme.${candidate}`)}
            </button>
          {/each}
        </div>
      </div>

      <div class="row">
        <span class="label">{t("appearance.accent")}</span>
        <div class="choice">
          {#each ACCENTS as candidate (candidate)}
            <button
              type="button"
              class="swatch"
              class:active={currentAccent() === candidate}
              data-accent={candidate}
              title={t(`accent.${candidate}`)}
              aria-label={t(`accent.${candidate}`)}
              onclick={() => setAccent(candidate)}
            ></button>
          {/each}
        </div>
      </div>

      <div class="row">
        <span class="label">{t("appearance.size")}</span>
        <div class="choice">
          {#each SIZES as candidate (candidate)}
            <button
              type="button"
              class:active={currentSize() === candidate}
              onclick={() => setSize(candidate)}
              style:font-size="{0.62 + candidate * 0.2}rem"
            >
              {t(`size.${String(candidate).replace(".", "-")}`)}
            </button>
          {/each}
        </div>
      </div>

      <div class="row">
        <span class="label">{t("language.title")}</span>
        <div class="choice">
          {#each LOCALES as candidate (candidate)}
            <button
              type="button"
              class:active={locale() === candidate}
              onclick={() => setLocale(candidate as Locale)}
            >
              {t(`language.${candidate}`)}
            </button>
          {/each}
        </div>
      </div>

      <hr />

      <div class="row">
        <span class="label">{t("layout.log")}</span>
        <div class="choice">
          {#each PLACES as where (where)}
            <button type="button" class:active={logWhere === where} onclick={() => onlog(where)}>
              {t(`layout.${where}`)}
            </button>
          {/each}
        </div>
      </div>

      <div class="row">
        <span class="label">{t("layout.queue")}</span>
        <div class="choice">
          {#each PLACES as where (where)}
            <button
              type="button"
              class:active={queueWhere === where}
              onclick={() => onqueue(where)}
            >
              {t(`layout.${where}`)}
            </button>
          {/each}
        </div>
      </div>
      <p class="hint">{t("layout.hint")}</p>

      <hr />

      <div class="row">
        <span class="label">{t("appearance.reset")}</span>
        <div class="choice">
          <button type="button" class:asking={sure} onclick={reset}>
            {sure ? t("appearance.reset.sure") : t("appearance.reset.do")}
          </button>
        </div>
      </div>
      <p class="hint">{t("appearance.reset.hint")}</p>
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
    z-index: 47;
  }

  .dialog {
    width: min(440px, 92vw);
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
    padding: 6px 18px 18px;
  }

  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-top: 12px;
  }

  .label {
    font-size: 0.84rem;
    color: var(--text);
  }

  .choice {
    display: flex;
    align-items: center;
    gap: 5px;
  }

  .choice button {
    border: 1px solid var(--border);
    border-radius: 0.45rem;
    padding: 3px 10px;
    background: transparent;
    color: var(--text-muted);
    font: inherit;
    font-size: 0.78rem;
    cursor: pointer;
  }

  .choice button:hover {
    border-color: var(--border-strong);
    color: var(--text);
  }

  .choice button.asking {
    border-color: var(--danger);
    color: var(--danger);
  }

  .choice button.active {
    border-color: var(--accent);
    background: var(--accent-soft);
    color: var(--accent);
  }

  .choice .swatch {
    width: 18px;
    height: 18px;
    padding: 0;
    border-radius: 999px;
    background: var(--accent);
    border: 2px solid transparent;
    box-shadow: 0 0 0 1px var(--border-strong);
  }

  .choice .swatch.active {
    box-shadow: 0 0 0 2px var(--accent);
  }

  hr {
    margin: 16px 0 0;
    border: none;
    border-top: 1px solid var(--border);
  }

  .hint {
    margin: 6px 0 0;
    font-size: 0.74rem;
    color: var(--text-faint);
  }
</style>
