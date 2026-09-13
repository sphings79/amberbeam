<script lang="ts">
  import { api, type EditRule, type OpenWith, type Settings } from "../bridge";
  import { t } from "../i18n/index.svelte";
  import Switch from "./Switch.svelte";
  import { trap } from "./trap";

  interface Props {
    onclose: () => void;
  }

  let { onclose }: Props = $props();

  let settings = $state<Settings | null>(null);

  /** Only a window with a file system of the viewer's own can point at one. */
  const canBrowse = api.shell === "desktop";

  /**
   * The table of what may be edited.
   *
   * Extensions are typed as one line of text rather than as a widget with
   * chips: it is a list of short words, and a text field is the fastest way in
   * and out of one. Split on anything that is not part of a name, so commas,
   * spaces and stray dots all work.
   */
  function asLine(rule: EditRule): string {
    return rule.extensions.join(", ");
  }

  function fromLine(text: string): string[] {
    const kinds = text
      .split(/[^A-Za-z0-9_+-]+/)
      .map((part) => part.trim().toLowerCase())
      .filter((part) => part !== "");
    // "psd, .PSD" is one kind typed twice. Keeping both would put a line in
    // the settings file that reads like a mistake, because it is one.
    return [...new Set(kinds)];
  }

  async function changeRule(index: number, patch: Partial<EditRule>): Promise<void> {
    if (!settings) return;
    const rules = settings.editing.map((rule, at) => (at === index ? { ...rule, ...patch } : rule));
    await change({ editing: rules });
  }

  async function addRule(): Promise<void> {
    if (!settings) return;
    // At the top, because the first line that matches wins: a line somebody
    // adds for one extension is meant to beat the long one underneath it.
    await change({
      editing: [{ extensions: [], openWith: "own", program: null }, ...settings.editing],
    });
  }

  async function removeRule(index: number): Promise<void> {
    if (!settings) return;
    await change({ editing: settings.editing.filter((_, at) => at !== index) });
  }

  async function pickProgram(index: number): Promise<void> {
    const chosen = await api.chooseFile(t("settings.editing.pick"));
    if (chosen) await changeRule(index, { openWith: "program", program: chosen });
  }

  /**
   * The path of the program a client would have to start, and null where
   * there is none to give.
   *
   * Asked for rather than written down, because this is the one line in the
   * client's configuration that has to be exactly right: a wrong path fails
   * silently over there, with an assistant that simply has no tools.
   */
  let command = $state<string | null>(null);
  let copied = $state(false);

  /**
   * Written out rather than handed to JSON.stringify, which puts a one-item
   * array on three lines. This is meant to be read and pasted, and it is the
   * shape these files are written in everywhere else.
   *
   * The path itself still goes through the encoder: a Windows one is full of
   * backslashes, and each of them has to be doubled in JSON.
   */
  const snippet = $derived(
    command === null
      ? ""
      : [
          "{",
          '  "mcpServers": {',
          '    "amberbeam": {',
          `      "command": ${JSON.stringify(command)},`,
          '      "args": ["--mcp"]',
          "    }",
          "  }",
          "}",
        ].join("\n"),
  );

  async function copySnippet(): Promise<void> {
    try {
      await navigator.clipboard.writeText(snippet);
      copied = true;
      window.setTimeout(() => (copied = false), 2000);
    } catch {
      // The clipboard can be refused. The block is selectable either way, so
      // there is nothing to report and nothing lost.
    }
  }

  $effect(() => {
    void api.settings().then((loaded) => (settings = loaded));
    void api.mcpCommand().then((path) => (command = path));
  });

  /** Saved as it is changed: a dialog with an OK button that can be lost is
      worse than one that simply remembers. */
  async function change(patch: Partial<Settings>): Promise<void> {
    if (!settings) return;
    const next = { ...settings, ...patch };
    settings = next;
    await api.setSettings(next);
  }
</script>

<div class="backdrop" role="presentation">
  <div class="dialog" use:trap role="dialog" aria-modal="true" aria-label={t("settings.title")}>
    <header>
      <h2>{t("settings.title")}</h2>
      <button
        type="button"
        class="close"
        onclick={onclose}
        title={t("action.close")}
        aria-label={t("action.close")}
      >×</button>
    </header>

    {#if settings}
      <div class="body">
        <label class="row">
          <span class="label">{t("settings.concurrency")}</span>
          <input
            type="number"
            min="1"
            max="64"
            value={settings.concurrency ?? ""}
            placeholder={t("settings.concurrency.auto")}
            onchange={(event) => {
              const text = event.currentTarget.value.trim();
              const parsed = Number.parseInt(text, 10);
              void change({
                concurrency: text === "" || Number.isNaN(parsed) ? null : parsed,
              });
            }}
          />
        </label>
        <p class="hint">{t("settings.concurrency.hint")}</p>

        <label class="row">
          <span class="label">{t("settings.retries")}</span>
          <input
            type="number"
            min="1"
            max="20"
            value={settings.retries}
            onchange={(event) => {
              const parsed = Number.parseInt(event.currentTarget.value, 10);
              if (!Number.isNaN(parsed)) void change({ retries: parsed });
            }}
          />
        </label>
        <p class="hint">{t("settings.retries.hint")}</p>

        <Switch
          checked={settings.temporaryName}
          label={t("settings.temporary-name")}
          hint={t("settings.temporary-name.hint")}
          onchange={(on) => change({ temporaryName: on })}
        />

        <Switch
          checked={settings.keepModified}
          label={t("settings.keep-modified")}
          onchange={(on) => change({ keepModified: on })}
        />

        <Switch
          checked={settings.keepPermissions}
          label={t("settings.keep-permissions")}
          hint={t("settings.keep-permissions.hint")}
          onchange={(on) => change({ keepPermissions: on })}
        />

        <Switch
          checked={settings.checkForUpdates}
          label={t("settings.check-updates")}
          hint={t("settings.check-updates.hint")}
          onchange={(on) => change({ checkForUpdates: on })}
        />

        <h3>{t("settings.comparing")}</h3>

        <Switch
          checked={settings.reviewComparison}
          label={t("settings.review")}
          hint={t("settings.review.hint")}
          onchange={(on) => change({ reviewComparison: on })}
        />

        <Switch
          checked={settings.deleteAlong}
          label={t("settings.delete-along")}
          hint={t("settings.delete-along.hint")}
          onchange={(on) => change({ deleteAlong: on })}
        />

        <h3>{t("settings.mcp")}</h3>
        <p class="hint">{t("settings.mcp.hint")}</p>

        <Switch
          checked={settings.mcpQuickConnect}
          label={t("settings.mcp.quick-connect")}
          hint={t("settings.mcp.quick-connect.hint")}
          onchange={(on) => change({ mcpQuickConnect: on })}
        />

        <Switch
          checked={settings.mcpCreateSites}
          label={t("settings.mcp.create-sites")}
          hint={t("settings.mcp.create-sites.hint")}
          onchange={(on) => change({ mcpCreateSites: on })}
        />

        {#if command === null}
          <p class="hint">{t("settings.mcp.web")}</p>
        {:else}
          <div class="setup">
            <span class="label">{t("settings.mcp.setup")}</span>
            <button type="button" onclick={copySnippet}>
              {copied ? t("settings.mcp.copied") : t("settings.mcp.copy")}
            </button>
          </div>
          <!-- Shown rather than only copied: somebody about to paste a path
               into a file that starts a program on their machine is entitled
               to read it first. -->
          <pre>{snippet}</pre>
          <p class="hint">{t("settings.mcp.setup.hint")}</p>
        {/if}

        <h3>{t("settings.editing")}</h3>
        <p class="hint">{t("settings.editing.hint")}</p>

        <div class="rules">
          {#each settings.editing as rule, index (index)}
            <div class="rule">
              <input
                class="kinds"
                value={asLine(rule)}
                placeholder={t("settings.editing.kinds")}
                spellcheck="false"
                autocapitalize="off"
                autocorrect="off"
                onchange={(event) =>
                  changeRule(index, { extensions: fromLine(event.currentTarget.value) })}
              />
              <select
                value={rule.openWith}
                onchange={(event) =>
                  changeRule(index, { openWith: event.currentTarget.value as OpenWith })}
              >
                <option value="own">{t("settings.editing.own")}</option>
                <option value="system">{t("settings.editing.system")}</option>
                <option value="program">{t("settings.editing.program")}</option>
              </select>
              <button
                type="button"
                class="drop"
                onclick={() => removeRule(index)}
                title={t("settings.editing.remove")}
                aria-label={t("settings.editing.remove")}
              >×</button>

              {#if rule.openWith === "program"}
                <div class="program">
                  <input
                    value={rule.program ?? ""}
                    placeholder={t("settings.editing.program.placeholder")}
                    spellcheck="false"
                    autocapitalize="off"
                    autocorrect="off"
                    onchange={(event) =>
                      changeRule(index, { program: event.currentTarget.value.trim() || null })}
                  />
                  {#if canBrowse}
                    <button type="button" onclick={() => pickProgram(index)}>
                      {t("settings.editing.browse")}
                    </button>
                  {/if}
                </div>
              {/if}
            </div>
          {/each}
        </div>

        <button type="button" class="add" onclick={addRule}>{t("settings.editing.add")}</button>

        {#if !canBrowse}
          <!-- The service runs on another machine, and a program started there
               would not appear in front of whoever asked for it. The rows stay
               editable: the same settings file is read by the desktop program
               when it drives this service. -->
          <p class="hint">{t("settings.editing.web")}</p>
        {/if}

        <p class="note">{t("settings.per-connection")}</p>
      </div>
    {/if}
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
    width: min(460px, 92vw);
    background: var(--surface-1);
    border: 1px solid var(--border-strong);
    border-radius: 1rem;
    box-shadow: var(--shadow-lg);
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
    display: flex;
    flex-direction: column;
    gap: 4px;
    /* The table grows with what somebody puts in it, and a dialog taller than
       the window is a dialog whose bottom row cannot be reached. */
    max-height: min(70vh, 560px);
    overflow-y: auto;
  }

  h3 {
    margin: 18px 0 0;
    padding-top: 14px;
    border-top: 1px solid var(--border);
    font-size: 0.84rem;
    font-weight: 600;
  }

  .setup {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 10px;
  }

  .setup .label {
    flex: 1;
    font-size: 0.8rem;
    color: var(--text-muted);
  }

  pre {
    /* The body is a column that scrolls, and a box with an overflow of its own
       has no automatic minimum height in one -- so without this it is squeezed
       to a line and a half while the text is still in there. */
    flex: none;
    margin: 6px 0 0;
    padding: 10px 12px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 0.6rem;
    font-family: var(--font-mono, ui-monospace, monospace);
    font-size: 0.72rem;
    line-height: 1.5;
    color: var(--text-muted);
    /* A path can be long, and a dialog that grows sideways with it is a dialog
       whose other fields move. */
    overflow-x: auto;
  }

  .rules {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-top: 8px;
  }

  .rule {
    display: grid;
    grid-template-columns: 1fr auto auto;
    align-items: center;
    gap: 6px;
  }

  .rule input,
  .rule select {
    font: inherit;
    font-size: 0.8rem;
    padding: 4px 7px;
    border: 1px solid var(--border-strong);
    border-radius: 0.45rem;
    background: var(--surface-2);
    color: var(--text);
    min-width: 0;
  }

  .program {
    grid-column: 1 / -1;
    display: flex;
    gap: 6px;
  }

  .program input {
    flex: 1;
  }

  .program button,
  .add {
    border: 1px solid var(--border-strong);
    border-radius: 0.45rem;
    background: transparent;
    color: var(--text-muted);
    font: inherit;
    font-size: 0.78rem;
    padding: 4px 10px;
    cursor: pointer;
  }

  .add {
    align-self: flex-start;
    margin-top: 8px;
  }

  .drop {
    border: none;
    background: none;
    color: var(--text-faint);
    font-size: 1rem;
    line-height: 1;
    padding: 0 4px;
    cursor: pointer;
  }

  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-top: 8px;
  }

  .label {
    font-size: 0.84rem;
    color: var(--text);
  }

  input[type="number"] {
    width: 96px;
    font: inherit;
    font-size: 0.86rem;
    padding: 4px 8px;
    border: 1px solid var(--border-strong);
    border-radius: 0.5rem;
    background: var(--surface-2);
    color: var(--text);
    text-align: right;
  }

  input:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-ring);
  }

  .hint {
    margin: 2px 0 0;
    font-size: 0.74rem;
    color: var(--text-faint);
  }

  .note {
    margin: 16px 0 0;
    padding-top: 12px;
    border-top: 1px solid var(--border);
    font-size: 0.74rem;
    color: var(--text-faint);
  }
</style>
