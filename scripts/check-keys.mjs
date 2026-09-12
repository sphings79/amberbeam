#!/usr/bin/env node
/**
 * Checks the keyboard table for the things no type can.
 *
 * Two actions in one scheme answering to the same key is not a compile error;
 * it is a key that does one of two things depending on the order they happen to
 * be listed in. And an action with no name in the catalogues is a row in the
 * settings window with nothing written on it.
 *
 * Every check runs for both keyboards, never for the one this happens to be
 * running on. A Mac checking only the Mac is how a shortcut shipped that asked
 * Windows users to press the Start menu.
 *
 * Node runs the TypeScript directly — there is no test framework in this
 * project and this does not need one.
 */

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const { ACTIONS, SCHEMES, schemeBindings, bindingOf, actionFor, label, resolve } = await import(
  join(here, "..", "src", "lib", "keys", "schemes.ts")
);

const catalogue = JSON.parse(
  readFileSync(join(here, "..", "src", "lib", "i18n", "en.json"), "utf8"),
);

let failed = false;

/** @param {string} message */
function complain(message) {
  console.error(message);
  failed = true;
}

const PLATFORMS = ["mac", "other"];

for (const platform of PLATFORMS) {
  for (const scheme of SCHEMES) {
    const bindings = schemeBindings(scheme, platform);
    const seen = new Map();

    for (const action of Object.keys(bindings)) {
      if (!ACTIONS.includes(action)) {
        complain(`scheme "${scheme}" binds "${action}", which is not an action.`);
      }
      for (const binding of bindings[action] ?? []) {
        const already = seen.get(binding);
        if (already) {
          complain(
            `${platform}, scheme "${scheme}": ${binding} means both "${already}" and "${action}".`,
          );
        }
        seen.set(binding, action);
        if (label(binding, platform) === "") {
          complain(`${platform}, scheme "${scheme}": ${binding} has nothing to show for itself.`);
        }
        // Away from a Mac the command modifier *is* Control, so a binding
        // naming both asks for one key twice and can never be pressed.
        if (platform === "other" && binding.includes("Mod+") && binding.includes("Control")) {
          complain(`scheme "${scheme}": ${binding} is unpressable where Mod is Control.`);
        }
      }
    }
  }
}

for (const action of ACTIONS) {
  const key = `action.${action}`;
  if (!(key in catalogue)) {
    complain(`action "${action}" has no name: "${key}" is in no catalogue.`);
  }
}

// The written form has to survive being read back, or a key somebody assigns is
// not the key that then answers.
const samples = [
  { key: "F5", metaKey: false, ctrlKey: false, altKey: false, shiftKey: false },
  { key: "s", metaKey: true, ctrlKey: true, altKey: false, shiftKey: false, expect: "Mod+S" },
  { key: "S", metaKey: true, ctrlKey: true, altKey: false, shiftKey: true, expect: "Mod+Shift+S" },
  { key: " ", metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, expect: "Space" },
  { key: ",", metaKey: true, ctrlKey: true, altKey: false, shiftKey: false, expect: "Mod+," },
];
for (const platform of PLATFORMS) {
  for (const sample of samples) {
    const expect = sample.expect ?? "F5";
    // Command on a Mac, Control elsewhere — the same press has to be written
    // down the same way on both, or the table only fits one of them.
    const written = bindingOf(
      { ...sample, metaKey: platform === "mac" ? sample.metaKey : false, ctrlKey: platform === "mac" ? false : sample.ctrlKey },
      platform,
    );
    if (written !== expect) {
      complain(`${platform}: "${sample.key}" was written as ${written}, expected ${expect}.`);
    }
  }

  // And the whole point: a press finds its action.
  const classic = resolve("classic", {}, platform);
  if (actionFor(classic, "F5") !== "refresh") {
    complain(`${platform}: F5 does not refresh in the classic scheme.`);
  }
  if (actionFor(classic, "Mod+Shift+Z") !== null) {
    complain(`${platform}: a key nobody bound found an action anyway.`);
  }
  if (actionFor(resolve("mac", {}, platform), "Mod+S") !== "sites") {
    complain(`${platform}: the servers window has no key.`);
  }
}

if (failed) {
  process.exit(1);
}
console.log(`keys in order: ${SCHEMES.length} schemes, ${ACTIONS.length} actions`);
