<script lang="ts">
  import {
    api,
    type AuthKind,
    type ConnectRequest,
    type Protocol,
    type QuickConnectEntry,
  } from "../bridge";
  import { t } from "../i18n/index.svelte";
  import { describe } from "./errors";

  interface Props {
    /** Which pane the connection is for. */
    endpoint: string;
    onconnect: (request: ConnectRequest, historyId: string) => Promise<void>;
    onclose: () => void;
    /** Set while an attempt is running, so the form can say so. */
    busy: boolean;
    failure: unknown;
  }

  let { endpoint, onconnect, onclose, busy, failure }: Props = $props();

  /**
   * The four a person actually chooses between. FTPS is two entries rather than
   * one with a switch, because explicit and implicit differ in the port and in
   * what happens before the first byte — not in a setting one would go looking
   * for afterwards.
   */
  const KINDS = [
    { id: "sftp", protocol: "sftp", port: 22 },
    { id: "ftps-explicit", protocol: "ftps", port: 21, encryption: "explicit" },
    { id: "ftps-implicit", protocol: "ftps", port: 990, encryption: "implicit" },
    { id: "ftp", protocol: "ftp", port: 21 },
  ] as const;

  type Kind = (typeof KINDS)[number];

  let kind = $state<Kind>(KINDS[0]);
  let host = $state("");
  let port = $state(22);
  let user = $state("");
  let auth = $state<AuthKind>("password");
  let password = $state("");
  let keyPath = $state("");
  let passphrase = $state("");

  let passive = $state(true);
  let latin1 = $state(false);

  /** FTP has no keys and no agent; offering them would be a dead end. */
  let remote = $derived(kind.protocol !== "sftp");

  /**
   * Changing the protocol moves the port with it, but only while the port is
   * still the one the previous choice suggested. A port typed by hand is the
   * user's, and nothing here overwrites that.
   */
  function choose(next: Kind): void {
    if (port === kind.port) port = next.port;
    if (next.protocol !== "sftp" && auth !== "password") auth = "password";
    kind = next;
  }

  /** Per connection, empty meaning "whatever the settings say". */
  let concurrency = $state("");
  let retries = $state("");
  let temporaryName = $state<boolean | null>(null);
  let advanced = $state(false);

  let history = $state<QuickConnectEntry[]>([]);
  let note = $state<string | null>(null);

  $effect(() => {
    void api.quickConnectHistory().then((entries) => (history = entries));
  });

  function idFor(): string {
    return `${user}@${host}:${port}`;
  }

  async function submit(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    if (!host || !user) return;
    await onconnect(
      {
        endpoint,
        host,
        port,
        user,
        auth,
        password: auth === "password" ? password : undefined,
        keyPath: auth === "key-file" ? keyPath : undefined,
        passphrase: auth === "key-file" && passphrase ? passphrase : undefined,
        concurrency: numberOrNothing(concurrency),
        retries: numberOrNothing(retries),
        temporaryName: temporaryName ?? undefined,
        protocol: kind.protocol as Protocol,
        encryption: "encryption" in kind ? kind.encryption : undefined,
        passive: remote ? passive : undefined,
        latin1: remote ? latin1 : undefined,
      },
      idFor(),
    );
  }

  /** Empty means "not set here", which lets the settings answer instead. */
  function numberOrNothing(text: string): number | undefined {
    const parsed = Number.parseInt(text.trim(), 10);
    return Number.isNaN(parsed) ? undefined : parsed;
  }

  /** Fills the form from a history entry. The password is never there. */
  function fill(entry: QuickConnectEntry): void {
    // Implicit and explicit share a protocol, and the port does not tell them
    // apart on a server that listens somewhere of its own choosing. The entry
    // says which it was; the port is only the fallback for entries written
    // before it did.
    kind =
      KINDS.find(
        (candidate) =>
          candidate.protocol === entry.protocol &&
          ("encryption" in candidate ? candidate.encryption : null) ===
            (entry.protocol === "ftps" ? entry.encryption : null),
      ) ??
      KINDS.find(
        (candidate) =>
          candidate.protocol === entry.protocol &&
          (entry.protocol !== "ftps" || candidate.port === entry.port),
      ) ??
      KINDS.find((candidate) => candidate.protocol === entry.protocol) ??
      KINDS[0];
    passive = entry.passive ?? true;
    latin1 = entry.latin1 ?? false;
    host = entry.host;
    port = entry.port;
    user = entry.user;
    auth = entry.auth;
    keyPath = entry.keyPath ?? "";
    password = "";
    passphrase = "";
    concurrency = entry.concurrency === null ? "" : String(entry.concurrency);
    retries = entry.retries === null ? "" : String(entry.retries);
    temporaryName = entry.temporaryName;
    advanced =
      entry.concurrency !== null ||
      entry.retries !== null ||
      entry.temporaryName !== null ||
      entry.passive === false ||
      entry.latin1 === true;
  }

  async function forget(entry: QuickConnectEntry): Promise<void> {
    await api.forgetQuickConnect(entry.id);
    history = await api.quickConnectHistory();
  }

  async function toSite(entry: QuickConnectEntry): Promise<void> {
    const path = await api.saveAsSite(entry.id);
    history = await api.quickConnectHistory();
    note = t("quick.saved", { path });
  }
</script>

<div class="backdrop" role="presentation">
  <div class="dialog" role="dialog" aria-modal="true" aria-label={t("quick.title")}>
    <header>
      <h2>{t("quick.title")}</h2>
      <button type="button" class="close" onclick={onclose} aria-label={t("action.cancel")}>×</button>
    </header>

    <div class="body">
      <form onsubmit={submit}>
        <fieldset>
          <legend>{t("quick.protocol")}</legend>
          <div class="choices">
            {#each KINDS as candidate (candidate.id)}
              <button
                type="button"
                class:active={kind.id === candidate.id}
                onclick={() => choose(candidate)}
              >
                {t(`protocol.${candidate.id}`)}
              </button>
            {/each}
          </div>
        </fieldset>

        {#if kind.id === "ftp"}
          <p class="failure">{t("protocol.ftp.warning")}</p>
        {/if}

        <div class="row">
          <label class="grow">
            <span>{t("quick.host")}</span>
            <input bind:value={host} placeholder="beispiel.de" autocomplete="off" autocapitalize="off" autocorrect="off" spellcheck="false" required />
          </label>
          <label class="port">
            <span>{t("quick.port")}</span>
            <input type="number" bind:value={port} min="1" max="65535" autocomplete="off" />
          </label>
        </div>

        <label>
          <span>{t("quick.user")}</span>
          <input bind:value={user} autocomplete="off" autocapitalize="off" autocorrect="off" spellcheck="false" required />
        </label>

        {#if !remote}
          <fieldset>
            <legend>{t("quick.auth")}</legend>
            <div class="choices">
              {#each ["password", "key-file", "agent"] as const as method (method)}
                <button
                  type="button"
                  class:active={auth === method}
                  onclick={() => (auth = method)}
                >
                  {t(`auth.${method}`)}
                </button>
              {/each}
            </div>
          </fieldset>
        {/if}

        {#if auth === "password"}
          <label>
            <span>{t("quick.password")}</span>
            <input type="password" bind:value={password} autocomplete="off" autocapitalize="off" autocorrect="off" spellcheck="false" />
          </label>
          <p class="hint">{t("quick.password.hint")}</p>
        {:else if auth === "key-file"}
          <label>
            <span>{t("quick.key-path")}</span>
            <input bind:value={keyPath} placeholder="~/.ssh/id_ed25519" autocomplete="off" autocapitalize="off" autocorrect="off" spellcheck="false" />
          </label>
          <label>
            <span>{t("quick.passphrase")}</span>
            <input type="password" bind:value={passphrase} autocomplete="off" autocapitalize="off" autocorrect="off" spellcheck="false" />
          </label>
        {:else}
          <p class="hint">{t("quick.agent.hint")}</p>
        {/if}

        <button type="button" class="more" onclick={() => (advanced = !advanced)}>
          {advanced ? "▾" : "▸"} {t("quick.advanced")}
        </button>

        {#if advanced}
          <div class="advanced">
            <label class="narrow">
              <span>{t("settings.concurrency")}</span>
              <input
                bind:value={concurrency}
                type="number"
                min="1"
                max="64"
                placeholder={t("quick.from-settings")}
                autocomplete="off"
              />
            </label>
            <label class="narrow">
              <span>{t("settings.retries")}</span>
              <input
                bind:value={retries}
                type="number"
                min="1"
                max="20"
                placeholder={t("quick.from-settings")}
                autocomplete="off"
              />
            </label>
            {#if remote}
              <label class="check">
                <input type="checkbox" bind:checked={passive} />
                <span>{t("quick.passive")}</span>
              </label>
              <label class="check">
                <input type="checkbox" bind:checked={latin1} />
                <span>{t("quick.latin1")}</span>
              </label>
            {/if}
            <div class="tri">
              <span>{t("settings.temporary-name")}</span>
              <div class="choices">
                {#each [null, true, false] as choice (String(choice))}
                  <button
                    type="button"
                    class:active={temporaryName === choice}
                    onclick={() => (temporaryName = choice)}
                  >
                    {choice === null
                      ? t("quick.from-settings")
                      : choice
                        ? t("quick.yes")
                        : t("quick.no")}
                  </button>
                {/each}
              </div>
            </div>
          </div>
        {/if}

        {#if failure}
          <p class="failure">{describe(failure)}</p>
        {/if}

        <div class="actions">
          <button type="button" onclick={onclose}>{t("action.cancel")}</button>
          <button type="submit" class="primary" disabled={busy || !host || !user}>
            {busy ? t("quick.connecting") : t("quick.connect")}
          </button>
        </div>
      </form>

      <aside>
        <h3>{t("quick.history")}</h3>
        {#if history.length === 0}
          <p class="hint">{t("quick.history.empty")}</p>
        {:else}
          <ul>
            {#each history as entry (entry.id)}
              <li>
                <button type="button" class="entry" onclick={() => fill(entry)}>
                  <span class="who">{entry.user}@{entry.host}</span>
                  <span class="detail mono">
                    :{entry.port} · {t(`auth.${entry.auth}`)}
                    {#if entry.savedAsSite}· {t("quick.is-site")}{/if}
                  </span>
                </button>
                <span class="entry-actions">
                  {#if !entry.savedAsSite}
                    <button type="button" title={t("quick.to-site")} onclick={() => toSite(entry)}>
                      ★
                    </button>
                  {/if}
                  <button type="button" title={t("quick.forget")} onclick={() => forget(entry)}>
                    ×
                  </button>
                </span>
              </li>
            {/each}
          </ul>
        {/if}
        {#if note}
          <p class="note">{note}</p>
        {/if}
      </aside>
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
    z-index: 40;
  }

  .dialog {
    width: min(760px, 94vw);
    max-height: 90vh;
    overflow: auto;
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
    font-size: 1rem;
    flex: 1;
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
    display: grid;
    grid-template-columns: 1fr 260px;
    gap: 0;
  }

  form {
    padding: 16px 18px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .row {
    display: flex;
    gap: 10px;
  }

  .grow {
    flex: 1;
  }

  .port {
    width: 92px;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  label span,
  legend {
    font-size: 0.68rem;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-faint);
  }

  input {
    font: inherit;
    font-size: 0.88rem;
    padding: 6px 9px;
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

  fieldset {
    border: none;
    padding: 0;
    margin: 0;
  }

  .choices {
    display: flex;
    gap: 6px;
    margin-top: 4px;
  }

  button {
    font: inherit;
    font-size: 0.84rem;
    padding: 5px 13px;
    border-radius: 999px;
    border: 1px solid var(--border-strong);
    background: var(--surface-1);
    color: var(--text);
    cursor: default;
  }

  button:hover {
    background: var(--surface-2);
  }

  button.active,
  button.primary {
    border-color: var(--accent);
    background: var(--accent-soft);
    color: var(--accent);
  }

  button:disabled {
    opacity: 0.5;
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 4px;
  }

  .hint {
    margin: 0;
    font-size: 0.78rem;
    color: var(--text-faint);
  }

  /* A row of its own with a visible edge: the settings behind it — how many
     transfers at once, how many attempts — are the ones people go looking for
     and do not find when they are hidden behind grey text. */
  .more {
    align-self: stretch;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    font-size: 0.82rem;
    border: 1px solid var(--border-strong);
    border-radius: 0.6rem;
    background: var(--surface-2);
    color: var(--text-muted);
  }

  .more:hover {
    border-color: var(--accent);
    background: var(--accent-soft);
    color: var(--accent);
  }

  .advanced {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px 12px;
    background: var(--surface-2);
    border-radius: 0.6rem;
  }

  .narrow,
  .check {
    display: flex;
    flex-direction: row;
    align-items: center;
    gap: 8px;
    grid-column: 1 / -1;
  }

  .check input {
    width: auto;
  }

  .check span {
    font-size: 0.82rem;
    color: var(--text-muted);
  }

  .tri {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }

  .narrow input {
    width: 110px;
    text-align: right;
  }

  .tri span,
  .narrow span {
    font-size: 0.68rem;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-faint);
  }

  .tri .choices button {
    font-size: 0.74rem;
    padding: 3px 9px;
  }

  .failure {
    margin: 0;
    font-size: 0.82rem;
    color: var(--danger);
    background: var(--danger-soft);
    padding: 8px 10px;
    border-radius: 0.5rem;
  }

  aside {
    border-left: 1px solid var(--border);
    background: var(--surface-2);
    padding: 16px 14px;
  }

  h3 {
    margin: 0 0 8px;
    font-size: 0.68rem;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-faint);
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  li {
    display: flex;
    align-items: center;
    gap: 2px;
  }

  .entry {
    flex: 1;
    border: none;
    background: none;
    text-align: left;
    padding: 5px 7px;
    border-radius: 0.5rem;
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }

  .who {
    font-size: 0.84rem;
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .detail {
    font-size: 0.7rem;
    color: var(--text-faint);
  }

  .entry-actions button {
    border: none;
    background: none;
    padding: 2px 5px;
    color: var(--text-faint);
    font-size: 0.9rem;
  }

  .entry-actions button:hover {
    color: var(--accent);
    background: var(--surface-3);
  }

  .note {
    margin: 10px 0 0;
    font-size: 0.74rem;
    color: var(--ok);
  }
</style>
