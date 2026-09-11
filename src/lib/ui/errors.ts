/**
 * Turning what the core refused into a sentence the user can read.
 *
 * The core sends a kind and the details; the language file writes the sentence.
 * Anything that is not one of the core's shapes still has to say something, so
 * the last line falls back rather than showing nothing.
 */

import type { CoreError } from "../bridge";
import { t } from "../i18n/index.svelte";

export function describe(failure: unknown): string {
  const error = failure as CoreError | undefined;
  if (!error || typeof error !== "object" || !("kind" in error)) {
    return t("error.other", { detail: String(failure) });
  }

  switch (error.kind) {
    case "unreachable":
      return t("error.unreachable", { host: error.host, port: error.port });
    case "authentication-failed":
      return t("error.authentication-failed", { user: error.user });
    case "key-file":
      return t("error.key-file", { path: error.path });
    case "key-passphrase":
      return t("error.key-passphrase", { path: error.path });
    case "agent":
      return t("error.agent");
    case "host-key-unknown":
      return t("error.host-key-unknown", { host: error.host });
    case "host-key-changed":
      return t("error.host-key-changed", { host: error.host });
    case "certificate-untrusted":
      return t("error.certificate-untrusted", { host: error.host });
    case "encryption-refused":
      return t("error.encryption-refused");
    case "path":
      return t(`error.path.${error.reason}`, { path: error.path });
    case "source-changed":
      return t("error.source-changed");
    case "disconnected":
      return t("error.disconnected");
    case "not-connected":
      return t("error.not-connected");
    default:
      return t("error.other", { detail: JSON.stringify(error) });
  }
}

/** Whether a failure is the core asking about a server's certificate. */
export function certificateQuestion(
  failure: unknown,
): Extract<CoreError, { kind: "certificate-untrusted" }> | null {
  const error = failure as CoreError | undefined;
  if (!error || typeof error !== "object" || !("kind" in error)) return null;
  return error.kind === "certificate-untrusted" ? error : null;
}

/** Whether a failure is the core asking about a server key. */
export function hostKeyQuestion(
  failure: unknown,
): Extract<CoreError, { kind: "host-key-unknown" | "host-key-changed" }> | null {
  const error = failure as CoreError | undefined;
  if (!error || typeof error !== "object" || !("kind" in error)) return null;
  return error.kind === "host-key-unknown" || error.kind === "host-key-changed" ? error : null;
}
