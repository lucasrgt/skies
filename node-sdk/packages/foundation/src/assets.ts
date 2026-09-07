import { FoundationError } from "./types.js";
import { normalizeRelativePath } from "./config.js";
import { applyTextPlan, readSafeText, type TextAction, type TextChange } from "./safe-fs.js";

export const FOUNDATION_VERSION = "0.1.2";
/** Shared CSM configuration, mirroring the .NET side: [storage] root = ".skies/csm". */
export const CSM_CONFIG = "csm.toml";
/** Pre-CSM contract legacy JSON config; read for migration but never written. */
export const LEGACY_CSM_CONFIG = "csm.json";
export const INSTRUCTIONS_START = "<!-- skies-node:foundations:start -->";
export const INSTRUCTIONS_END = "<!-- skies-node:foundations:end -->";
const MANAGED = "<!-- managed by @skiesjs/foundation; run `skies-node-foundation foundations sync` -->";

export interface CsmConfig {
  readonly schemaVersion: 1;
  readonly storage: string;
}

export interface FoundationAssetsOptions {
  readonly root: string;
  readonly operation: "init" | "sync";
  readonly dryRun?: boolean;
  readonly agentFiles?: readonly string[];
}

export interface FoundationAssetsResult {
  readonly operation: "init" | "sync";
  readonly dryRun: boolean;
  readonly storage: string;
  readonly actions: readonly TextAction[];
}

function skill(id: "nwc" | "nya" | "rtw", purpose: string, operations: readonly string[]): string {
  return `${MANAGED}
# ${id.toUpperCase()} — ${purpose}

This is the repository-local Node foundation surface. Do not install or invoke an ambient CSM binary.

${operations.map((operation) => `- \`skies-node-foundation ${id} ${operation}\``).join("\n")}
- \`skies-node-foundation context --task "<goal>"\`
- \`skies-node-foundation check --task "<goal>" --affected\`
`;
}

// WTW mirrors the shared CSM contract: agents explain and guard; hosts alone collect after an
// authoritative source contains a durable choice or falsifiable invariant.
const WTW_SKILL = `${MANAGED}
# WTW — decisions and invariants

This is the repository-local Node foundation surface. Do not install or invoke an ambient CSM binary.

Use \`skies-node-foundation wtw explain\` only when deeper decision inspection is needed. Read the returned authority, rationale, alternatives, violation examples, and links before choosing an implementation.

WTW records are written only by the host through the shared CSM host collection after an authoritative source contains a durable choice or falsifiable invariant. This Node surface has no manual add command: \`skies-node-foundation wtw collect\` does not exist. Two isolated judges must return the same evidence-backed candidate before a record is written.

Use \`skies-node-foundation wtw guard\` only for focused investigation or maintenance. The standard \`skies-node-foundation check\` receipt already runs it and treats a malformed record, conflicting local relation, dangling WTW URI, or suite-mode invariant without an inbound proof as a blocking health failure.

- \`skies-node-foundation context --task "<goal>"\`
- \`skies-node-foundation check --task "<goal>" --affected\`
`;

export const FOUNDATION_INSTRUCTIONS = `${INSTRUCTIONS_START}
## Skies Node foundation workflow

The primary coding agent owns the complete foundation lifecycle. Never create or delegate one agent per foundation.

1. At task start, run \`skies-node-foundation context --task "<goal>" --path <expected-path>\`. Treat every returned decision, invariant, way, scar, and due deferment as governing context.
2. Rerun \`skies-node-foundation context\` after scope changes, context compaction, or movement into an unfamiliar area. Keep retrieval bounded with accurate task text and paths.
3. Use the repository-local foundation skills only when a real lifecycle event occurs: accepted decisions for WTW, proven patterns for RTW, corrected failures for NYA, or evidence-backed conditional deferments for NWC. Never record hypothetical guidance.
4. Run focused repository tests and linters during implementation.
5. Before commit, stage the exact intended paths and run \`skies-node-foundation check --task "<completed work>" --staged\`. Staged checks are always bounded: mapped proofs run, while exhaustive fallbacks and browser/device execution wait for authoritative CI.
6. Before push, run \`skies-node-foundation check --task "<review>" --base <target-revision> --fast\`. The pre-push hook repeats this bounded committed-diff review.
7. Never replace the automation-owned depth gates: pull-request CI runs affected without --fast, and release automation runs --full. Do not report an external delivery complete until its required status is green. Bare \`skies-node-foundation check --task ...\` is intentionally invalid so an ambiguous scope cannot start a surprise exhaustive run.
8. Allow one automatic authoritative/full attempt. After failure or interruption, stop and check whether you are in a loop. Diagnose the first failure, correct its cause and run focused verification before one retry with \`--retry-review <json-file>\` containing PreviousAttemptId, Diagnosis, Correction and FocusedVerification. Never launch overlapping broad checks. Exit 1 means findings and exit 2 or greater means incomplete validation. Neither is a pass; focused checks do not replace the required gate.

Tests, linters, review, and individual foundation commands do not replace \`skies-node-foundation check\`.
${INSTRUCTIONS_END}`;

function consolidateInstructions(content: string): string {
  const starts = content.split(INSTRUCTIONS_START).length - 1;
  const ends = content.split(INSTRUCTIONS_END).length - 1;
  if (starts !== ends || starts > 1) throw new FoundationError("agent instructions contain malformed foundation markers", "io");
  let outside = content;
  if (starts === 1) {
    const start = outside.indexOf(INSTRUCTIONS_START);
    const end = outside.indexOf(INSTRUCTIONS_END, start) + INSTRUCTIONS_END.length;
    outside = `${outside.slice(0, start)}${outside.slice(end)}`;
  }
  const trimmed = outside.trimEnd();
  return `${trimmed.length === 0 ? "" : `${trimmed}

`}${FOUNDATION_INSTRUCTIONS}
`;
}

const STORAGE_PATTERN = /^[A-Za-z0-9._-]+(?:\/[A-Za-z0-9._-]+)*$/u;

function storage(value: string, at: string): string {
  const normalized = normalizeRelativePath(value, at);
  if (normalized === ".") throw new FoundationError(`${at} must not be the workspace root`);
  if (!STORAGE_PATTERN.test(normalized)) throw new FoundationError(`${at} must be a portable relative directory`);
  return normalized;
}

function parseCsmJson(json: string): CsmConfig {
  let value: unknown;
  try { value = JSON.parse(json) as unknown; } catch (error) {
    throw new FoundationError(`cannot parse ${LEGACY_CSM_CONFIG}: ${error instanceof Error ? error.message : String(error)}`, "config");
  }
  if (value === null || typeof value !== "object" || Array.isArray(value)) throw new FoundationError(`${LEGACY_CSM_CONFIG} must be an object`);
  const raw = value as Record<string, unknown>;
  const unknown = Object.keys(raw).filter((key) => !["schemaVersion", "storage"].includes(key));
  if (unknown.length > 0) throw new FoundationError(`${LEGACY_CSM_CONFIG} has unknown key(s): ${unknown.sort().join(", ")}`);
  if (raw.schemaVersion !== 1 || typeof raw.storage !== "string") throw new FoundationError(`${LEGACY_CSM_CONFIG} requires schemaVersion 1 and a storage string`);
  return { schemaVersion: 1, storage: storage(raw.storage, `${LEGACY_CSM_CONFIG}.storage`) };
}

function parseCsmToml(content: string): CsmConfig {
  const lines = content.split(/\r?\n/u);
  const storageIndex = lines.findIndex((line) => /^\s*\[storage\]\s*$/u.test(line));
  if (storageIndex < 0) throw new FoundationError(`${CSM_CONFIG}: [storage] must declare a root`, "config");
  let root: string | undefined;
  for (let index = storageIndex + 1; index < lines.length; index++) {
    const line = lines[index]!;
    if (/^\s*\[/u.test(line)) break;
    const match = /^\s*root\s*=\s*"(?<value>[^"]+)"\s*$/u.exec(line);
    if (match !== null) { root = match.groups!.value; break; }
  }
  if (root === undefined || root.trim().length === 0) {
    throw new FoundationError(`${CSM_CONFIG}: [storage] must declare a non-empty root`, "config");
  }
  return { schemaVersion: 1, storage: storage(root, `${CSM_CONFIG}.storage`) };
}

export function tomlConfig(config: CsmConfig): string {
  return `schema = 1

[storage]
root = "${config.storage}"
`;
}

export async function loadCsmConfig(root: string): Promise<CsmConfig> {
  const toml = await readSafeText(root, CSM_CONFIG);
  if (toml !== undefined) return parseCsmToml(toml);
  const legacy = await readSafeText(root, LEGACY_CSM_CONFIG);
  if (legacy !== undefined) return parseCsmJson(legacy);
  throw new FoundationError(`${CSM_CONFIG} is missing; run foundations init`, "config");
}

function managedAssets(storage: string): readonly { path: string; content: string }[] {
  const keep = `${MANAGED}
`;
  return [
    { path: `${storage}/.gitignore`, content: `# managed by @skiesjs/foundation
**/index*.sqlite
` },
    { path: `${storage}/lock.json`, content: `${JSON.stringify({ schemaVersion: 1, managedBy: "@skiesjs/foundation", version: FOUNDATION_VERSION }, null, 2)}
` },
    { path: `${storage}/nya/config.json`, content: `${JSON.stringify({ schemaVersion: 1 }, null, 2)}
` },
    { path: `${storage}/nwc/SKILL.md`, content: skill("nwc", "next work and deferments", ["wake", "collect", "resolve", "check"]) },
    { path: `${storage}/nya/SKILL.md`, content: skill("nya", "corrected scars and lessons", ["recall", "spec", "check", "replay"]) },
    { path: `${storage}/rtw/SKILL.md`, content: skill("rtw", "repository ways and guidance", ["guide", "add", "check"]) },
    { path: `${storage}/wtw/SKILL.md`, content: WTW_SKILL },
    { path: `${storage}/nwc/deferments/.gitkeep`, content: keep },
    { path: `${storage}/nya/scars/.gitkeep`, content: keep },
    { path: `${storage}/rtw/ways/.gitkeep`, content: keep },
    { path: `${storage}/wtw/records/decisions/.gitkeep`, content: keep },
    { path: `${storage}/wtw/records/invariants/.gitkeep`, content: keep },
  ];
}

export interface FoundationTextFile {
  readonly path: string;
  readonly content: string;
}

/** Canonical foundation files for a newly generated empty application, before any local records exist. */
export function defaultFoundationFiles(): readonly FoundationTextFile[] {
  const config = { schemaVersion: 1 as const, storage: ".skies/csm" };
  return [
    { path: CSM_CONFIG, content: tomlConfig(config) },
    ...managedAssets(config.storage),
    { path: "AGENTS.md", content: `${FOUNDATION_INSTRUCTIONS}
` },
  ];
}

async function agentTargets(root: string, selected?: readonly string[]): Promise<string[]> {
  let files: string[];
  if (selected === undefined || selected.length === 0) {
    const detected: string[] = [];
    for (const candidate of ["AGENTS.md", "CLAUDE.md", "GEMINI.md"]) {
      if (await readSafeText(root, candidate) !== undefined) detected.push(candidate);
    }
    files = detected.length === 0 ? ["AGENTS.md"] : detected;
  } else files = selected.map((file, index) => normalizeRelativePath(file, `agentFiles[${index}]`));
  const deduplicated: string[] = [];
  const seen = new Set<string>();
  for (const file of files) {
    const key = file.toLowerCase();
    if (!seen.has(key)) { seen.add(key); deduplicated.push(file); }
  }
  if (!deduplicated.some((file) => !file.includes("/") && /^(agents|claude|gemini)\.md$/iu.test(file))) deduplicated.push("AGENTS.md");
  for (const file of deduplicated) {
    if (!file.toLowerCase().endsWith(".md")) throw new FoundationError(`agent file '${file}' must be Markdown`, "invocation");
    if (!/^[A-Za-z0-9._/-]+$/u.test(file)) throw new FoundationError(`agent file '${file}' is not a portable path`, "invocation");
  }
  return deduplicated;
}

export async function installFoundationAssets(options: FoundationAssetsOptions): Promise<FoundationAssetsResult> {
  const existingToml = await readSafeText(options.root, CSM_CONFIG);
  const legacyJson = existingToml === undefined ? await readSafeText(options.root, LEGACY_CSM_CONFIG) : undefined;
  const config = existingToml !== undefined
    ? parseCsmToml(existingToml)
    : legacyJson !== undefined
      ? parseCsmJson(legacyJson)
      : { schemaVersion: 1 as const, storage: ".skies/csm" };
  const changes: TextChange[] = [{
    path: CSM_CONFIG,
    content: existingToml ?? tomlConfig(config),
    overwrite: false,
  }];
  for (const asset of managedAssets(config.storage)) {
    const current = await readSafeText(options.root, asset.path);
    if (options.operation === "sync" && current !== undefined && current !== asset.content) {
      const owned = current.includes("managed by @skiesjs/foundation")
        || current.includes('"managedBy": "@skiesjs/foundation"')
        || asset.path.endsWith("/lock.json")
        || asset.path.endsWith("nya/config.json");
      if (!owned) throw new FoundationError(`refusing to replace unmanaged asset '${asset.path}'`, "io");
    }
    changes.push({ ...asset, overwrite: options.operation === "sync" });
  }
  for (const file of await agentTargets(options.root, options.agentFiles)) {
    const current = await readSafeText(options.root, file) ?? "";
    changes.push({ path: file, content: consolidateInstructions(current), overwrite: true });
  }
  const actions = await applyTextPlan(options.root, changes, options.dryRun ?? false);
  return { operation: options.operation, dryRun: options.dryRun ?? false, storage: config.storage, actions };
}

export async function checkFoundationAssets(root: string): Promise<readonly string[]> {
  const findings: string[] = [];
  let config: CsmConfig;
  try { config = await loadCsmConfig(root); } catch (error) {
    return [error instanceof Error ? error.message : String(error)];
  }
  for (const asset of managedAssets(config.storage)) {
    const current = await readSafeText(root, asset.path);
    if (current === undefined) findings.push(`missing foundation asset '${asset.path}'`);
    else if (current !== asset.content) findings.push(`outdated foundation asset '${asset.path}'`);
  }
  const agents = await agentTargets(root);
  if (agents.length === 0) findings.push("no root agent instruction file is installed");
  for (const file of agents) {
    const current = await readSafeText(root, file);
    if (current === undefined || !current.includes(FOUNDATION_INSTRUCTIONS)) findings.push(`agent file '${file}' lacks the current foundation protocol`);
  }
  return findings;
}
