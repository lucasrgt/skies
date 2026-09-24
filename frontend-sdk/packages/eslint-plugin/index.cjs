"use strict";

const { version } = require("./package.json");

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
  "viewmodel-platform-agnostic": require("./rules/viewmodel-platform-agnostic.cjs"), // SKYFE009
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

const recommended = {
  name: "skies/recommended",
  plugins: { skies: plugin },
  rules: Object.fromEntries(Object.keys(rules).map((rule) => [`skies/${rule}`, "warn"])),
};

plugin.configs.recommended = recommended;
plugin.configs["flat/recommended"] = recommended;

module.exports = plugin;
