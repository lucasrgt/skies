import assert from "node:assert/strict";
import test from "node:test";

import { renderFlutterHarness, validateFlutterProject } from "./app-scaffold.mjs";

test("renders the conventional Flutter package scripts and empty closed flow inventory", () => {
  const files = renderFlutterHarness();
  const packageJson = JSON.parse(files["package.json"]);

  assert.equal(packageJson.scripts.test, "flutter test --exclude-tags=avp");
  assert.equal(packageJson.scripts["test:avp"], "flutter test --tags=avp");
  assert.equal(packageJson.scripts["test:e2e"], "flutter test integration_test");
  assert.equal(packageJson.devDependencies["skies-flutter"], "4.1.24");
  assert.equal(files["e2e/flows.json"], "[]\n");
});

test("rejects a directory that is not a Flutter project", () => {
  assert.equal(validateFlutterProject("Z:/missing-skies-flutter-project"), "pubspec.yaml is missing");
});
