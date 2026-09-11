<script lang="ts">
  /**
   * The server log, and the line you type into it.
   *
   * Raw commands belong here rather than in a window of their own: what somebody
   * types and what the server answers are the same conversation, and splitting
   * them across two places makes both harder to read.
   */
  import { api, LOCAL, type Protocol } from "../bridge";
  import { clearLog, logLines } from "../state/log.svelte";
  import { t } from "../i18n/index.svelte";
  import { formatTime } from "./format";

  interface Props {
    /** Which endpoint a typed command goes to, and what it speaks. */
    endpoint?: string;
    protocol?: Protocol;
    /** Opened by the raw command key, and closed by it again. */
    raw?: boolean;
    /** Escape closes the line. The key that opened it cannot, once the cursor
     *  is in a field — keys in fields belong to the field. */
    onclose?: () => void;
  }

  let { endpoint = LOCAL, protocol = "local", raw = false, onclose }: Props = $props();

  let scroller = $state<HTMLDivElement | null>(null);
  let typed = $state("");
  let history = $state<string[]>([]);
  /** Where in the history the up arrow has walked to. */
  let back = $state(-1);
  let sending = $state(false);
  let field = $state<HTMLInputElement | null>(null);

  /**
   * Only FTP has raw commands. SSH carries a file transfer protocol with typed
   * operations, not a conversation in text, so there is nothing to type into —
   * and a box that never answers would be worse than none.
   */
  let canType = $derived(protocol === "ftp" || protocol === "ftps");

  // The cursor goes into the line when the key opens it. A command line one has
  // to click into first is a command line for the mouse.
  $effect(() => {
    if (raw && canType) queueMicrotask(() => field?.focus());
  });

  async function send(): Promise<void> {
    const command = typed.trim();
    if (!command || sending) return;
    sending = true;
    try {
      await api.rawCommand(endpoint, command);
      // Both the command and the answer are already in the log — the core put
      // them there as it sent and received them, which is where they belong.
      history = [command, ...history.filter((old) => old !== command)].slice(0, 50);
      back = -1;
      typed = "";
      follow = true;
    } catch {
      // The failure is in the log too. Nothing to add here.
    } finally {
      sending = false;
    }
  }

  function onKey(event: KeyboardEvent): void {
    if (event.key === "Escape") {
      event.preventDefault();
      onclose?.();
      return;
    }
    if (event.key === "Enter") {
      event.preventDefault();
      void send();
      return;
    }
    // The last commands, the way a shell offers them.
    if (event.key === "ArrowUp" && back + 1 < history.length) {
      event.preventDefault();
      back += 1;
      typed = history[back] ?? "";
    } else if (event.key === "ArrowDown" && back > -1) {
      event.preventDefault();
      back -= 1;
      typed = back === -1 ? "" : (history[back] ?? "");
    }
  }

  /**
   * Whether what is typed opens a data connection.
   *
   * Shown while typing rather than after sending: the point of the warning is
   * that somebody can still decide otherwise.
   */
  let opensData = $derived(
    ["RETR", "STOR", "STOU", "APPE", "LIST", "NLST", "MLSD", "PASV", "EPSV", "PORT", "EPRT"].includes(
      (typed.trim().split(/\s+/)[0] ?? "").toUpperCase(),
    ),
  );
  let lines = $derived(logLines());
  /** Off when the user scrolled up to read something. */
  let follow = $state(true);

  $effect(() => {
    void lines.length;
    if (follow && scroller) {
      scroller.scrollTop = scroller.scrollHeight;
    }
  });

  function onScroll(): void {
    if (!scroller) return;
    const distance = scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight;
    follow = distance < 24;
  }

  const arrow: Record<string, string> = {
    sent: "→",
    received: "←",
    note: "·",
  };

  /** Short tag for the pane a line came from. */
  function tag(endpoint: string): string {
    if (endpoint === LOCAL) return t("log.tag.local");
    if (endpoint.startsWith("left")) return t("log.tag.left");
    if (endpoint.startsWith("right")) return t("log.tag.right");
    return endpoint;
  }
</script>

<section class="log">
  <header>
    <span class="title">{t("log.title")}</span>
    <span class="spacer"></span>
    {#if !follow}
      <button type="button" onclick={() => (follow = true)}>{t("log.follow")}</button>
    {/if}
    <button type="button" onclick={clearLog}>{t("log.clear")}</button>
  </header>
  <div class="lines mono" bind:this={scroller} onscroll={onScroll}>
    {#each lines as line (line.id)}
      <div class="line {line.direction}">
        <span class="time">{formatTime(line.at)}</span>
        <span class="who">{tag(line.endpoint)}</span>
        <span class="arrow">{arrow[line.direction]}</span>
        <span class="text">{line.text}</span>
      </div>
    {:else}
      <p class="empty">{t("log.empty")}</p>
    {/each}
  </div>

  {#if raw}
    <div class="raw">
      {#if canType}
        <span class="prompt mono">›</span>
        <input
          class="mono"
          bind:this={field}
          bind:value={typed}
          onkeydown={onKey}
          placeholder={t("raw.placeholder")}
          autocomplete="off"
          autocapitalize="off"
          autocorrect="off"
          spellcheck="false"
          disabled={sending}
        />
        <button type="button" onclick={() => void send()} disabled={sending || typed.trim() === ""}>
          {t("raw.send")}
        </button>
      {:else}
        <p class="why">{t("raw.not-here")}</p>
      {/if}
    </div>
    {#if opensData}
      <p class="warn">{t("raw.data-warning")}</p>
    {/if}
  {/if}
</section>

<style>
  .raw {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 8px;
    border-top: 1px solid var(--border);
    background: var(--surface-1);
  }

  .raw .prompt {
    color: var(--accent);
    font-size: 0.85rem;
  }

  .raw input {
    flex: 1;
    min-width: 0;
    font-size: 0.8rem;
    padding: 4px 8px;
    border-radius: 0.4rem;
    border: 1px solid var(--border-strong);
    background: var(--surface-0);
    color: var(--text);
  }

  .raw button {
    font: inherit;
    font-size: 0.78rem;
    padding: 4px 12px;
    border-radius: 999px;
    border: 1px solid var(--border-strong);
    background: var(--surface-1);
    color: var(--text);
    cursor: default;
  }

  .raw button:disabled {
    opacity: 0.4;
  }

  .why {
    margin: 0;
    font-size: 0.78rem;
    color: var(--text-faint);
  }

  .warn {
    margin: 0;
    padding: 5px 10px;
    font-size: 0.76rem;
    color: var(--warn);
    background: var(--warn-soft);
  }

  .log {
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: var(--surface-1);
  }

  header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 3px 10px;
    background: var(--surface-2);
    border-bottom: 1px solid var(--border);
  }

  .title {
    font-size: 0.66rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-faint);
    font-weight: 600;
  }

  .spacer {
    flex: 1;
  }

  header button {
    font: inherit;
    font-size: 0.7rem;
    border: none;
    background: none;
    color: var(--text-faint);
    cursor: default;
    padding: 2px 6px;
    border-radius: 4px;
  }

  header button:hover {
    background: var(--surface-3);
    color: var(--text-muted);
  }

  .lines {
    flex: 1;
    overflow: auto;
    padding: 4px 0;
  }

  .line {
    display: grid;
    grid-template-columns: 72px 28px 16px 1fr;
    gap: 4px;
    padding: 0 10px;
    font-size: 0.74rem;
    line-height: 1.5;
  }

  .time {
    color: var(--text-faint);
  }

  .who {
    color: var(--text-faint);
    text-align: right;
  }

  .arrow {
    color: var(--accent);
    text-align: center;
  }

  .text {
    color: var(--text-muted);
    white-space: pre-wrap;
    word-break: break-word;
  }

  .line.received .text {
    color: var(--text);
  }

  .line.note .text {
    color: var(--text-faint);
  }

  .line.note .arrow {
    color: var(--text-faint);
  }

  .empty {
    margin: 0;
    padding: 8px 10px;
    color: var(--text-faint);
    font-size: 0.74rem;
  }
</style>
