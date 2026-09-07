#!/usr/bin/env node

import { existsSync, readdirSync, readFileSync } from "node:fs";
import { basename, dirname, join, relative, resolve } from "node:path";
import { checkE2eProject } from "./e2e-doctor.mjs";
import { isDataDoor as isEndpointDataDoor, operationIds as readOperationIds } from "./endpoint-coverage.mjs";
import { checkFeatureE2e, findViewModels } from "./feature-e2e-coverage.mjs";
import { backendInventoryError, checkJourneyParity, readBackendJourneyInventory } from "./journey-parity.mjs";

/** The complete React-to-Flutter rule correspondence. IDs retain semantic meaning. */
export const SKYFL_RULES = Object.freeze([
  "view-purity", "data-door", "no-mock", "viewmodel-render-agnostic", "viewmodel-test",
  "view-integration-test", "mandatory-state", "endpoint-coverage", "viewmodel-platform-agnostic",
  "state-completeness", "i18n-parity", "design-tokens", "mutation-error-surface",
  "no-hardcoded-copy", "declarative-redirect", "session-one-door", "guard-tristate",
  "route-param-guard", "safe-back", "configured-base-url", "raw-html-one-door", "no-open-redirect",
  "no-placeholder", "ui-door", "scale-only", "semantic-colors", "mutation-defaults",
  "no-manual-refetch", "refresh-one-door", "typed-navigation", "submit-invalid-path",
  "field-error-surface", "avp-proof", "no-disabled-tests", "feature-e2e-flow",
].map((name, index) => Object.freeze({ code: `SKYFL${String(index + 1).padStart(3, "0")}`, name })));

const RULE = Object.freeze(Object.fromEntries(SKYFL_RULES.map((entry) => [entry.name, entry.code])));
const RAW_WIDGETS = [
  "Scaffold", "AppBar", "Text", "RichText", "ElevatedButton", "FilledButton", "OutlinedButton",
  "TextButton", "IconButton", "TextField", "TextFormField", "Container", "Padding", "Card", "ListTile",
  "Row", "Column", "Wrap", "ListView", "GridView", "Icon", "Image", "Divider", "Chip",
];
const PLATFORM_PACKAGES = [
  "camera", "connectivity_plus", "device_info_plus", "file_picker", "flutter_secure_storage",
  "geolocator", "image_picker", "package_info_plus", "path_provider", "permission_handler",
  "shared_preferences", "url_launcher",
];

/** Diagnose the complete removable Flutter architecture, design, and evidence band. */
export function diagnose(root, { strict = false, operationIds = [] } = {}) {
  const project = resolve(root);
  const lib = join(project, "lib");
  const test = join(project, "test");
  if (!existsSync(lib)) throw new Error(`Flutter lib directory not found: ${lib}`);
  const files = [...dartFiles(lib), ...(existsSync(test) ? dartFiles(test) : [])];
  const known = new Set(files.map((file) => resolve(file)));
  const findings = [];
  const views = files.filter((file) => file.endsWith("_view.dart") && under(file, lib));
  const viewModels = files.filter((file) => file.endsWith("_view_model.dart") && under(file, lib));

  for (const file of files) scanFile(file, readFileSync(file, "utf8"), project, findings);
  for (const view of views) checkFeatureUnit(view, lib, test, known, findings);
  for (const model of viewModels) checkEvidence(model, lib, test, known, project, findings);
  checkI18n(project, findings);
  const dataDoors = files.filter((file) => under(file, lib) && isEndpointDataDoor(relative(project, file)));
  checkEndpointCoverage(dataDoors, operationIds, findings, strict);
  checkMutationDefaults(files, findings);

  return findings.map((item) => strict && item.severity === "warning" ? { ...item, severity: "error" } : item);
}

/** Aggregate rule, endpoint, feature-E2E, runner, and optional backend-journey evidence. */
export function auditProject(root, { strict = false, contractFile, backendRoot, backendRoots = [], includeE2e = true } = {}) {
  const project = resolve(root);
  const flowsPath = join(project, "e2e", "flows.json");
  const flows = existsSync(flowsPath) ? JSON.parse(readFileSync(flowsPath, "utf8")) : [];
  const contracts = new Set();
  if (contractFile) contracts.add(resolve(project, contractFile));
  else {
    const directory = join(project, "contract");
    if (existsSync(directory)) {
      for (const entry of readdirSync(directory, { withFileTypes: true })) {
        if (entry.isFile() && entry.name.endsWith(".json")) contracts.add(join(directory, entry.name));
      }
    }
  }
  const operations = [...contracts].flatMap((path) =>
    existsSync(path) ? readOperationIds(JSON.parse(readFileSync(path, "utf8"))) : []);
  const findings = diagnose(project, { strict, operationIds: [...new Set(operations)] });
  if (!includeE2e) return findings;
  const referenceContracts = new Set(flows
    .filter((flow) => typeof flow.backendContract === "string")
    .map((flow) => resolve(project, flow.backendContract)));
  const referenceOperations = [...referenceContracts].flatMap((path) =>
    existsSync(path) ? readOperationIds(JSON.parse(readFileSync(path, "utf8"))) : []);
  const featureE2e = checkFeatureE2e(findViewModels(project), flows, [...new Set([...operations, ...referenceOperations])]);
  for (const gap of featureE2e.gaps) findings.push({ rule: "SKYFL035", file: flowsPath, message: gap, severity: "error" });
  const e2e = checkE2eProject(project);
  for (const gap of e2e.gaps) findings.push({ rule: "SKYFL-E2E", file: join(project, "integration_test"), message: gap, severity: "error" });
  const roots = [...new Set([...(backendRoot ? [backendRoot] : []), ...backendRoots].map((path) => resolve(path)))];
  if (roots.length > 0) {
    const inventories = roots.map((path) => ({ path, inventory: readBackendJourneyInventory(path) }));
    for (const item of inventories) {
      const inventoryGap = backendInventoryError(item.inventory);
      if (inventoryGap) findings.push({ rule: "SKYFL-JOURNEY", file: item.path, message: inventoryGap, severity: "error" });
    }
    const inventory = mergeBackendInventories(inventories.map((item) => item.inventory));
    const journey = checkJourneyParity(inventory, flows);
    for (const gap of journey.messages) findings.push({ rule: "SKYFL-JOURNEY", file: roots.join(","), message: gap, severity: "error" });
  }
  return findings;
}

function mergeBackendInventories(inventories) {
  const paths = new Map();
  for (const inventory of inventories) {
    for (const [slice, values] of inventory.paths) {
      const merged = paths.get(slice) ?? new Set();
      for (const value of values) merged.add(value);
      paths.set(slice, merged);
    }
  }
  return {
    slices: [...new Set(inventories.flatMap((inventory) => inventory.slices))].sort(),
    writes: [...new Set(inventories.flatMap((inventory) => inventory.writes))].sort(),
    paths,
  };
}

function scanFile(file, source, project, findings) {
  const path = slash(relative(project, file));
  const testFile = /(?:_test|\.assay_test)\.dart$/.test(file);
  const view = file.endsWith("_view.dart");
  const model = file.endsWith("_view_model.dart");
  const ui = /(?:^|\/)lib\/ui\//.test(path);
  const token = /(?:theme|tokens|palette|colors)\.dart$/.test(file);
  const sessionDoor = /(?:^|\/)lib\/(?:session|skies_client|guards?)\.dart$/.test(path);
  const htmlDoor = /(?:^|\/)lib\/html(?:\/|\.dart$)/.test(path);

  if (!testFile && /import\s+['"][^'"]*(?:__mocks__|fixtures|mockito|mocktail|msw)[^'"]*['"]/.test(source)) {
    add(findings, "no-mock", file, "production code imports a mock or fixture");
  }
  if (view && (/package:dio\//.test(source) || /\b(?:Dio|SkiesClient|executeSkiesRequest)\b/.test(source))) {
    add(findings, "view-purity", file, "View reaches transport or client behavior");
  }
  if (!model && !sessionDoor && consumesOperation(source)) {
    add(findings, "data-door", file, "generated operation is consumed outside a ViewModel or infrastructure door");
  }
  if (model && /package:flutter\/(?:widgets|material|cupertino)\.dart|\b(?:Widget|BuildContext|Navigator)\b/.test(source)) {
    add(findings, "viewmodel-render-agnostic", file, "ViewModel imports or names rendering APIs");
  }
  if (model && (/(?:^|\n)import\s+['"]dart:io['"]/.test(source) || PLATFORM_PACKAGES.some((name) => source.includes(`package:${name}/`)))) {
    add(findings, "viewmodel-platform-agnostic", file, "ViewModel imports a device capability instead of an injected port");
  }
  if (model && !/\bAsyncState\s*</.test(source)) {
    add(findings, "mandatory-state", file, "server-backed ViewModel exposes no AsyncState");
  }
  if (view && pairedModelUsesAsyncState(file) && !/\bResourceBuilder\s*</.test(source)) {
    add(findings, "state-completeness", file, "View does not route its closed resource through ResourceBuilder");
  }
  if (!testFile && !token && !ui) {
    for (const match of source.matchAll(/#[0-9a-fA-F]{3,8}\b/g)) add(findings, "design-tokens", file, `raw hex ${match[0]}`);
    if (/\bColor\s*\(\s*0x[0-9a-fA-F]+\s*\)|\bColors\.[A-Za-z]+/.test(source)) {
      add(findings, "semantic-colors", file, "raw or palette color appears outside tokens or ui");
    }
    if (/EdgeInsets\.(?:all|only|symmetric)\([^)]*\b(?:[1-9]\d*\.?\d*)\b|(?:fontSize|height)\s*:\s*[1-9]\d*/.test(source)) {
      add(findings, "scale-only", file, "spacing or typography uses a numeric value outside the token scale");
    }
  }
  if (view && /(?:Text|RichText)\s*\(\s*['"]|(?:label|hintText|semanticLabel|tooltip)\s*:\s*['"]/.test(source)) {
    add(findings, "no-hardcoded-copy", file, "View contains user-facing string copy instead of localizations");
  }
  if (view) {
    const raw = RAW_WIDGETS.find((widget) => new RegExp(`\\b${widget}\\s*(?:<[^>]+>)?\\s*\\(`).test(source));
    if (raw) add(findings, "ui-door", file, `View renders raw ${raw} instead of an app ui primitive`);
    if (/\bstyle\s*:/.test(source)) add(findings, "ui-door", file, "View carries a free-form style argument");
  }
  if (/addPostFrameCallback[\s\S]{0,500}(?:pushReplacement|go|replace)\s*\(/.test(source) || /addListener[\s\S]{0,500}(?:pushReplacement|go|replace)\s*\(/.test(source)) {
    add(findings, "declarative-redirect", file, "state-driven redirect runs imperatively after render");
  }
  if (!sessionDoor && (/\bsetAccessToken\s*\(/.test(source) || /(?:write|setString)\s*\(\s*['"][^'"]*token/i.test(source))) {
    add(findings, "session-one-door", file, "session token is written outside the session seam");
  }
  if (/(?:route|guard)[^/]*\.dart$/.test(file) && /\bisAuthenticated\b/.test(source)) {
    add(findings, "guard-tristate", file, "route guard collapses session loading into a boolean");
  }
  if (/(?:pathParameters|queryParameters)\s*\[[^\]]*(?:id|Id)['"]?\]/.test(source) && !/requiredParam\s*\(/.test(source)) {
    add(findings, "route-param-guard", file, "required route id is read without requiredParam");
  }
  if (/Navigator\.(?:of\([^)]*\)\.)?pop\s*\(|\bcontext\.pop\s*\(/.test(source)) {
    add(findings, "safe-back", file, "bare back navigation has no deep-link fallback");
  }
  if (/(?:BaseOptions|Dio)\s*\([^)]*baseUrl\s*:\s*['"]https?:\/\//s.test(source)) {
    add(findings, "configured-base-url", file, "API base URL is hardcoded");
  }
  if (!htmlDoor && (/package:flutter_html\//.test(source) || /\b(?:Html|HtmlElementView|WebViewWidget)\s*\(/.test(source))) {
    add(findings, "raw-html-one-door", file, "raw HTML rendering occurs outside lib/html");
  }
  if (/(?:go|push|pushReplacement|replace)\s*\(\s*(?:returnTo|next|redirect)(?:\s|,|\))/.test(source)) {
    add(findings, "no-open-redirect", file, "navigation consumes a URL-derived target without an allowlist");
  }
  if (!testFile && /\b(?:TODO|FIXME|HACK|XXX|wire later)\b|UnimplementedError\s*\(/i.test(source)) {
    add(findings, "no-placeholder", file, "production code contains an unfinished placeholder");
  }
  if (model && hasMutation(source) && !/MutationBoundary|catch\s*\([^)]*\)[\s\S]{0,300}(?:AsyncFailure|error|feedback)/.test(source)) {
    add(findings, "mutation-error-surface", file, "mutation has no global or local failure surface");
  }
  if (model && /expectedFailure\s*:\s*true/.test(source) && !/catch\s*\([^)]*\)[\s\S]{0,300}(?:AsyncFailure|error|feedback)/.test(source)) {
    add(findings, "mutation-error-surface", file, "expected failure suppresses global feedback without a modeled local failure surface");
  }
  if (/onSuccess\s*[:=][\s\S]{0,200}(?:refetch|reload|invalidate)[\s\S]{0,30}[;}]/.test(source)) {
    add(findings, "no-manual-refetch", file, "success callback only repeats global invalidation", "warning");
  }
  if (!sessionDoor && /(?:refreshSession|bootstrapSession|\brefresh\s*\()/.test(source)) {
    add(findings, "refresh-one-door", file, "refresh rotation is consumed outside the session/client seam");
  }
  if (/(?:context\.(?:go|push|replace)|Navigator\.[^(]+)\s*\([^\n]*\bas\s+(?:dynamic|Object|string|String)/.test(source)) {
    add(findings, "typed-navigation", file, "navigation target escapes its typed route through a cast");
  }
  if (model && /\bvalidate\s*\(\s*\)/.test(source) && !/submitOrReveal\s*\(|\belse\b|if\s*\(\s*!?[^)]*validate/.test(source)) {
    add(findings, "submit-invalid-path", file, "form validation has no explicit invalid path", "warning");
  }
  if (ui && /TextFormField\s*\(/.test(source) && !/validator\s*:|errorText\s*:/.test(source)) {
    add(findings, "field-error-surface", file, "form field exposes no validation error surface", "warning");
  }
  if (testFile && /\bskip\s*:\s*(?:true|['"])|@Skip\b|\b(?:soloTest|soloTestWidgets)\s*\(/.test(source)) {
    add(findings, "no-disabled-tests", file, "test is skipped or focused");
  }
}

function checkFeatureUnit(view, lib, test, known, findings) {
  const stem = view.slice(0, -"_view.dart".length);
  const folder = relative(lib, dirname(view));
  const name = basename(stem);
  const model = `${stem}_view_model.dart`;
  const viewTest = join(test, folder, `${name}_view_test.dart`);
  const modelTest = join(test, folder, `${name}_view_model_test.dart`);
  if (!known.has(resolve(model))) add(findings, "view-purity", view, "ViewModel is not co-located");
  if (!testProves(known, viewTest, `${name}_view.dart`, "testWidgets")) {
    add(findings, "view-integration-test", view, "mirrored widget test does not import and pump the View");
  }
  if (known.has(resolve(viewTest))) {
    const proof = readFileSync(viewTest, "utf8");
    for (const guideline of ["androidTapTargetGuideline", "labeledTapTargetGuideline", "textContrastGuideline"]) {
      if (!proof.includes(`meetsGuideline(${guideline})`)) {
        findings.push({ rule: "SKYFL-A11Y", file: resolve(viewTest), message: `widget proof omits ${guideline}`, severity: "error" });
      }
    }
  }
  if (!testProves(known, modelTest, `${name}_view_model.dart`, "test(")) {
    add(findings, "viewmodel-test", model, "mirrored unit test does not import and execute the ViewModel");
  }
}

function checkEvidence(model, lib, test, known, project, findings) {
  const source = readFileSync(model, "utf8");
  const stem = model.slice(0, -"_view_model.dart".length);
  const folder = relative(lib, dirname(model));
  const assay = join(test, folder, `${basename(stem)}.assay_test.dart`);
  const verify = markers(source, "verify");
  if (verify.length === 0) add(findings, "avp-proof", model, "ViewModel declares no @verify criterion");
  const assaySource = known.has(resolve(assay)) ? readFileSync(assay, "utf8") : "";
  const avp = markers(assaySource, "avp");
  for (const id of verify) {
    if (!avp.includes(id) || !hasExecutableProof(assaySource, id)) {
      add(findings, "avp-proof", assay, `@verify ${id} has no executable co-located @avp proof`);
    }
  }
  for (const id of avp) if (!verify.includes(id)) add(findings, "avp-proof", assay, `orphan @avp ${id}`);

  const obligations = markers(source, "e2e");
  if (obligations.length < 2) add(findings, "feature-e2e-flow", model, "ViewModel needs distinct happy and sad @e2e flows");
  const flows = readFlows(project);
  for (const id of obligations) {
    if (!flows.some((flow) => flow.id === id && flow.features?.includes(pascal(basename(stem))))) {
      add(findings, "feature-e2e-flow", model, `@e2e ${id} is absent from e2e/flows.json`);
    }
  }
}

function checkI18n(project, findings) {
  const roots = [join(project, "lib", "l10n"), join(project, "l10n")].filter(existsSync);
  const arbs = roots.flatMap((root) => filesWith(root, ".arb"));
  const groups = new Map();
  for (const file of arbs) {
    const key = basename(file).replace(/_(?:pt(?:_BR)?|en(?:_US)?|es(?:_ES)?)\.arb$/, "");
    const entries = Object.keys(JSON.parse(readFileSync(file, "utf8"))).filter((item) => !item.startsWith("@"));
    if (!groups.has(key)) groups.set(key, []);
    groups.get(key).push({ file, entries });
  }
  for (const catalogs of groups.values()) {
    const union = new Set(catalogs.flatMap((item) => item.entries));
    for (const catalog of catalogs) {
      const missing = [...union].filter((key) => !catalog.entries.includes(key));
      if (missing.length > 0) add(findings, "i18n-parity", catalog.file, `missing ARB keys: ${missing.join(", ")}`);
    }
  }
}

function checkEndpointCoverage(models, operationIds, findings, strict) {
  if (operationIds.length === 0) return;
  const sources = models.map((file) => readFileSync(file, "utf8")).join("\n");
  for (const id of operationIds) {
    if (!new RegExp(`\\.${escape(lowerCamel(id))}\\s*\\(`).test(sources)) {
      findings.push({ rule: RULE["endpoint-coverage"], file: "<openapi>", message: `${id} has no ViewModel consumer`, severity: strict ? "error" : "warning" });
    }
  }
}

function checkMutationDefaults(files, findings) {
  const models = files.filter((file) => file.endsWith("_view_model.dart"));
  if (!models.some((file) => hasMutation(readFileSync(file, "utf8")))) return;
  const production = files.filter((file) => !file.endsWith("_test.dart"));
  if (!production.some((file) => /MutationBoundary\s*\(/.test(readFileSync(file, "utf8")))) {
    add(findings, "mutation-defaults", "<project>", "write features exist without one configured MutationBoundary");
  }
}

function consumesOperation(source) {
  return /\.api\s*\.\s*get[A-Z]\w*Api\s*\(\s*\)\s*\.\s*\w+\s*\(|\bexecuteSkiesRequest\s*</.test(source);
}
function hasMutation(source) {
  return /Future\s*<[^>]*>\s+(?:submit|save|create|update|delete|remove|deposit|withdraw)\b/.test(source);
}
function pairedModelUsesAsyncState(view) {
  const model = view.replace(/_view\.dart$/, "_view_model.dart");
  return existsSync(model) && /\bAsyncState\s*</.test(readFileSync(model, "utf8"));
}
function testProves(known, file, importName, invocation) {
  if (!known.has(resolve(file))) return false;
  const source = readFileSync(file, "utf8");
  return source.includes(importName) && source.includes(invocation);
}
function markers(source, name) {
  return [...source.matchAll(new RegExp(`@${name}\\s+([a-z0-9][a-z0-9._-]*)`, "gi"))].map((match) => match[1]);
}
function hasExecutableProof(source, id) {
  const declaration = new RegExp(`(?:test|testWidgets)\\s*\\(\\s*['\"][^'\"]*${escape(id)}[^'\"]*['\"]`, "i").exec(source);
  if (!declaration) return false;
  const tail = source.slice(declaration.index + declaration[0].length);
  const nextTest = tail.search(/\n\s*(?:test|testWidgets)\s*\(/);
  const body = nextTest >= 0 ? tail.slice(0, nextTest) : tail;
  return /\b(?:expect|expectLater|fail)\s*\(/.test(body);
}
function readFlows(project) {
  const path = join(project, "e2e", "flows.json");
  if (!existsSync(path)) return [];
  const parsed = JSON.parse(readFileSync(path, "utf8"));
  return Array.isArray(parsed) ? parsed : parsed.flows ?? [];
}
function dartFiles(root) { return filesWith(root, ".dart"); }
function filesWith(root, suffix) {
  const result = [];
  const visit = (directory) => {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const path = join(directory, entry.name);
      if (entry.isDirectory() && !["build", ".dart_tool"].includes(entry.name)) visit(path);
      else if (entry.isFile() && entry.name.endsWith(suffix)) result.push(path);
    }
  };
  visit(resolve(root));
  return result.sort();
}
function add(findings, name, file, message, severity = "error") {
  findings.push({ rule: RULE[name], file: resolve(file), message, severity });
}
function under(file, root) { return !relative(root, file).startsWith(".."); }
function slash(value) { return value.replace(/\\/g, "/"); }
function escape(value) { return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"); }
function lowerCamel(value) { return value ? value[0].toLowerCase() + value.slice(1) : value; }
function pascal(value) { return value.split("_").map((part) => part ? part[0].toUpperCase() + part.slice(1) : "").join(""); }

const invokedDirectly = process.argv[1] && import.meta.url.split("/").pop() === process.argv[1].replace(/\\/g, "/").split("/").pop();
if (invokedDirectly) {
  const args = process.argv.slice(2);
  const strict = args.includes("--strict");
  const root = args.find((arg) => !arg.startsWith("--"));
  if (!root) {
    console.error("usage: skies-flutter-doctor <flutter-project> [--strict] [--structure-only] [--contract <openapi.json>] [--backend-root <directory>]");
    process.exitCode = 2;
  } else {
    const valueAfter = (flag) => {
      const index = args.indexOf(flag);
      return index >= 0 ? args[index + 1] : undefined;
    };
    const valuesAfter = (flag) => args.flatMap((value, index) =>
      value === flag && args[index + 1] ? [args[index + 1]] : []);
    const findings = auditProject(root, {
      strict,
      contractFile: valueAfter("--contract"),
      backendRoots: valuesAfter("--backend-root"),
      includeE2e: !args.includes("--structure-only"),
    });
    for (const item of findings) console.error(`${item.rule} ${item.severity} ${item.file}: ${item.message}`);
    if (findings.length === 0) console.log("skies flutter doctor: PASS");
    process.exitCode = findings.some((item) => item.severity === "error") ? 1 : 0;
  }
}
