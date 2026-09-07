import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { auditProject, diagnose, SKYFL_RULES } from "./doctor.mjs";
import { renderFeature } from "./generate.mjs";

test("catalog maps every SKYFE semantic slot to SKYFL001 through SKYFL035", () => {
  assert.equal(SKYFL_RULES.length, 35);
  assert.equal(SKYFL_RULES[0].code, "SKYFL001");
  assert.equal(SKYFL_RULES[34].code, "SKYFL035");
  assert.equal(new Set(SKYFL_RULES.map((entry) => entry.name)).size, 35);
});

test("a complete scaffold plus explicit journeys passes the structural doctor", () => {
  const root = mkdtempSync(join(tmpdir(), "skies-flutter-doctor-"));
  try {
    const files = renderFeature({
      name: "wallets",
      itemType: "WalletView",
      itemImport: "package:sample_api/sample_api.dart",
      appPackage: "sample_app",
      criteria: ["lists-wallets", "reveals-wallet-failure"],
    });
    writeFeature(root, "wallets", files);
    mkdirSync(join(root, "e2e"), { recursive: true });
    writeFileSync(join(root, "e2e", "flows.json"), JSON.stringify([
      { id: "wallets-happy", features: ["Wallets"], path: "happy" },
      { id: "wallets-sad", features: ["Wallets"], path: "sad" },
    ]));

    assert.deepEqual(diagnose(root), []);

    const assay = join(root, "test", "features", "wallets", "wallets.assay_test.dart");
    writeFileSync(assay, "import 'package:flutter_test/flutter_test.dart';\nvoid main() {\n  // @avp lists-wallets\n  test('lists-wallets', () {});\n  // @avp reveals-wallet-failure\n  test('reveals-wallet-failure', () {});\n}\n");
    assert.ok(diagnose(root).some((finding) => finding.rule === "SKYFL033"));
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("reports architecture, state, proof, and evidence gaps by their parity IDs", () => {
  const root = mkdtempSync(join(tmpdir(), "skies-flutter-doctor-"));
  try {
    const lib = join(root, "lib", "features", "wallets");
    mkdirSync(lib, { recursive: true });
    writeFileSync(join(lib, "wallets_view.dart"), "import 'package:dio/dio.dart';\nText('hardcoded');\n");
    writeFileSync(join(lib, "wallets_view_model.dart"), "final class WalletsViewModel {}\n");

    const rules = new Set(diagnose(root).map((finding) => finding.rule));
    for (const expected of ["SKYFL001", "SKYFL005", "SKYFL006", "SKYFL007", "SKYFL014", "SKYFL024", "SKYFL033", "SKYFL035"]) {
      assert.ok(rules.has(expected), `${expected} was not reported`);
    }
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("reports production mocks, unfinished placeholders, raw colors, and disabled tests", () => {
  const root = mkdtempSync(join(tmpdir(), "skies-flutter-doctor-"));
  try {
    const lib = join(root, "lib", "helpers");
    const tests = join(root, "test");
    mkdirSync(lib, { recursive: true });
    mkdirSync(tests, { recursive: true });
    writeFileSync(join(lib, "helper.dart"), "import 'package:mocktail/mocktail.dart';\n// TODO wire later\nfinal c = Color(0xff112233);\n");
    writeFileSync(join(tests, "helper_test.dart"), "test('later', () {}, skip: true);\n");

    const rules = new Set(diagnose(root).map((finding) => finding.rule));
    for (const expected of ["SKYFL003", "SKYFL023", "SKYFL026", "SKYFL034"]) {
      assert.ok(rules.has(expected), `${expected} was not reported`);
    }
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("strict mode promotes endpoint coverage warnings to release errors", () => {
  const root = mkdtempSync(join(tmpdir(), "skies-flutter-doctor-"));
  try {
    mkdirSync(join(root, "lib", "features"), { recursive: true });
    const interactive = diagnose(root, { operationIds: ["ListWallets"] });
    const release = diagnose(root, { operationIds: ["ListWallets"], strict: true });
    assert.equal(interactive[0].rule, "SKYFL008");
    assert.equal(interactive[0].severity, "warning");
    assert.equal(release[0].severity, "error");
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("endpoint coverage accepts sanctioned Flutter infrastructure doors", () => {
  const root = mkdtempSync(join(tmpdir(), "skies-flutter-doctor-"));
  try {
    mkdirSync(join(root, "lib"), { recursive: true });
    writeFileSync(join(root, "lib", "session.dart"), "void wire(api) => api.refreshSession();\n");
    assert.equal(diagnose(root, { operationIds: ["RefreshSession"], strict: true })
      .some((finding) => finding.rule === "SKYFL008"), false);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("expected mutation failures require a modeled local surface", () => {
  const root = mkdtempSync(join(tmpdir(), "skies-flutter-doctor-"));
  try {
    const lib = join(root, "lib", "features", "sign_in");
    mkdirSync(lib, { recursive: true });
    writeFileSync(join(lib, "sign_in_view_model.dart"), "final class SignInViewModel { Future<void> submit() => boundary.run(() async {}, successMessage: 'signed in', expectedFailure: true); }\n");
    assert.ok(diagnose(root).some((finding) => finding.rule === "SKYFL013" && finding.message.includes("expected failure")));
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("the unified strict doctor closes rules, endpoint, AVP, and real-backend E2E", () => {
  const root = mkdtempSync(join(tmpdir(), "skies-flutter-doctor-"));
  try {
    const files = renderFeature({
      name: "wallets",
      itemType: "WalletView",
      itemImport: "package:sample_api/sample_api.dart",
      appPackage: "sample_app",
      criteria: ["lists-wallets", "reveals-wallet-failure"],
    });
    files.lib["wallets_view_model.dart"] += "\nvoid generatedWire(api) => api.listWallets();\n";
    writeFeature(root, "wallets", files);
    mkdirSync(join(root, "e2e"), { recursive: true });
    mkdirSync(join(root, "integration_test"), { recursive: true });
    mkdirSync(join(root, "contract"), { recursive: true });
    const contract = { openapi: "3.1.0", paths: { "/wallets": { get: { operationId: "ListWallets", responses: {} } } } };
    writeFileSync(join(root, "contract", "api.json"), JSON.stringify(contract));
    writeFileSync(join(root, "package.json"), JSON.stringify({ scripts: { "test:e2e": "flutter test integration_test" } }));
    const flows = [
      { id: "wallets-happy", features: ["Wallets"], path: "happy", target: "native", spec: "integration_test/wallets_happy_test.dart", terminal: "wallets-ready", criteria: [{ id: "lists-wallets", evidence: "wallet-row" }], backendSlices: ["ListWallets"], backendContract: "contract/api.json" },
      { id: "wallets-sad", features: ["Wallets"], path: "sad", target: "native", spec: "integration_test/wallets_sad_test.dart", terminal: "wallets-error", criteria: [{ id: "reveals-wallet-failure", evidence: "retry-wallets" }], backendSlices: ["ListWallets"], backendContract: "contract/api.json" },
    ];
    writeFileSync(join(root, "e2e", "flows.json"), JSON.stringify(flows));
    for (const flow of flows) {
      const outcome = flow.path === "happy" ? "success" : "error";
      writeFileSync(join(root, flow.spec), `IntegrationTestWidgetsFlutterBinding.ensureInitialized();\ntestWidgets('${flow.id}', (tester) async { BackendLedgerInterceptor([]); expect(find.byKey(Key('${flow.terminal}')), findsOneWidget); expect(find.byKey(Key('${flow.criteria[0].evidence}')), findsOneWidget); ledger.expectSlices(['ListWallets'], outcome: BackendOutcome.${outcome}); });\n`);
    }

    assert.deepEqual(auditProject(root, { strict: true, contractFile: join(root, "contract", "api.json") }), []);
    assert.deepEqual(auditProject(root, { strict: true }), []);
    const reference = join(root, "backend-api.json");
    writeFileSync(reference, JSON.stringify({ paths: {
      ...contract.paths,
      "/admin": { get: { operationId: "ListAdminUsers" } },
      "/session": { get: { operationId: "GetSession" } },
    } }));
    for (const flow of flows) flow.backendContract = "backend-api.json";
    writeFileSync(join(root, "e2e", "flows.json"), JSON.stringify(flows));
    assert.deepEqual(auditProject(root, { strict: true }), []);
    const modelPath = join(root, "lib", "features", "wallets", "wallets_view_model.dart");
    const model = readFileSync(modelPath, "utf8");
    writeFileSync(modelPath, `${model}\nvoid sessionWire(api) => api.getSession();\n`);
    assert.ok(auditProject(root, { strict: true }).some((finding) =>
      finding.rule === "SKYFL035" && finding.message.includes("GetSession")));
    writeFileSync(modelPath, model);
    const firstBackend = join(root, "backend-one");
    const secondBackend = join(root, "backend-two");
    mkdirSync(firstBackend);
    mkdirSync(secondBackend);
    writeFileSync(join(firstBackend, "Wallets.cs"), "[Slice]\npublic static class ListWallets { static void Map() => MapPost(); }\n[Journey(typeof(ListWallets), JourneyPath.Happy)]\npublic class Happy {}\n");
    writeFileSync(join(secondBackend, "WalletsTests.cs"), "[Slice]\npublic static class ReadOnlyProbe {}\n[Journey(typeof(ListWallets), JourneyPath.Sad)]\npublic class Sad {}\n");
    assert.deepEqual(auditProject(root, { strict: true, backendRoots: [firstBackend, secondBackend] }), []);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

function writeFeature(root, name, files) {
  const lib = join(root, "lib", "features", name);
  const tests = join(root, "test", "features", name);
  const l10n = join(root, "lib", "l10n", "features");
  mkdirSync(lib, { recursive: true });
  mkdirSync(tests, { recursive: true });
  mkdirSync(l10n, { recursive: true });
  for (const [file, source] of Object.entries(files.lib)) writeFileSync(join(lib, file), source);
  for (const [file, source] of Object.entries(files.test)) writeFileSync(join(tests, file), source);
  for (const [file, source] of Object.entries(files.l10n)) writeFileSync(join(l10n, file), source);
}
