<script lang="ts">
  /**
   * What a program driving AmberBeam may do, in one place.
   *
   * Its own window rather than a section of the settings, because it is the
   * only screen in this program where every line is a permission. Six switches
   * per server buried between host, port and passive mode is a form nobody
   * reads to the bottom — and what a server allows belongs beside what every
   * server allows, or nobody can see what is open.
   *
   * Everything here starts off and is saved as it is changed. There is no OK
   * button: a window that can be closed without applying is a window that
   * leaves somebody sure they granted something they did not.
   */
  import { api, type McpActivity, type Settings, type Site } from "../bridge";
  import { t } from "../i18n/index.svelte";
  import Switch from "./Switch.svelte";
  import { trap } from "./trap";

  interface Props {
    onclose: () => void;
  }

  let { onclose }: Props = $props();

  let settings = $state<Settings | null>(null);
  let servers = $state<Site[]>([]);
  let command = $state<string | null>(null);
  let copied = $state(false);

  /** Only a window with a file system of the viewer's own can point at one. */
  const canBrowse = api.shell === "desktop";

  /**
   * What the shell has been doing, asked for again while this is open.
   *
   * Only while it is open: a window nobody is looking at has no business
   * reading a file every few seconds, and the moment somebody does look is
   * the moment they want it current.
   */
  let activity = $state<McpActivity | null>(null);

  $effect(() => {
    const look = () => void api.mcpActivity().then((seen) => (activity = seen));
    look();
    const timer = window.setInterval(look, 4000);
    return () => window.clearInterval(timer);
  });

  $effect(() => {
    void api.settings().then((loaded) => (settings = loaded));
    void api.sites().then((loaded) => (servers = loaded));
    // Asked for rather than assembled: this is the one line in a client's
    // configuration that has to be exactly right, and a wrong path fails over
    // there with nothing on screen — an assistant with no tools and no way to
    // say why.
    void api.mcpCommand().then((path) => (command = path));
  });

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

  async function change(patch: Partial<Settings>): Promise<void> {
    if (!settings) return;
    const next = { ...settings, ...patch };
    settings = next;
    await api.setSettings(next);
  }

  /** One switch on one entry, written straight back to its file. */
  async function allow(site: Site, patch: Partial<Site>): Promise<void> {
    const next = { ...site, ...patch };
    servers = servers.map((one) => (one.id === site.id ? next : one));
    await api.saveSite(next.folder, next);
  }

  /** What is in the box, for somebody who would rather type than browse. */
  let typed = $state("");

  async function addPath(path: string): Promise<void> {
    const wanted = path.trim();
    if (!settings || wanted === "" || settings.mcpLocalPaths.includes(wanted)) return;
    await change({ mcpLocalPaths: [...settings.mcpLocalPaths, wanted] });
    typed = "";
  }

  async function browseForPath(): Promise<void> {
    const chosen = await api.chooseFolder(t("mcp.paths.pick"));
    if (chosen) await addPath(chosen);
  }

  async function dropPath(path: string): Promise<void> {
    if (!settings) return;
    await change({
      mcpLocalPaths: settings.mcpLocalPaths.filter((one) => one !== path),
    });
  }

  async function copySnippet(): Promise<void> {
    try {
      await navigator.clipboard.writeText(snippet);
      copied = true;
      window.setTimeout(() => (copied = false), 2000);
    } catch {
      // The clipboard can be refused. The block is selectable either way.
    }
  }

  /** The six, in the order somebody would turn them on. */
  const PERMISSIONS: { key: keyof Site; label: string }[] = [
    { key: "mcpSee", label: "mcp.see" },
    { key: "mcpUpload", label: "mcp.upload" },
    { key: "mcpDownload", label: "mcp.download" },
    { key: "mcpCreate", label: "mcp.create" },
    { key: "mcpRename", label: "mcp.rename" },
    { key: "mcpRemove", label: "mcp.remove" },
  ];
</script>

<div class="backdrop" role="presentation">
  <div class="dialog" use:trap role="dialog" aria-modal="true" aria-label={t("mcp.title")}>
    <header>
      <h2>{t("mcp.title")}</h2>
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
        <Switch
          checked={settings.mcpEnabled}
          label={t("mcp.enabled")}
          hint={t("mcp.enabled.hint")}
          onchange={(on) => change({ mcpEnabled: on })}
        />

        <!-- Everything below only means anything while the one above is on,
             and saying so once beats greying out twenty lines somebody then
             cannot read. -->
        <div class="rest" class:asleep={!settings.mcpEnabled}>
          <h3>{t("mcp.local")}</h3>
          <p class="hint">{t("mcp.local.hint")}</p>

          <Switch
            checked={settings.mcpLocalRead}
            label={t("mcp.local-read")}
            onchange={(on) => change({ mcpLocalRead: on })}
          />
          <Switch
            checked={settings.mcpLocalWrite}
            label={t("mcp.local-write")}
            onchange={(on) => change({ mcpLocalWrite: on })}
          />

          <p class="label">{t("mcp.paths")}</p>
          {#if settings.mcpLocalPaths.length === 0}
            <p class="hint">{t("mcp.paths.empty")}</p>
          {:else}
            <ul class="paths">
              {#each settings.mcpLocalPaths as path (path)}
                <li>
                  <span>{path}</span>
                  <button
                    type="button"
                    onclick={() => dropPath(path)}
                    title={t("mcp.paths.remove")}
                    aria-label={t("mcp.paths.remove")}
                  >×</button>
                </li>
              {/each}
            </ul>
          {/if}
          <!-- A box as well as a button: in the container there is no dialog
               to open, and the directory being named is on the machine running
               the service rather than the one looking at it. -->
          <div class="adding">
            <input
              bind:value={typed}
              placeholder={t("mcp.paths.placeholder")}
              spellcheck="false"
              autocapitalize="off"
              autocorrect="off"
              onkeydown={(event) => {
                if (event.key === "Enter") {
                  event.preventDefault();
                  void addPath(typed);
                }
              }}
            />
            <button type="button" onclick={() => addPath(typed)}>{t("mcp.paths.add")}</button>
            {#if canBrowse}
              <button type="button" onclick={browseForPath}>{t("mcp.paths.browse")}</button>
            {/if}
          </div>
          <p class="hint">{t("mcp.paths.hint")}</p>

          <h3>{t("mcp.servers")}</h3>
          <p class="hint">{t("mcp.servers.hint")}</p>

          {#if servers.length === 0}
            <p class="hint">{t("mcp.servers.none")}</p>
          {:else}
            {#each servers as site (site.id)}
              <div class="server">
                <p class="name">{site.name}</p>
                <div class="six">
                  {#each PERMISSIONS as permission (permission.key)}
                    <Switch
                      checked={site[permission.key] === true}
                      label={t(permission.label)}
                      onchange={(on) => allow(site, { [permission.key]: on })}
                    />
                  {/each}
                </div>
              </div>
            {/each}
          {/if}

          <h3>{t("mcp.global")}</h3>

          <Switch
            checked={settings.mcpQuickConnect}
            label={t("mcp.quick-connect")}
            hint={t("mcp.quick-connect.hint")}
            onchange={(on) => change({ mcpQuickConnect: on })}
          />
          <Switch
            checked={settings.mcpCreateSites}
            label={t("mcp.create-sites")}
            hint={t("mcp.create-sites.hint")}
            onchange={(on) => change({ mcpCreateSites: on })}
          />
        </div>

        <h3>{t("mcp.happening")}</h3>
        <p class="hint">
          {#if activity && activity.attached > 0}
            {t("mcp.happening.attached", { count: activity.attached })}
          {:else}
            {t("mcp.happening.quiet")}
          {/if}
        </p>
        {#if activity && activity.lines.length > 0}
          <!-- As it was written, in the order it was written. A log rearranged
               for the window is a log that cannot be compared with the file. -->
          <pre class="written">{activity.lines.join("\n")}</pre>
          <p class="hint">{t("mcp.happening.where", { path: activity.path })}</p>
        {:else}
          <p class="hint">{t("mcp.happening.nothing")}</p>
        {/if}

        {#if command === null}
          <h3>{t("mcp.setup")}</h3>
          <p class="hint">{t("mcp.web")}</p>
        {:else}
          <div class="setup">
            <h3>{t("mcp.setup")}</h3>
            <button type="button" onclick={copySnippet}>
              {copied ? t("mcp.copied") : t("mcp.copy")}
            </button>
          </div>
          <!-- Shown rather than only copied: somebody about to paste a path
               into a file that starts a program on their machine is entitled
               to read it first. -->
          <pre>{snippet}</pre>
          <p class="hint">{t("mcp.setup.hint")}</p>
        {/if}
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
    width: min(520px, 92vw);
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
    max-height: min(70vh, 620px);
    overflow-y: auto;
  }

  /* Stepped back rather than disabled: the switches still work, because
     setting up what will be allowed before allowing anything is a reasonable
     way round, and a greyed-out form cannot be read. */
  .rest.asleep {
    opacity: 0.55;
  }

  h3 {
    margin: 18px 0 0;
    padding-top: 14px;
    border-top: 1px solid var(--border);
    font-size: 0.84rem;
    font-weight: 600;
  }

  .hint {
    margin: 2px 0 0;
    font-size: 0.74rem;
    color: var(--text-faint);
    line-height: 1.5;
  }

  .label {
    margin: 14px 0 2px;
    font-size: 0.8rem;
    color: var(--text-muted);
  }

  .paths {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .paths li {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 8px;
    background: var(--surface-2);
    border-radius: 0.5rem;
  }

  .paths span {
    flex: 1;
    font-family: var(--font-mono, ui-monospace, monospace);
    font-size: 0.72rem;
    color: var(--text-muted);
    word-break: break-all;
  }

  .paths button {
    border: none;
    background: none;
    color: var(--text-faint);
    font-size: 1rem;
    cursor: default;
    padding: 0 2px;
  }

  .adding {
    display: flex;
    gap: 6px;
    margin-top: 6px;
  }

  .adding input {
    flex: 1;
    min-width: 0;
    font: inherit;
    font-size: 0.78rem;
    padding: 5px 8px;
    border: 1px solid var(--border-strong);
    border-radius: 0.5rem;
    background: var(--surface-1);
    color: var(--text);
  }

  .adding input:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-ring);
  }

  .adding button {
    padding: 5px 10px;
    font: inherit;
    font-size: 0.78rem;
    border: 1px solid var(--border-strong);
    border-radius: 0.5rem;
    background: var(--surface-2);
    color: var(--text);
    cursor: default;
  }

  .server {
    margin-top: 10px;
    padding: 8px 10px;
    background: var(--surface-2);
    border-radius: 0.6rem;
  }

  .name {
    margin: 0 0 4px;
    font-size: 0.82rem;
    font-weight: 600;
  }

  .six {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
    gap: 0 12px;
  }

  .setup {
    display: flex;
    align-items: flex-end;
    gap: 8px;
  }

  .setup h3 {
    flex: 1;
  }

  .setup button {
    margin-bottom: 2px;
    padding: 4px 10px;
    font: inherit;
    font-size: 0.78rem;
    border: 1px solid var(--border-strong);
    border-radius: 0.5rem;
    background: var(--surface-2);
    color: var(--text);
    cursor: default;
  }

  .written {
    /* Tall enough to read a few lines and no taller: this is the bottom of a
       window whose subject is the switches above it. */
    max-height: 180px;
    overflow-y: auto;
    white-space: pre;
  }

  pre {
    /* The body is a column that scrolls, and a box with an overflow of its own
       has no automatic minimum height in one. */
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
    overflow-x: auto;
  }
</style>
