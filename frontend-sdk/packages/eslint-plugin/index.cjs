"use strict";

const { version } = require("./package.json");
// The accessibility floor ships inside recommended, resolved from this package (a dependency, not a peer), so an app
// that extends Skies' recommended gets jsx-a11y without wiring it.
const jsxA11y = require("eslint-plugin-jsx-a11y");

// @skiesjs/eslint-plugin — the SKYFE architecture rules. The front-side parallel of the backend's Roslyn analyzers
// (Skies.Framework.Doctor): the View renders, the ViewModel is the only data door, no mock leaks into production,
// async state goes through <Resource>, copy goes through i18n, routing/session/forms follow one seam each, and every
// test lives in a spec.
// Doctor-removable: delete the plugin and the app still builds; you only lose enforcement.
//
// One file per rule under rules/; the rule id (the object key) is stable and the SKYFE code lives in its messages.

const rules = {
  "view-purity": require("./rules/view-purity.cjs"), // SKYFE001
  "data-door": require("./rules/data-door.cjs"), // SKYFE002
  "no-mock": require("./rules/no-mock.cjs"), // SKYFE003
  "state-completeness": require("./rules/state-completeness.cjs"), // SKYFE010
  "i18n-completeness": require("./rules/i18n-completeness.cjs"), // SKYFE011
  "mutation-error-handled": require("./rules/mutation-error-handled.cjs"), // SKYFE013
  "no-hardcoded-copy": require("./rules/no-hardcoded-copy.cjs"), // SKYFE014
  "no-router-replace-in-effect": require("./rules/no-router-replace-in-effect.cjs"), // SKYFE015
  "session-one-door": require("./rules/session-one-door.cjs"), // SKYFE016
  "guard-tristate": require("./rules/guard-tristate.cjs"), // SKYFE017
  "route-param-guard": require("./rules/route-param-guard.cjs"), // SKYFE018
  "safe-back": require("./rules/safe-back.cjs"), // SKYFE019
  "no-hardcoded-base-url": require("./rules/no-hardcoded-base-url.cjs"), // SKYFE020
  "no-raw-html": require("./rules/no-raw-html.cjs"), // SKYFE021
  "no-open-redirect": require("./rules/no-open-redirect.cjs"), // SKYFE022
  "query-client-defaults": require("./rules/query-client-defaults.cjs"), // SKYFE027
  "no-manual-refetch-ritual": require("./rules/no-manual-refetch-ritual.cjs"), // SKYFE028
  "refresh-one-door": require("./rules/refresh-one-door.cjs"), // SKYFE029
  "no-cast-navigation": require("./rules/no-cast-navigation.cjs"), // SKYFE030
  "submit-handles-invalid": require("./rules/submit-handles-invalid.cjs"), // SKYFE031
  "controller-field-state": require("./rules/controller-field-state.cjs"), // SKYFE032
  "tests-live-in-specs": require("./rules/tests-live-in-specs.cjs"), // SKYFE036
};

const plugin = {
  meta: { name: "@skiesjs/eslint-plugin", version },
  rules,
  configs: {},
};

// The accessibility floor: jsx-a11y's recommended set at error, on by default in the same spirit as the CA* security
// floor the .NET doctor ships. An app relaxes one rule explicitly in its own config ("jsx-a11y/<rule>": "off").
// aria-role checks DOM elements only: a component prop named `role` (a typography or layout role) is not ARIA.
const a11yRules = Object.fromEntries(
  Object.entries(jsxA11y.flatConfigs.recommended.rules).map(([rule, setting]) => [rule, promote(setting)]),
);
a11yRules["jsx-a11y/aria-role"] = ["error", { ignoreNonDOM: true }];
// A label's text is looked for three levels deep, not jsx-a11y's default two: `<label><input/><span><strong>{name}`
// is an accessible card-style option that the default depth reports as text-less (seen calibrating on a real app).
a11yRules["jsx-a11y/label-has-associated-control"] = ["error", { depth: 3 }];

/** Raises a "warn" setting to "error", keeping "off" and the rule's options. */
function promote(setting) {
  const [severity, ...options] = Array.isArray(setting) ? setting : [setting];
  const raised = severity === "warn" || severity === 1 ? "error" : severity;
  return options.length > 0 ? [raised, ...options] : raised;
}

// Architecture rules are errors: each guards a shape whose drift ships a bug (a mocked screen, an open redirect, a
// silent failure). The polish rules stay warnings: a redundant refetch (SKYFE028) is harmless, and a single-screen
// form with every error visible inline is a legitimate reason to skip submitOrReveal (SKYFE031/032).
const WARN_TIER = new Set(["no-manual-refetch-ritual", "submit-handles-invalid", "controller-field-state"]);

const recommended = {
  name: "skies/recommended",
  plugins: { skies: plugin, "jsx-a11y": jsxA11y },
  rules: {
    ...Object.fromEntries(
      Object.keys(rules).map((rule) => [`skies/${rule}`, WARN_TIER.has(rule) ? "warn" : "error"]),
    ),
    ...a11yRules,
  },
};

plugin.configs.recommended = recommended;
plugin.configs["flat/recommended"] = recommended;

module.exports = plugin;
