#!/usr/bin/env node
/**
 * Checks the keyboard table for the things no type can.
 *
 * Two actions in one scheme answering to the same key is not a compile error;
 * it is a key that does one of two things depending on the order they happen to
 * be listed in. And an action with no name in the catalogues is a row in the
 * settings window with nothing written on it.
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

for (const scheme of SCHEMES) {
  const bindings = schemeBindings(scheme);
  const seen = new Map();

  for (const action of Object.keys(bindings)) {
    if (!ACTIONS.includes(action)) {
      complain(`scheme "${scheme}" binds "${action}", which is not an action.`);
    }
    for (const binding of bindings[action] ?? []) {
      const already = seen.get(binding);
      if (already) {
        complain(`scheme "${scheme}": ${binding} means both "${already}" and "${action}".`);
      }
      seen.set(binding, action);
      if (label(binding) === "") {
        complain(`scheme "${scheme}": ${binding} has nothing to show for itself.`);
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
  { key: "F5", metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, expect: "F5" },
  { key: "s", metaKey: true, ctrlKey: false, altKey: false, shiftKey: false, expect: "Meta+S" },
  { key: "S", metaKey: true, ctrlKey: false, altKey: false, shiftKey: true, expect: "Meta+Shift+S" },
  { key: " ", metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, expect: "Space" },
  { key: ",", metaKey: true, ctrlKey: false, altKey: false, shiftKey: false, expect: "Meta+," },
];
for (const sample of samples) {
  const written = bindingOf(sample);
  if (written !== sample.expect) {
    complain(`"${sample.key}" was written as ${written}, expected ${sample.expect}.`);
  }
}

// And the whole point: a press finds its action.
const classic = resolve("classic", {});
if (actionFor(classic, "F5") !== "refresh") {
  complain("F5 does not refresh in the classic scheme.");
}
if (actionFor(classic, "Meta+Shift+Z") !== null) {
  complain("a key nobody bound found an action anyway.");
}

if (failed) {
  process.exit(1);
}
console.log(`keys in order: ${SCHEMES.length} schemes, ${ACTIONS.length} actions`);
