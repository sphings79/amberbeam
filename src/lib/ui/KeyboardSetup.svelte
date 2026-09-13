<script lang="ts">
  /**
   * The first start, and the largest single obstacle this program has.
   *
   * On a Mac, F1 to F12 are not function keys. Pressing F5 raises the
   * brightness and never reaches the program at all; and once that is settled,
   * the system still keeps F3, F4 and F11 for Mission Control, Spotlight and
   * showing the desktop.
   *
   * Two hurdles, and they fall in that order. This dialog finds out which of
   * them is still standing — by asking for a key and seeing whether it arrives,
   * which is the only test that means anything — and then offers three ways
   * forward rather than insisting on one.
   *
   * What it never does is reach below the operating system to catch the keys
   * anyway. That would need the accessibility permission, break with every
   * macOS update, and make the program look like something that reads
   * keystrokes. The price is higher than the gain.
   */
  import { api } from "../bridge";
  import { t } from "../i18n/index.svelte";
  import { trap } from "./trap";
  import {
    label,
    SCHEMES,
    schemeBindings,
    setScheme,
    type Action,
    type SchemeName,
  } from "../keys/index.svelte";

  interface Props {
    onclose: () => void;
  }

  let { onclose }: Props = $props();

  /**
   * Whether the two links have anything to open.
   *
   * In a browser they have not: the settings being described belong to the
   * machine somebody is typing on, and this shell runs on a different one. The
   * sentences above them still hold — the drawing shows where to look — so the
   * page keeps its explanation and loses the button that would do nothing.
   */
  const canOpenSettings = api.shell === "desktop";

  /**
   * What the test found for each key: it arrived, or it did not.
   *
   * There is no mode to start and no clock to run out. The dialog simply
   * listens the whole time it is open, so pressing the key is the test — and
   * the other answer, that nothing happened, comes from the only party who can
   * actually know it. A timer guessing "six seconds, so probably not" would be
   * wrong for anybody who paused to read.
   */
  let arrived = $state<Record<string, boolean>>({});

  function heard(event: KeyboardEvent): void {
    if (event.key !== "F5" && event.key !== "F3") return;
    event.preventDefault();
    arrived = { ...arrived, [event.key]: true };
  }

  function nothingHappened(key: string): void {
    arrived = { ...arrived, [key]: false };
  }

  function choose(name: SchemeName): void {
    setScheme(name);
    onclose();
  }

  /**
   * Three keys out of a scheme, to show what choosing it feels like.
   *
   * Refreshing, switching sides and searching: the three somebody reaches for
   * most, and the three the schemes disagree about most.
   */
  const SHOWN: Action[] = ["refresh", "switch-focus", "search"];

  function sample(name: SchemeName): string {
    const bindings = schemeBindings(name);
    return SHOWN.map((action) => label(bindings[action]?.[0] ?? "")).join(" · ");
  }

  /**
   * What each scheme really costs.
   *
   * Only one thing is ever *required*, and only for the Windows layout: the
   * three keys the system keeps for itself have to be given up, or F3, F4 and
   * F11 never arrive however they are pressed.
   *
   * Turning the function keys on is not required at all. Without it they still
   * work — hold fn. The setting saves holding fn, which is a convenience and
   * not a hurdle, and presenting it as one overstates what this costs.
   */
  const COST: Record<SchemeName, { required: number; fnWorks: boolean }> = {
    classic: { required: 1, fnWorks: true },
    mixed: { required: 0, fnWorks: true },
    mac: { required: 0, fnWorks: false },
  };
</script>

<svelte:window onkeydown={heard} />

<div class="backdrop" role="presentation">
  <div class="dialog" use:trap role="dialog" aria-modal="true" aria-label={t("setup.title")}>
    <header>
      <h2>{t("setup.title")}</h2>
    </header>

    <p>{t("setup.body")}</p>

    <!-- The test first. Everything below depends on its answer, and reading
         three paragraphs about a problem one may not have is wasted. -->
    <section class="test">
      <div class="line">
        <span class="ask">{t("setup.test.try", { key: "F5" })}</span>
        {#if arrived.F5 === true}
          <span class="good">{t("setup.test.arrived")}</span>
        {:else if arrived.F5 === false}
          <span class="bad">{t("setup.test.missing")}</span>
        {:else}
          <button type="button" onclick={() => nothingHappened("F5")}>
            {t("setup.test.nothing")}
          </button>
        {/if}
      </div>

      {#if arrived.F5 === false}
        <p class="hint">{t("setup.hurdle.one")}</p>
        {#if canOpenSettings}
          <button type="button" class="link" onclick={() => void api.openSystemKeyboard("function-keys")}>
            {t("setup.open.function-keys")}
          </button>
        {/if}
        {@render functionKeyDrawing()}
      {:else if arrived.F5 === true}
        <p class="hint">{t("setup.hurdle.one.done")}</p>
        <div class="line">
          <span class="ask">{t("setup.test.try", { key: "F3" })}</span>
          {#if arrived.F3 === true}
            <span class="good">{t("setup.test.arrived")}</span>
          {:else if arrived.F3 === false}
            <span class="bad">{t("setup.test.taken")}</span>
          {:else}
            <button type="button" onclick={() => nothingHappened("F3")}>
              {t("setup.test.mission-control")}
            </button>
          {/if}
        </div>
        {#if arrived.F3 === false}
          <p class="hint">{t("setup.hurdle.two")}</p>
          {#if canOpenSettings}
            <button type="button" class="link" onclick={() => void api.openSystemKeyboard("shortcuts")}>
              {t("setup.open.shortcuts")}
            </button>
          {/if}
          {@render shortcutDrawing()}
        {/if}
      {/if}
    </section>

    <h3>{t("setup.choose")}</h3>
    <div class="schemes">
      {#each SCHEMES as name (name)}
        <button type="button" class="scheme" onclick={() => choose(name)}>
          <span class="name">{t(`scheme.${name}`)}</span>
          <span class="what">{t(`scheme.${name}.what`)}</span>
          <!-- The last two lines sit at the bottom of every card, whatever the
               description above them runs to. Three cards whose facts start at
               three different heights read as three different kinds of thing. -->
          <!-- Two lines of the same shape in all three cards: what the system
               needs, and how the function keys are reached. -->
          <span class="cost">
            {COST[name].required === 1 ? t("setup.cost.settings.one") : t("setup.cost.none")}<br />
            <span class="fn">
              {COST[name].fnWorks ? t("setup.cost.fn") : t("setup.cost.no-f-keys")}
            </span>
          </span>
          <!-- This scheme's own keys, not a fixed example. Three cards showing
               the same three keys would say nothing about the choice. -->
          <span class="sample mono">{sample(name)}</span>
        </button>
      {/each}
    </div>

    <p class="hint">{t("setup.later")}</p>

    <div class="actions">
      <span class="spacer"></span>
      <button type="button" onclick={onclose}>{t("setup.skip")}</button>
    </div>
  </div>
</div>

<!--
  Drawn rather than photographed. A screenshot of System Settings is wrong the
  day macOS moves a row, and a picture that is confidently wrong is worse than
  none: somebody looks for what it shows and concludes the program is broken.
  A sketch says what to look for and stays true.
-->
{#snippet functionKeyDrawing()}
  <svg class="drawing" viewBox="0 0 320 92" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
    <rect x="1" y="1" width="318" height="90" rx="8" class="frame" />
    <text x="14" y="24" class="caption">{t("setup.drawing.keyboard")}</text>
    <line x1="14" y1="34" x2="306" y2="34" class="rule" />
    <text x="14" y="56" class="row">{t("setup.drawing.fn-row")}</text>
    <rect x="272" y="44" width="30" height="16" rx="8" class="switch" />
    <circle cx="294" cy="52" r="6" class="knob" />
    <text x="14" y="78" class="quiet">{t("setup.drawing.fn-note")}</text>
  </svg>
{/snippet}

{#snippet shortcutDrawing()}
  <svg class="drawing" viewBox="0 0 320 104" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
    <rect x="1" y="1" width="318" height="102" rx="8" class="frame" />
    <text x="14" y="24" class="caption">{t("setup.drawing.shortcuts")}</text>
    <line x1="14" y1="34" x2="306" y2="34" class="rule" />
    {#each [["F3", 52], ["F4", 72], ["F11", 92]] as row (row[0])}
      <rect x="14" y={Number(row[1]) - 11} width="12" height="12" rx="3" class="box" />
      <text x="34" y={row[1]} class="row">{t(`setup.drawing.key-${row[0]}`)}</text>
      <text x="276" y={row[1]} class="mono-small">{row[0]}</text>
    {/each}
  </svg>
{/snippet}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgb(0 0 0 / 55%);
    display: grid;
    place-items: center;
    z-index: 70;
  }

  .dialog {
    width: min(620px, 94vw);
    max-height: 90vh;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 12px;
    background: var(--surface-1);
    border: 1px solid var(--border-strong);
    border-top: 3px solid var(--accent);
    border-radius: 1rem;
    box-shadow: var(--shadow-lg);
    padding: 20px 22px;
  }

  h2 {
    margin: 0;
    font-size: 1.05rem;
  }

  h3 {
    margin: 4px 0 0;
    font-size: 0.86rem;
    color: var(--text-muted);
  }

  p {
    margin: 0;
    font-size: 0.86rem;
    color: var(--text-muted);
  }

  .hint {
    font-size: 0.8rem;
    color: var(--text-faint);
  }

  .test {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px 14px;
    background: var(--surface-2);
    border-radius: 0.6rem;
  }

  .line {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .ask {
    font-size: 0.86rem;
    color: var(--text);
    flex: 1;
  }

  .good {
    color: var(--accent);
    font-size: 0.82rem;
  }

  .bad {
    color: var(--warn);
    font-size: 0.82rem;
  }

  .schemes {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(170px, 1fr));
    gap: 8px;
  }

  .scheme {
    display: flex;
    flex-direction: column;
    gap: 4px;
    text-align: left;
    font: inherit;
    padding: 12px 14px;
    border-radius: 0.7rem;
    border: 1px solid var(--border-strong);
    background: var(--surface-0);
    color: var(--text);
    cursor: default;
  }

  .scheme:hover {
    border-color: var(--accent);
    background: var(--surface-2);
  }

  .scheme .name {
    font-size: 0.9rem;
    font-weight: 600;
  }

  .scheme .what {
    font-size: 0.76rem;
    color: var(--text-muted);
    /* Takes whatever room is left, which is what pushes the two lines below
       it to the bottom edge. */
    flex: 1;
  }

  .scheme .cost {
    font-size: 0.76rem;
    color: var(--text-faint);
    line-height: 1.5;
  }

  .scheme .fn {
    color: var(--text-muted);
  }

  .scheme .sample {
    font-size: 0.72rem;
    color: var(--accent);
  }

  .drawing {
    width: 100%;
    max-width: 340px;
  }

  .drawing .frame {
    fill: var(--surface-0);
    stroke: var(--border-strong);
  }

  .drawing .rule {
    stroke: var(--border);
  }

  .drawing .caption {
    fill: var(--text-muted);
    font-size: 0.69rem;
    font-weight: 600;
  }

  .drawing .row {
    fill: var(--text);
    font-size: 0.69rem;
  }

  .drawing .quiet {
    fill: var(--text-faint);
    font-size: 0.63rem;
  }

  .drawing .mono-small {
    fill: var(--text-faint);
    font-size: 0.63rem;
    font-family: ui-monospace, monospace;
  }

  .drawing .switch {
    fill: var(--accent-soft);
    stroke: var(--accent);
  }

  .drawing .knob {
    fill: var(--accent);
  }

  .drawing .box {
    fill: none;
    stroke: var(--text-faint);
  }

  button {
    font: inherit;
    font-size: 0.84rem;
    padding: 5px 14px;
    border-radius: 999px;
    border: 1px solid var(--border-strong);
    background: var(--surface-1);
    color: var(--text);
    cursor: default;
  }

  button:hover {
    background: var(--surface-2);
  }

  button:disabled {
    opacity: 0.6;
  }

  button.link {
    align-self: flex-start;
    border-color: var(--accent);
    background: var(--accent-soft);
    color: var(--accent);
  }

  .actions {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .spacer {
    flex: 1;
  }
</style>
