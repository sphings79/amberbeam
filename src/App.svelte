<script lang="ts">
  import { api, type CoreInfo } from "./lib/bridge";
  import { LOCALES, locale, setLocale, t } from "./lib/i18n/index.svelte";
  import {
    ACCENTS,
    currentAccent,
    currentTheme,
    setAccent,
    setTheme,
    THEMES,
  } from "./lib/theme/index.svelte";

  let info = $state<CoreInfo | null>(null);
  let failure = $state<string | null>(null);

  api
    .coreInfo()
    .then((answer) => (info = answer))
    .catch((error: unknown) => (failure = error instanceof Error ? error.message : String(error)));

  const seams = [
    { key: "endpoints", number: "08" },
    { key: "bridge", number: "09" },
    { key: "i18n", number: "11" },
  ] as const;
</script>

<div class="shell">
  <header>
    <div class="mark" aria-hidden="true">
      <svg viewBox="0 0 256 256">
        <rect x="16" y="16" width="224" height="224" rx="56" fill="url(#markGradient)" />
        <defs>
          <linearGradient id="markGradient" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0" stop-color="var(--accent)" stop-opacity="0.95" />
            <stop offset="1" stop-color="var(--accent)" stop-opacity="0.7" />
          </linearGradient>
        </defs>
        <g fill="none" stroke="#fff" stroke-width="14" stroke-linecap="round" stroke-linejoin="round">
          <path d="M62 128h108" />
          <path d="M146 96l38 32-38 32" />
          <path d="M62 92h30M62 164h30" opacity="0.55" />
        </g>
      </svg>
    </div>
    <div class="title">
      <h1>Amber<span>Beam</span></h1>
      <p>{t("app.tagline")}</p>
    </div>
    <span class="chip accent mono">{t("status.badge")}</span>
  </header>

  <div class="beam" aria-hidden="true"></div>

  <main>
    <p class="lead">{t("status.explain")}</p>

    <section>
      <h2>{t("seams.title")}</h2>
      <p class="intro">{t("seams.intro")}</p>
      <div class="seams">
        {#each seams as seam (seam.key)}
          <article class="card">
            <span class="number mono">{seam.number}</span>
            <h3>{t(`seam.${seam.key}.title`)}</h3>
            <p>{t(`seam.${seam.key}.body`)}</p>
          </article>
        {/each}
      </div>
    </section>

    <section>
      <h2>{t("core.title")}</h2>
      {#if failure}
        <p class="card danger">{t("core.failed", { reason: failure })}</p>
      {:else if !info}
        <p class="card quiet">{t("core.loading")}</p>
      {:else}
        <div class="card">
          <dl>
            <div>
              <dt>{t("core.version")}</dt>
              <dd class="mono">{info.version}</dd>
            </div>
            <div>
              <dt>{t("core.platform")}</dt>
              <dd class="mono">{info.arch} · {info.os}</dd>
            </div>
            <div>
              <dt>{t("core.shell")}</dt>
              <dd>{t(`core.shell.${api.shell}`)}</dd>
            </div>
          </dl>

          <h3>{t("protocols.title")}</h3>
          <table>
            <thead>
              <tr>
                <th>{t("protocols.column.protocol")}</th>
                <th>{t("protocols.column.port")}</th>
                <th>{t("protocols.column.concurrency")}</th>
                <th>{t("protocols.column.encryption")}</th>
              </tr>
            </thead>
            <tbody>
              {#each info.protocols as entry (entry.protocol)}
                <tr>
                  <td>{t(`protocol.${entry.protocol}`)}</td>
                  <td class="mono">{entry.defaultPort ?? t("protocols.port.none")}</td>
                  <td class="mono">{entry.defaultConcurrency}</td>
                  <td>
                    <span class="chip {entry.encrypted ? 'ok' : 'warn'}">
                      {entry.encrypted ? t("encryption.on") : t("encryption.off")}
                    </span>
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
    </section>

    <section>
      <h2>{t("appearance.title")}</h2>
      <div class="card settings">
        <div class="row">
          <span class="label">{t("appearance.theme")}</span>
          <div class="choices">
            {#each THEMES as candidate (candidate)}
              <button
                type="button"
                class:active={currentTheme() === candidate}
                aria-pressed={currentTheme() === candidate}
                onclick={() => setTheme(candidate)}
              >
                {t(`theme.${candidate}`)}
              </button>
            {/each}
          </div>
        </div>

        <div class="row">
          <span class="label">{t("appearance.accent")}</span>
          <div class="choices">
            {#each ACCENTS as candidate (candidate)}
              <button
                type="button"
                class="swatch"
                class:active={currentAccent() === candidate}
                aria-pressed={currentAccent() === candidate}
                title={t(`accent.${candidate}`)}
                aria-label={t(`accent.${candidate}`)}
                data-accent={candidate}
                onclick={() => setAccent(candidate)}
              ></button>
            {/each}
          </div>
        </div>

        <div class="row">
          <span class="label">{t("language.title")}</span>
          <div class="choices">
            {#each LOCALES as candidate (candidate)}
              <button
                type="button"
                class:active={locale() === candidate}
                aria-pressed={locale() === candidate}
                onclick={() => setLocale(candidate)}
              >
                {t(`language.${candidate}`)}
              </button>
            {/each}
          </div>
        </div>
      </div>
    </section>

    <section>
      <h2>{t("next.title")}</h2>
      <p class="intro">{t("next.body")}</p>
    </section>
  </main>

  <footer>
    <span class="mono">{t("footer.line", { version: info?.version ?? "0.1.0" })}</span>
    <a href="https://github.com/sphings79/amberbeam" target="_blank" rel="noreferrer">
      {t("footer.repository")}
    </a>
  </footer>
</div>

<style>
  .shell {
    max-width: 940px;
    margin: 0 auto;
    padding: 34px 28px 48px;
    animation: ab-fade-up 0.28s cubic-bezier(0.22, 1, 0.36, 1) both;
  }

  header {
    display: flex;
    align-items: center;
    gap: 16px;
  }

  .mark {
    width: 52px;
    height: 52px;
    flex: none;
  }

  .mark svg {
    width: 100%;
    height: 100%;
    display: block;
    filter: drop-shadow(var(--shadow));
  }

  .title {
    flex: 1;
    min-width: 0;
  }

  h1 {
    margin: 0;
    font-size: 1.68rem;
    font-weight: 700;
    letter-spacing: -0.02em;
  }

  h1 span {
    color: var(--accent);
  }

  .title p {
    margin: 2px 0 0;
    color: var(--text-muted);
    max-width: 60ch;
  }

  .beam {
    height: 4px;
    margin: 20px 0 26px;
    border-radius: 999px;
    background: linear-gradient(90deg, transparent, var(--accent), transparent);
    background-size: 60% 100%;
    background-repeat: no-repeat;
    animation: ab-beam 3.4s cubic-bezier(0.4, 0, 0.2, 1) infinite;
  }

  .lead {
    margin: 0 0 32px;
    max-width: 72ch;
    color: var(--text-muted);
    font-size: 0.96rem;
  }

  section {
    margin-bottom: 32px;
  }

  h2 {
    margin: 0 0 6px;
    font-size: 0.72rem;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--text-faint);
  }

  .intro {
    margin: 0 0 14px;
    color: var(--text-muted);
    max-width: 72ch;
  }

  .card {
    background: var(--surface-1);
    border: 1px solid var(--border);
    border-radius: 1rem;
    box-shadow: var(--shadow);
    padding: 16px 18px;
  }

  .card.quiet {
    color: var(--text-muted);
    margin: 0;
  }

  .card.danger {
    margin: 0;
    border-color: var(--danger);
    background: var(--danger-soft);
    color: var(--danger);
  }

  .seams {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
    gap: 14px;
  }

  .seams .number {
    font-size: 0.7rem;
    letter-spacing: 0.08em;
    color: var(--accent);
  }

  .seams h3 {
    margin: 4px 0 6px;
    font-size: 0.96rem;
  }

  .seams p {
    margin: 0;
    color: var(--text-muted);
    font-size: 0.87rem;
  }

  dl {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(190px, 1fr));
    gap: 14px;
    margin: 0 0 20px;
  }

  dl div {
    background: var(--surface-2);
    border-radius: 0.7rem;
    padding: 10px 12px;
  }

  dt {
    font-size: 0.68rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-faint);
  }

  dd {
    margin: 2px 0 0;
    font-size: 0.98rem;
  }

  .card h3 {
    margin: 0 0 8px;
    font-size: 0.86rem;
    color: var(--text-muted);
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.88rem;
  }

  th,
  td {
    text-align: left;
    padding: 8px 10px;
    border-bottom: 1px solid var(--border);
  }

  tbody tr:last-child td {
    border-bottom: none;
  }

  th {
    font-size: 0.68rem;
    letter-spacing: 0.07em;
    text-transform: uppercase;
    color: var(--text-faint);
    font-weight: 600;
  }

  .chip {
    display: inline-block;
    padding: 2px 9px;
    border-radius: 999px;
    font-size: 0.72rem;
    letter-spacing: 0.04em;
    background: var(--surface-3);
    color: var(--text-muted);
  }

  .chip.accent {
    background: var(--accent-soft);
    color: var(--accent);
    text-transform: uppercase;
    align-self: flex-start;
  }

  .chip.ok {
    background: var(--ok-soft);
    color: var(--ok);
  }

  .chip.warn {
    background: var(--warn-soft);
    color: var(--warn);
  }

  .settings .row {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 10px 16px;
    padding: 10px 0;
    border-bottom: 1px solid var(--border);
  }

  .settings .row:first-child {
    padding-top: 0;
  }

  .settings .row:last-child {
    padding-bottom: 0;
    border-bottom: none;
  }

  .label {
    min-width: 92px;
    font-size: 0.68rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-faint);
  }

  .choices {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }

  button {
    font: inherit;
    font-size: 0.86rem;
    padding: 5px 14px;
    border: 1px solid var(--border-strong);
    border-radius: 999px;
    background: var(--surface-1);
    color: var(--text);
    cursor: default;
    transition: background 0.15s, border-color 0.15s, color 0.15s;
  }

  button:hover {
    background: var(--surface-2);
  }

  button.active {
    border-color: var(--accent);
    background: var(--accent-soft);
    color: var(--accent);
  }

  button:focus-visible {
    outline: none;
    box-shadow: 0 0 0 3px var(--accent-ring);
  }

  button.swatch {
    width: 26px;
    height: 26px;
    padding: 0;
    border-radius: 999px;
    background: var(--accent);
    border: 2px solid transparent;
    box-shadow: 0 0 0 1px var(--border-strong);
  }

  button.swatch.active {
    border-color: var(--surface-1);
    box-shadow: 0 0 0 2px var(--accent);
  }

  footer {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    gap: 8px 20px;
    padding-top: 16px;
    border-top: 1px solid var(--border);
    color: var(--text-faint);
    font-size: 0.8rem;
  }

  a {
    color: var(--accent);
  }
</style>
