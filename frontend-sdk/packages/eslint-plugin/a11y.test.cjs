"use strict";

// The accessibility floor: recommended carries jsx-a11y's recommended set at error, resolved from this package, so an
// app that extends Skies' recommended gets a11y without wiring it. Pinned two ways: the config shape (every jsx-a11y
// recommended rule present, none below error) and a real lint run through ESLint's Linter.

const { Linter } = require("eslint");
const assert = require("node:assert/strict");
const tsParser = require("@typescript-eslint/parser");
const jsxA11y = require("eslint-plugin-jsx-a11y");
const plugin = require("./index.cjs");

const recommended = plugin.configs.recommended;
const severity = (setting) => (Array.isArray(setting) ? setting[0] : setting);

assert.equal(recommended.plugins["jsx-a11y"], jsxA11y, "recommended registers jsx-a11y from this package");
assert.ok(
  require("./package.json").dependencies["eslint-plugin-jsx-a11y"],
  "jsx-a11y is a dependency, not a peer the app must remember",
);

const upstream = jsxA11y.flatConfigs.recommended.rules;
for (const [rule, setting] of Object.entries(upstream)) {
  assert.ok(rule in recommended.rules, `${rule} is in recommended`);
  const expected = severity(setting) === "off" || severity(setting) === 0 ? "off" : "error";
  assert.equal(severity(recommended.rules[rule]), expected, `${rule} is at ${expected}`);
}
const enabled = Object.entries(recommended.rules).filter(
  ([rule, setting]) => rule.startsWith("jsx-a11y/") && severity(setting) === "error",
);
assert.ok(enabled.length >= 30, `the a11y floor enables the jsx-a11y recommended set (${enabled.length} rules)`);
assert.deepEqual(recommended.rules["jsx-a11y/aria-role"], ["error", { ignoreNonDOM: true }], "aria-role ignores non-DOM");

// A real run: the floor fires on DOM a11y violations and stays quiet on the accessible shapes.
const linter = new Linter({ configType: "flat" });
const config = [
  {
    ...recommended,
    files: ["**/*.tsx"],
    languageOptions: { parser: tsParser, parserOptions: { ecmaFeatures: { jsx: true } } },
  },
];
const lint = (code) =>
  linter
    .verify(code, config, { filename: "Screen.tsx" })
    .filter((message) => message.ruleId?.startsWith("jsx-a11y/"))
    .map((message) => [message.ruleId, message.severity]);

assert.deepEqual(lint(`export const A = () => <img src="/logo.png" />;`), [["jsx-a11y/alt-text", 2]]);
assert.deepEqual(lint(`export const A = () => <a>go</a>;`), [["jsx-a11y/anchor-is-valid", 2]]);
assert.deepEqual(
  lint(`export const A = ({ f }: { f: () => void }) => <div onClick={f}>x</div>;`).map(([rule]) => rule).sort(),
  ["jsx-a11y/click-events-have-key-events", "jsx-a11y/no-static-element-interactions"],
);
assert.deepEqual(lint(`export const A = () => <div role="buton" />;`), [["jsx-a11y/aria-role", 2]]);
assert.deepEqual(lint(`export const A = () => <img src="/logo.png" alt="Skies" />;`), []);
assert.deepEqual(lint(`export const A = ({ f }: { f: () => void }) => <button onClick={f}>go</button>;`), []);
// ignoreNonDOM: a component prop named `role` is the component's business, not ARIA.
assert.deepEqual(lint(`export const A = () => <Text role="caption">x</Text>;`), []);

// An app relaxes one rule explicitly, after recommended.
const relaxed = [...config, { files: ["**/*.tsx"], rules: { "jsx-a11y/alt-text": "off" } }];
assert.deepEqual(
  linter.verify(`export const A = () => <img src="/logo.png" />;`, relaxed, { filename: "Screen.tsx" }),
  [],
);
