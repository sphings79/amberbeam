<script lang="ts">
  /**
   * The server list, in a window of its own.
   *
   * The tree on screen is the tree on disk: a folder here is a directory under
   * `sites/`, an entry is one JSON file in it. That is why moving an entry is a
   * single save with a different folder, and why nothing has to be kept in step
   * with anything else.
   *
   * Connecting does not happen here. This window asks the main one to open an
   * entry, because every question a connection can raise — an unknown host key,
   * a certificate nobody vouches for, a password that was not stored — has its
   * dialog over there, and asking twice in two places is how two answers end up
   * disagreeing.
   */
  import { api, type OpenSide, type ProtocolInfo, type Protocol, type Site } from "../bridge";
  import { t } from "../i18n/index.svelte";
  import { trap } from "./trap";
  import { ACCENTS } from "../theme/index.svelte";
  import { describe } from "./errors";
  import Icon from "./Icon.svelte";
  import ExportDialog from "./ExportDialog.svelte";
  import ImportDialog from "./ImportDialog.svelte";

  /** The four a person chooses between, as in the connect dialog. */
  const KINDS = [
    { id: "sftp", protocol: "sftp", port: 22 },
    { id: "ftps-explicit", protocol: "ftps", port: 21, encryption: "explicit" },
    { id: "ftps-implicit", protocol: "ftps", port: 990, encryption: "implicit" },
    { id: "ftp", protocol: "ftp", port: 21 },
  ] as const;
  type Kind = (typeof KINDS)[number];

  /**
   * What each protocol suggests, asked of the core rather than written down
   * here. Eight simultaneous transfers over SFTP are channels in one
   * connection; eight over FTP are eight logins, which is what a shared hoster
   * refuses. Keeping the numbers in one place is how the window cannot drift
   * away from the reason behind them.
   */
  let defaults = $state<ProtocolInfo[]>([]);

  let rows = $state<Site[]>([]);
  let folders = $state<string[]>([]);
  let search = $state("");
  let collapsed = $state(new Set<string>());
  let selectedId = $state<string | null>(null);
  let draft = $state<Site | null>(null);
  /** The folder the draft belongs in, kept apart because it is not part of the entry. */
  let draftFolder = $state("");
  let newPassword = $state("");
  let newPassphrase = $state("");
  let failure = $state<unknown>(null);
  let note = $state<string | null>(null);
  /** The entry waiting for an answer about which side to open it on. */
  let asking = $state<Site | null>(null);
  /**
   * A name being asked for, in a dialog of our own rather than `window.prompt`.
   *
   * Not out of taste: a webview may answer a browser prompt with nothing at
   * all, and a folder that silently refuses to be created is a worse bug than
   * an ugly box.
   */
  let naming = $state<{ title: string; value: string; apply: (name: string) => void } | null>(null);
  let importing = $state(false);
  let exporting = $state(false);
  let dragging = $state<string | null>(null);
  /** The entry a delete is waiting to be confirmed for. */
  let removing = $state<Site | null>(null);
  /** The search field, so a key can put the cursor in it. */
  let searchField = $state<HTMLInputElement | null>(null);
  let nameField = $state<HTMLInputElement | null>(null);
  let dropFolder = $state<string | null>(null);

  $effect(() => {
    void api.coreInfo().then((info) => (defaults = info.protocols));
  });

  function suggested(protocol: Protocol): number | null {
    return defaults.find((entry) => entry.protocol === protocol)?.defaultConcurrency ?? null;
  }

  async function reload(): Promise<void> {
    try {
      [rows, folders] = await Promise.all([api.sites(), api.siteFolders()]);
      failure = null;
    } catch (problem) {
      failure = problem;
    }
  }

  // Read once when the window opens. Site files are meant to be edited by hand,
  // and watching the directory for that would mean deciding what to do about a
  // file that changes while somebody is editing the same entry here.
  $effect(() => {
    void reload();
  });

  let matching = $derived(
    rows.filter((row) => {
      const needle = search.trim().toLowerCase();
      if (!needle) return true;
      return [row.name, row.host, row.user, row.folder].some((field) =>
        field?.toLowerCase().includes(needle),
      );
    }),
  );

  /** Every folder that has to be drawn, including the empty ones. */
  let allFolders = $derived(
    [...new Set([...folders, ...matching.map((row) => row.folder).filter(Boolean)])].sort((a, b) =>
      a.toLowerCase().localeCompare(b.toLowerCase()),
    ),
  );

  /** A folder is hidden when any folder above it is collapsed. */
  function visible(folder: string): boolean {
    const parts = folder.split("/");
    for (let i = 1; i < parts.length; i += 1) {
      if (collapsed.has(parts.slice(0, i).join("/"))) return false;
    }
    return true;
  }

  function toggle(folder: string): void {
    const next = new Set(collapsed);
    if (next.has(folder)) next.delete(folder);
    else next.add(folder);
    collapsed = next;
  }

  /**
   * The entries in the order they are drawn, which is the order the arrow keys
   * walk. Derived from the same lists the tree uses, so the two can never
   * disagree about what is where.
   */
  let walkable = $derived([
    ...matching.filter((row) => row.folder === ""),
    ...allFolders
      .filter(visible)
      .flatMap((folder) => matching.filter((row) => row.folder === folder)),
  ]);

  function step(by: number): void {
    if (walkable.length === 0) return;
    const at = walkable.findIndex((row) => row.id === selectedId);
    const next = at < 0 ? (by > 0 ? 0 : walkable.length - 1) : at + by;
    const landed = walkable[Math.min(walkable.length - 1, Math.max(0, next))];
    if (landed) select(landed);
  }

  /**
   * Keys for the whole window.
   *
   * Nothing here fires while a field has the cursor: somebody typing a password
   * with a "d" in it must not delete a server. The one exception is Escape,
   * which is how a person gets out of a field in the first place.
   */
  function onKey(event: KeyboardEvent): void {
    const target = event.target as HTMLElement | null;
    const typing = !!target && ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName);

    if (event.key === "Escape") {
      if (naming) naming = null;
      else if (asking) asking = null;
      else if (removing) removing = null;
      else if (importing || exporting) {
        importing = false;
        exporting = false;
      } else if (typing) target?.blur();
      return;
    }

    // A dialog is in front; the tree is not what the keys are for.
    if (naming || asking || removing || importing || exporting || typing) return;

    const meta = event.metaKey || event.ctrlKey;
    if (meta && event.key.toLowerCase() === "f") {
      event.preventDefault();
      searchField?.focus();
      searchField?.select();
      return;
    }
    if (meta && event.key.toLowerCase() === "n") {
      event.preventDefault();
      if (event.shiftKey) addFolder();
      else addSite();
      return;
    }

    switch (event.key) {
      case "ArrowDown":
        event.preventDefault();
        step(1);
        break;
      case "ArrowUp":
        event.preventDefault();
        step(-1);
        break;
      case "Enter": {
        const chosen = rows.find((row) => row.id === selectedId);
        if (chosen) {
          event.preventDefault();
          asking = chosen;
        }
        break;
      }
      case "Delete":
      case "Backspace": {
        const chosen = rows.find((row) => row.id === selectedId);
        if (chosen) {
          event.preventDefault();
          removing = chosen;
        }
        break;
      }
      default:
        break;
    }
  }

  function kindOf(site: Site): Kind {
    return (
      KINDS.find(
        (candidate) =>
          candidate.protocol === site.protocol &&
          ("encryption" in candidate ? candidate.encryption : null) ===
            (site.protocol === "ftps" ? site.encryption : null),
      ) ??
      KINDS.find((candidate) => candidate.protocol === site.protocol) ??
      KINDS[0]
    );
  }

  function select(site: Site): void {
    selectedId = site.id;
    draft = { ...site };
    draftFolder = site.folder;
    newPassword = "";
    newPassphrase = "";
    note = null;
  }

  function blank(folder: string): Site {
    return {
      id: "",
      name: "",
      folder,
      hasPassword: false,
      protocol: "sftp",
      host: "",
      port: 22,
      user: "",
      auth: "password",
      keyPath: null,
      remotePath: null,
      localPath: null,
      concurrency: suggested("sftp") ?? 8,
      retries: null,
      temporaryName: null,
      encryption: null,
      passive: null,
      latin1: null,
      keepAlive: null,
      rememberPassword: false,
      colour: null,
    };
  }

  function addSite(): void {
    selectedId = null;
    draft = blank(draftFolder);
    newPassword = "";
    newPassphrase = "";
    // The cursor goes where the typing goes. A new entry whose first field has
    // to be found with the mouse is a new entry that takes a mouse.
    queueMicrotask(() => nameField?.focus());
  }

  function choose(kind: Kind): void {
    if (!draft) return;
    const before = kindOf(draft);

    // The port and the number of simultaneous transfers move with the protocol,
    // but only while they are still what the previous protocol suggested. A
    // value somebody typed is theirs, and nothing here overwrites it.
    if (draft.port === before.port) draft.port = kind.port;
    const wasDefault = draft.concurrency === suggested(before.protocol as Protocol);
    if (wasDefault) {
      draft.concurrency = suggested(kind.protocol as Protocol) ?? draft.concurrency;
    }

    if (kind.protocol !== "sftp" && draft.auth !== "password") draft.auth = "password";
    draft.protocol = kind.protocol as Protocol;
    draft.encryption = "encryption" in kind ? kind.encryption : null;
  }

  async function save(): Promise<void> {
    if (!draft || !draft.name || !draft.host) return;
    try {
      // A new entry gets its identifier from the core, not from here: it is
      // what the credential store files the password under, and one place has
      // to be in charge of it.
      const site = { ...draft, folder: draftFolder };
      site.id = await api.saveSite(draftFolder, site);

      // The secrets after the entry, so a password is never filed under an
      // identifier that failed to be written.
      if (site.rememberPassword && newPassword) {
        await api.setSiteSecret(site.id, "password", newPassword);
      }
      if (site.rememberPassword && newPassphrase) {
        await api.setSiteSecret(site.id, "passphrase", newPassphrase);
      }
      newPassword = "";
      newPassphrase = "";
      await reload();
      selectedId = site.id;
      const written = rows.find((row) => row.id === site.id);
      if (written) select(written);
      note = t("sites.saved");
      failure = null;
    } catch (problem) {
      failure = problem;
    }
  }

  async function remove(site: Site): Promise<void> {
    removing = null;
    try {
      await api.deleteSite(site.id);
      if (selectedId === site.id) {
        selectedId = null;
        draft = null;
      }
      await reload();
    } catch (problem) {
      failure = problem;
    }
  }

  function addFolder(): void {
    naming = {
      title: t("sites.folder.new"),
      value: "",
      apply: (name) => {
        void (async () => {
          try {
            await api.createSiteFolder(draftFolder ? `${draftFolder}/${name}` : name);
            await reload();
          } catch (problem) {
            failure = problem;
          }
        })();
      },
    };
  }

  function renameFolder(folder: string): void {
    const parts = folder.split("/");
    const was = parts[parts.length - 1] ?? folder;
    naming = {
      title: t("sites.folder.rename"),
      value: was,
      apply: (name) => {
        if (name === was) return;
        parts[parts.length - 1] = name;
        void (async () => {
          try {
            await api.renameSiteFolder(folder, parts.join("/"));
            await reload();
          } catch (problem) {
            failure = problem;
          }
        })();
      },
    };
  }

  async function removeFolder(folder: string): Promise<void> {
    try {
      await api.deleteSiteFolder(folder);
      if (draftFolder === folder) draftFolder = "";
      await reload();
    } catch (problem) {
      // Almost always "this folder is not empty", which is a sentence worth
      // showing rather than a silent refusal.
      failure = problem;
    }
  }

  /** Dropping an entry on a folder moves it: one save with a different folder. */
  async function moveTo(folder: string): Promise<void> {
    const site = rows.find((row) => row.id === dragging);
    dragging = null;
    dropFolder = null;
    if (!site || site.folder === folder) return;
    try {
      await api.saveSite(folder, { ...site, folder });
      await reload();
      if (selectedId === site.id) {
        const moved = rows.find((row) => row.id === site.id);
        if (moved) select(moved);
      }
    } catch (problem) {
      failure = problem;
    }
  }

  async function open(site: Site, side: OpenSide): Promise<void> {
    asking = null;
    try {
      await api.openSite(site.id, side);
    } catch (problem) {
      failure = problem;
    }
  }

  let remote = $derived(draft ? draft.protocol !== "sftp" && draft.protocol !== "local" : false);
</script>

<svelte:window onkeydown={onKey} />

<div class="manager">
  <aside>
    <div class="tools">
      <input
        class="search"
        bind:this={searchField}
        bind:value={search}
        placeholder={t("sites.search")}
        autocomplete="off"
        autocapitalize="off"
        autocorrect="off"
        spellcheck="false"
      />
      <button type="button" onclick={addSite} title={t("sites.new")}>
        <Icon name="new-file" size={14} />
      </button>
      <button type="button" onclick={addFolder} title={t("sites.folder.new")}>
        <Icon name="new-folder" size={14} />
      </button>
      <button type="button" onclick={() => (importing = true)} title={t("sites.import")}>
        <Icon name="transfer" size={14} />
      </button>
      <button
        type="button"
        onclick={() => (exporting = true)}
        title={t("sites.export")}
        disabled={rows.length === 0}
      >
        <Icon name="disconnect" size={14} />
      </button>
    </div>

    <div
      class="tree"
      role="tree"
      tabindex="-1"
      ondragover={(event) => {
        event.preventDefault();
        dropFolder = "";
      }}
      ondrop={() => void moveTo("")}
    >
      <!-- The top level is a drop target of its own, so an entry can be taken
           out of a folder as well as put into one. -->
      <div
        class="row folder"
        class:active={draftFolder === ""}
        class:over={dropFolder === ""}
        role="treeitem"
        tabindex="0"
        aria-selected={draftFolder === ""}
        onclick={() => (draftFolder = "")}
        onkeydown={(event) => event.key === "Enter" && (draftFolder = "")}
      >
        <Icon name="folder" size={13} />
        <span class="name">{t("sites.top-level")}</span>
      </div>

      {#each matching.filter((row) => row.folder === "") as site (site.id)}
        {@render entry(site, 1)}
      {/each}

      {#each allFolders.filter(visible) as folder (folder)}
        {@const depth = folder.split("/").length}
        {@const name = folder.split("/").pop()}
        <div
          class="row folder"
          class:active={draftFolder === folder}
          class:over={dropFolder === folder}
          role="treeitem"
          tabindex="0"
          aria-selected={draftFolder === folder}
          aria-expanded={!collapsed.has(folder)}
          style="padding-left: {depth * 14}px"
          onclick={() => (draftFolder = folder)}
          onkeydown={(event) => event.key === "Enter" && (draftFolder = folder)}
          ondragover={(event) => {
            event.preventDefault();
            event.stopPropagation();
            dropFolder = folder;
          }}
          ondrop={(event) => {
            event.stopPropagation();
            void moveTo(folder);
          }}
        >
          <button
            type="button"
            class="twist"
            onclick={(event) => {
              event.stopPropagation();
              toggle(folder);
            }}
            aria-label={t("sites.folder.toggle")}
          >
            {collapsed.has(folder) ? "▸" : "▾"}
          </button>
          <Icon name="folder" size={13} />
          <span class="name">{name}</span>
          <button
            type="button"
            class="quiet"
            onclick={(event) => {
              event.stopPropagation();
              renameFolder(folder);
            }}
            title={t("sites.folder.rename")}>✎</button
          >
          <button
            type="button"
            class="quiet"
            onclick={(event) => {
              event.stopPropagation();
              void removeFolder(folder);
            }}
            title={t("sites.folder.delete")}>×</button
          >
        </div>

        {#each matching.filter((row) => row.folder === folder) as site (site.id)}
          {@render entry(site, depth + 1)}
        {/each}
      {/each}

      {#if matching.length === 0 && allFolders.length === 0}
        <p class="empty">{t("sites.empty")}</p>
      {/if}
    </div>

    <p class="keys mono">{t("sites.keys")}</p>
  </aside>

  <section class="detail">
    {#if draft}
      <form
        onsubmit={(event) => {
          event.preventDefault();
          void save();
        }}
      >
        <div class="row">
          <label class="grow">
            <span>{t("sites.name")}</span>
            <input
              bind:this={nameField}
              bind:value={draft.name}
              autocomplete="off"
              autocapitalize="off"
              autocorrect="off"
              spellcheck="false"
              required
            />
          </label>
          <label class="narrow">
            <span>{t("sites.colour")}</span>
            <select bind:value={draft.colour}>
              <option value={null}>{t("sites.colour.none")}</option>
              {#each ACCENTS as accent (accent)}
                <option value={accent}>{t(`accent.${accent}`)}</option>
              {/each}
            </select>
          </label>
        </div>

        <fieldset>
          <legend>{t("quick.protocol")}</legend>
          <div class="choices">
            {#each KINDS as candidate (candidate.id)}
              <button
                type="button"
                class:active={kindOf(draft).id === candidate.id}
                onclick={() => choose(candidate)}
              >
                {t(`protocol.${candidate.id}`)}
              </button>
            {/each}
          </div>
        </fieldset>

        {#if draft.protocol === "ftp"}
          <p class="warning">{t("protocol.ftp.warning")}</p>
        {/if}

        <div class="row">
          <label class="grow">
            <span>{t("quick.host")}</span>
            <input
              bind:value={draft.host}
              placeholder="beispiel.de"
              autocomplete="off"
              autocapitalize="off"
              autocorrect="off"
              spellcheck="false"
              required
            />
          </label>
          <label class="port">
            <span>{t("quick.port")}</span>
            <input type="number" bind:value={draft.port} min="1" max="65535" autocomplete="off" />
          </label>
        </div>

        <label>
          <span>{t("quick.user")}</span>
          <input
            bind:value={draft.user}
            autocomplete="off"
            autocapitalize="off"
            autocorrect="off"
            spellcheck="false"
          />
        </label>

        {#if !remote}
          <fieldset>
            <legend>{t("quick.auth")}</legend>
            <div class="choices">
              {#each ["password", "key-file", "agent"] as const as method (method)}
                <button
                  type="button"
                  class:active={draft.auth === method}
                  onclick={() => draft && (draft.auth = method)}
                >
                  {t(`auth.${method}`)}
                </button>
              {/each}
            </div>
          </fieldset>
        {/if}

        {#if draft.auth === "key-file"}
          <label>
            <span>{t("quick.key-path")}</span>
            <input
              bind:value={draft.keyPath}
              placeholder="~/.ssh/id_ed25519"
              autocomplete="off"
              autocapitalize="off"
              autocorrect="off"
              spellcheck="false"
            />
          </label>
          <p class="hint">{t("quick.key-path.hint")}</p>
        {/if}

        {#if draft.auth !== "agent"}
          <label class="check">
            <input type="checkbox" bind:checked={draft.rememberPassword} />
            <span>{t("sites.remember")}</span>
          </label>

          {#if draft.rememberPassword}
            <label>
              <span>
                {draft.auth === "key-file" ? t("quick.passphrase") : t("quick.password")}
                {#if draft.hasPassword}<em class="stored">{t("sites.stored")}</em>{/if}
              </span>
              {#if draft.auth === "key-file"}
                <input
                  type="password"
                  bind:value={newPassphrase}
                  placeholder={draft.hasPassword ? t("sites.unchanged") : ""}
                  autocomplete="off"
                />
              {:else}
                <input
                  type="password"
                  bind:value={newPassword}
                  placeholder={draft.hasPassword ? t("sites.unchanged") : ""}
                  autocomplete="off"
                />
              {/if}
            </label>
            <p class="hint">{t("sites.remember.hint")}</p>
          {/if}
        {/if}

        <div class="row">
          <label class="grow">
            <span>{t("sites.remote-path")}</span>
            <input
              bind:value={draft.remotePath}
              placeholder="/var/www/html"
              autocomplete="off"
              autocapitalize="off"
              autocorrect="off"
              spellcheck="false"
            />
          </label>
          <label class="grow">
            <span>{t("sites.local-path")}</span>
            <input
              bind:value={draft.localPath}
              autocomplete="off"
              autocapitalize="off"
              autocorrect="off"
              spellcheck="false"
            />
          </label>
        </div>

        <div class="row">
          <label class="narrow">
            <span>{t("settings.concurrency")}</span>
            <input type="number" bind:value={draft.concurrency} min="1" max="64" autocomplete="off" />
          </label>
          <label class="narrow">
            <span>{t("settings.retries")}</span>
            <input type="number" bind:value={draft.retries} min="1" max="20" autocomplete="off" />
          </label>
          {#if remote}
            <label class="narrow">
              <span>{t("sites.keep-alive")}</span>
              <input type="number" bind:value={draft.keepAlive} min="1" max="600" autocomplete="off" />
            </label>
          {/if}
        </div>

        {#if remote}
          <label class="check">
            <input
              type="checkbox"
              checked={draft.passive !== false}
              onchange={(event) => draft && (draft.passive = event.currentTarget.checked)}
            />
            <span>{t("quick.passive")}</span>
          </label>
          <label class="check">
            <input
              type="checkbox"
              checked={draft.latin1 === true}
              onchange={(event) => draft && (draft.latin1 = event.currentTarget.checked)}
            />
            <span>{t("quick.latin1")}</span>
          </label>
        {/if}

        {#if failure}
          <p class="failure">{describe(failure)}</p>
        {/if}
        {#if note}
          <p class="note">{note}</p>
        {/if}

        <div class="actions">
          {#if draft.id}
            <button type="button" class="danger" onclick={() => (removing = draft)}>
              {t("action.delete")}
            </button>
          {/if}
          <span class="spacer"></span>
          <button
            type="button"
            onclick={() => {
              draft = null;
              selectedId = null;
            }}
          >
            {t("action.cancel")}
          </button>
          <button type="submit" class="primary" disabled={!draft.name || !draft.host}>
            {t("action.save")}
          </button>
        </div>
      </form>
    {:else}
      <p class="placeholder">{t("sites.pick")}</p>
      {#if failure}
        <p class="failure">{describe(failure)}</p>
      {/if}
    {/if}
  </section>
</div>

{#if removing}
  <div class="backdrop" role="presentation">
    <div class="dialog" use:trap role="alertdialog" aria-modal="true">
      <h2>{t("sites.delete.title", { name: removing.name })}</h2>
      <p>
        {removing.hasPassword ? t("sites.delete.with-password") : t("sites.delete.body")}
      </p>
      <div class="actions">
        <span class="spacer"></span>
        <button type="button" onclick={() => (removing = null)}>{t("action.cancel")}</button>
        <button type="button" class="danger" onclick={() => removing && void remove(removing)}>
          {t("action.delete")}
        </button>
      </div>
    </div>
  </div>
{/if}

{#if exporting}
  <ExportDialog
    onclose={() => (exporting = false)}
    ondone={(count, path) => {
      exporting = false;
      note = t("export.done", { count, path });
    }}
  />
{/if}

{#if importing}
  <ImportDialog
    into={draftFolder}
    onclose={() => (importing = false)}
    ondone={(taken) => {
      importing = false;
      note = t("import.done", { count: taken });
      void reload();
    }}
  />
{/if}

{#if naming}
  <div class="backdrop" role="presentation">
    <div class="dialog" use:trap role="dialog" aria-modal="true" aria-label={naming.title}>
    <form
      onsubmit={(event) => {
        event.preventDefault();
        const name = naming?.value.trim();
        const apply = naming?.apply;
        naming = null;
        if (name && apply) apply(name);
      }}
    >
      <h2>{naming.title}</h2>
      <!-- svelte-ignore a11y_autofocus -->
      <input
        bind:value={naming.value}
        autofocus
        autocomplete="off"
        autocapitalize="off"
        autocorrect="off"
        spellcheck="false"
      />
      <div class="actions">
        <span class="spacer"></span>
        <button type="button" onclick={() => (naming = null)}>{t("action.cancel")}</button>
        <button type="submit" class="primary" disabled={!naming.value.trim()}>
          {t("action.save")}
        </button>
      </div>
    </form>
    </div>
  </div>
{/if}

{#if asking}
  <div class="backdrop" role="presentation">
    <div class="dialog" use:trap role="dialog" aria-modal="true">
      <h2>{t("sites.open.title", { name: asking.name })}</h2>
      <p>{t("sites.open.body")}</p>
      <div class="actions">
        <button type="button" onclick={() => (asking = null)}>{t("action.cancel")}</button>
        <button type="button" onclick={() => asking && void open(asking, "left")}>
          {t("sites.open.left")}
        </button>
        <button type="button" class="primary" onclick={() => asking && void open(asking, "right")}>
          {t("sites.open.right")}
        </button>
      </div>
    </div>
  </div>
{/if}

{#snippet entry(site: Site, depth: number)}
  <div
    class="row site"
    class:selected={selectedId === site.id}
    role="treeitem"
    tabindex="0"
    aria-selected={selectedId === site.id}
    draggable="true"
    style="padding-left: {depth * 14}px"
    ondragstart={() => (dragging = site.id)}
    ondragend={() => {
      dragging = null;
      dropFolder = null;
    }}
    onclick={() => select(site)}
    ondblclick={() => (asking = site)}
    onkeydown={(event) => {
      if (event.key === "Enter") asking = site;
    }}
  >
    {#if site.colour}
      <span class="dot" data-accent={site.colour}></span>
    {:else}
      <span class="dot none"></span>
    {/if}
    <span class="name">{site.name}</span>
    <span class="where mono">{site.user}@{site.host}</span>
  </div>
{/snippet}

<style>
  .manager {
    display: grid;
    grid-template-columns: minmax(240px, 320px) 1fr;
    height: 100vh;
    background: var(--surface-0);
    color: var(--text);
  }

  aside {
    display: flex;
    flex-direction: column;
    border-right: 1px solid var(--border);
    min-height: 0;
  }

  .tools {
    display: flex;
    gap: 6px;
    padding: 8px;
    border-bottom: 1px solid var(--border);
  }

  .search {
    flex: 1;
    min-width: 0;
  }

  .tree {
    flex: 1;
    overflow: auto;
    padding: 4px 0;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 3px 8px;
    font-size: 0.84rem;
    cursor: default;
    border: 1px solid transparent;
  }

  .row:hover {
    background: var(--surface-2);
  }

  .row.folder.active {
    background: var(--surface-2);
    font-weight: 600;
  }

  .row.folder.over {
    border-color: var(--accent);
    background: var(--accent-soft);
  }

  .row.site.selected {
    background: var(--accent-soft);
    color: var(--accent);
  }

  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .where {
    font-size: 0.72rem;
    color: var(--text-faint);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 45%;
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 999px;
    background: var(--accent);
    flex: none;
  }

  .dot.none {
    background: transparent;
    border: 1px solid var(--border-strong);
  }

  .twist,
  .quiet {
    background: none;
    border: none;
    color: var(--text-faint);
    font: inherit;
    font-size: 0.72rem;
    padding: 0 2px;
    cursor: default;
  }

  .twist:hover,
  .quiet:hover {
    color: var(--text);
  }

  .keys {
    margin: 0;
    padding: 6px 8px;
    border-top: 1px solid var(--border);
    font-size: 0.66rem;
    color: var(--text-faint);
    line-height: 1.5;
  }

  .empty,
  .placeholder {
    color: var(--text-faint);
    font-size: 0.82rem;
    padding: 16px;
    margin: 0;
  }

  .detail {
    overflow: auto;
    padding: 16px 20px;
    min-height: 0;
  }

  form {
    display: flex;
    flex-direction: column;
    gap: 10px;
    max-width: 620px;
  }

  .row:has(label) {
    display: flex;
    gap: 10px;
    padding: 0;
    border: none;
  }

  .row:has(label):hover {
    background: none;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 0.78rem;
    color: var(--text-muted);
  }

  label.grow {
    flex: 1;
    min-width: 0;
  }

  label.port {
    width: 90px;
  }

  label.narrow {
    width: 120px;
  }

  label.check {
    flex-direction: row;
    align-items: center;
    gap: 8px;
  }

  label.check input {
    width: auto;
  }

  input,
  select {
    font: inherit;
    font-size: 0.86rem;
    padding: 5px 8px;
    border-radius: 0.4rem;
    border: 1px solid var(--border-strong);
    background: var(--surface-1);
    color: var(--text);
    width: 100%;
  }

  fieldset {
    border: none;
    padding: 0;
    margin: 0;
  }

  legend {
    font-size: 0.78rem;
    color: var(--text-muted);
    padding: 0 0 3px;
  }

  .choices {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }

  .choices button {
    font: inherit;
    font-size: 0.82rem;
    padding: 5px 12px;
    border-radius: 999px;
    border: 1px solid var(--border-strong);
    background: var(--surface-1);
    color: var(--text-muted);
    cursor: default;
  }

  .choices button.active {
    border-color: var(--accent);
    background: var(--accent-soft);
    color: var(--accent);
  }

  .hint {
    font-size: 0.76rem;
    color: var(--text-faint);
    margin: 0;
  }

  .stored {
    font-style: normal;
    font-size: 0.7rem;
    color: var(--accent);
  }

  .warning,
  .failure {
    font-size: 0.8rem;
    color: var(--danger);
    background: var(--danger-soft);
    padding: 8px 10px;
    border-radius: 0.5rem;
    margin: 0;
  }

  .note {
    font-size: 0.8rem;
    color: var(--accent);
    margin: 0;
  }

  .actions {
    display: flex;
    gap: 8px;
    align-items: center;
    margin-top: 4px;
  }

  .spacer {
    flex: 1;
  }

  .actions button,
  .tools button {
    font: inherit;
    font-size: 0.84rem;
    padding: 5px 14px;
    border-radius: 999px;
    border: 1px solid var(--border-strong);
    background: var(--surface-1);
    color: var(--text);
    cursor: default;
  }

  .actions button:hover,
  .tools button:hover {
    background: var(--surface-2);
  }

  .actions button.primary {
    border-color: var(--accent);
    background: var(--accent-soft);
    color: var(--accent);
  }

  .actions button.danger {
    border-color: var(--danger);
    background: var(--danger-soft);
    color: var(--danger);
  }

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

  .dialog .danger {
    border-color: var(--danger);
    background: var(--danger-soft);
    color: var(--danger);
  }

  .dialog h2 {
    margin: 0 0 8px;
    font-size: 1rem;
  }

  .dialog p {
    margin: 0 0 14px;
    font-size: 0.85rem;
    color: var(--text-muted);
  }

  .dialog input {
    margin-bottom: 14px;
  }
</style>
