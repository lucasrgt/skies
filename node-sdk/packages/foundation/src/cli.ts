import { resolve } from "node:path";
import { withAttempt } from "./attempt.js";
import { FOUNDATION_VERSION, checkFoundationAssets, installFoundationAssets } from "./assets.js";
import { readCsmRecords, recordsHuman, resolveDeferment, writeCsmRecord, type CsmFamily, type RecordInput, type WtwKind } from "./csm.js";
import { loadConfig } from "./config.js";
import { runGate } from "./gate.js";
import { buildInventory, coverageRows, criteriaFindings, inventoryHuman, matrixFromReceipt, matrixHuman } from "./inventory.js";
import { readSafeText } from "./safe-fs.js";
import { FoundationError, type GateMode, type GateReceipt } from "./types.js";
import { runContext, runFoundationCheck } from "./workflow.js";

export interface CliIo {
  readonly stdout: Pick<NodeJS.WriteStream, "write">;
  readonly stderr: Pick<NodeJS.WriteStream, "write">;
}

const HELP = `skies-node-foundation — fail-closed Node proof gates and repository-local CSM foundations

Usage:
  skies-node-foundation inventory [--json] [--root <dir>] [--config <file>]
  skies-node-foundation matrix [--receipt <file>] [--json] [--root <dir>]
  skies-node-foundation criteria [check] [--json] [--root <dir>]
  skies-node-foundation gate [--affected [--base <revision>] | --staged | --full] [--fast] [options]
  skies-node-foundation foundations <init|sync> [--dry-run] [--agent-file <file>]
  skies-node-foundation <nwc|nya|rtw|wtw> <operation> [options]
  skies-node-foundation context --task <goal> [--path <path>] [--event <event>]
  skies-node-foundation check --task <goal> (--affected | --staged | --base <revision> | --full) [--fast] [options]

Gate modes:
  --affected            Select proofs by --changed paths, or Git merge-base plus working changes (default).
  --base <revision>     Freeze affected selection to the committed diff <revision>...HEAD (pre-push scope).
  --staged              Select proofs by the Git index diff; always bounded (implies --fast).
  --full                Run every declared proof and write VERIFICATION.json plus VERIFICATION.md.

Gate options:
  --fast                Defer exhaustive fallbacks to authoritative CI; conflicts with --full.
  --changed <path>      Explicit changed path; repeatable and incompatible with --merge-base/--base.
  --merge-base <ref>    Override git.base for affected discovery; incompatible with --base.
  --report <path>        JSON receipt path; --no-report disables receipt files.
  --markdown <path>      Markdown report path.
  --retry-review <file>  Diagnose a failed or interrupted expensive attempt before one retry.

Exit codes: 0 success; 1 gate, proof, or foundation finding; 2 invalid invocation/configuration.
`;

class Arguments {
  index = 0;
  constructor(readonly values: readonly string[]) {}
  next(): string | undefined { return this.values[this.index++]; }
  value(option: string): string {
    const value = this.next();
    if (value === undefined || value.startsWith("--")) throw new FoundationError(`${option} requires a value`, "invocation");
    return value;
  }
}

interface Common {
  root: string;
  config?: string;
  json: boolean;
}

function writeJson(io: CliIo, value: unknown): void { io.stdout.write(`${JSON.stringify(value, null, 2)}
`); }
function failUnknown(argument: string): never { throw new FoundationError(`unknown option '${argument}'`, "invocation"); }

function parseInspection(args: readonly string[], allowReceipt: boolean): Common & { receipt?: string } {
  const result: Common & { receipt?: string } = { root: process.cwd(), json: false };
  const cursor = new Arguments(args);
  for (let argument = cursor.next(); argument !== undefined; argument = cursor.next()) {
    if (argument === "--root") result.root = cursor.value(argument);
    else if (argument === "--config") result.config = cursor.value(argument);
    else if (argument === "--json") result.json = true;
    else if (allowReceipt && argument === "--receipt") result.receipt = cursor.value(argument);
    else failUnknown(argument);
  }
  return result;
}

async function inventoryCommand(args: readonly string[], io: CliIo): Promise<number> {
  const options = parseInspection(args, false);
  const inventory = buildInventory(loadConfig(options.root, options.config));
  if (options.json) writeJson(io, inventory); else io.stdout.write(inventoryHuman(inventory));
  return 0;
}

function validateReceipt(value: unknown, configFingerprint: string, proofIds: ReadonlySet<string>): GateReceipt {
  if (value === null || typeof value !== "object" || Array.isArray(value)) throw new FoundationError("receipt must be an object", "config");
  const receipt = value as Partial<GateReceipt>;
  if (receipt.type !== "skies-node-foundation-gate" || receipt.schemaVersion !== 1 || !Array.isArray(receipt.proofResults)) {
    throw new FoundationError("receipt is not a Skies Node foundation gate receipt", "config");
  }
  if (receipt.configFingerprint !== configFingerprint) throw new FoundationError("receipt was produced from a different configuration", "config");
  const seen = new Set<string>();
  for (const proof of receipt.proofResults) {
    if (proof === null || typeof proof !== "object" || typeof proof.id !== "string" || !proofIds.has(proof.id)) {
      throw new FoundationError("receipt contains an unknown proof", "config");
    }
    if (seen.has(proof.id)) throw new FoundationError(`receipt repeats proof '${proof.id}'`, "config");
    seen.add(proof.id);
    if (!["pass", "fail", "not-run", "not-affected"].includes(proof.outcome)) throw new FoundationError(`receipt has an unknown outcome for '${proof.id}'`, "config");
  }
  return receipt as GateReceipt;
}

async function matrixCommand(args: readonly string[], io: CliIo): Promise<number> {
  const options = parseInspection(args, true);
  const config = loadConfig(options.root, options.config);
  let rows = coverageRows(config);
  if (options.receipt !== undefined) {
    const content = await readSafeText(resolve(options.root), options.receipt);
    if (content === undefined) throw new FoundationError(`receipt '${options.receipt}' does not exist`, "config");
    let value: unknown;
    try { value = JSON.parse(content) as unknown; } catch (error) { throw new FoundationError(`cannot parse receipt: ${error instanceof Error ? error.message : String(error)}`, "config"); }
    rows = matrixFromReceipt(config, validateReceipt(value, config.fingerprint, new Set(config.proofs.map((proof) => proof.id))));
  }
  const findings = criteriaFindings(config);
  if (options.json) writeJson(io, { schemaVersion: 1, rows, findings });
  else {
    io.stdout.write(matrixHuman(rows));
    for (const finding of findings) io.stdout.write(`  finding: ${finding}
`);
  }
  return findings.length > 0 || rows.some((row) => ["no-proof", "fail", "not-run"].includes(row.outcome)) ? 1 : 0;
}

async function criteriaCommand(args: readonly string[], io: CliIo): Promise<number> {
  const normalized = args[0] === "check" ? args.slice(1) : args;
  const options = parseInspection(normalized, false);
  const config = loadConfig(options.root, options.config);
  const rows = coverageRows(config);
  const findings = criteriaFindings(config);
  if (options.json) writeJson(io, { schemaVersion: 1, rows, findings });
  else {
    io.stdout.write(matrixHuman(rows, "Skies Node criteria coverage"));
    for (const finding of findings) io.stdout.write(`  finding: ${finding}
`);
  }
  return findings.length > 0 ? 1 : 0;
}

interface GateCli {
  retryReview?: string;
  root: string; config?: string; mode: GateMode; modeSeen: boolean; changed: string[];
  changedSeen: boolean; mergeBase?: string; baseRevision?: string; fast: boolean;
  report?: string | false; markdown?: string | false; json: boolean;
}

function parseGate(args: readonly string[], requireMode: boolean): GateCli {
  const result: GateCli = { root: process.cwd(), mode: "affected", modeSeen: false, changed: [], changedSeen: false, fast: false, json: false };
  const cursor = new Arguments(args);
  for (let argument = cursor.next(); argument !== undefined; argument = cursor.next()) {
    if (["--affected", "--staged", "--full"].includes(argument)) {
      if (result.modeSeen) throw new FoundationError("gate modes are mutually exclusive", "invocation");
      result.modeSeen = true; result.mode = argument.slice(2) as GateMode;
    } else if (argument === "--base") {
      if (result.modeSeen) throw new FoundationError("gate modes are mutually exclusive", "invocation");
      result.modeSeen = true; result.mode = "affected"; result.baseRevision = cursor.value(argument);
    } else if (argument === "--fast") result.fast = true;
    else if (argument === "--root") result.root = cursor.value(argument);
    else if (argument === "--retry-review") result.retryReview = cursor.value(argument);
    else if (argument === "--config") result.config = cursor.value(argument);
    else if (argument === "--changed") { result.changed.push(cursor.value(argument)); result.changedSeen = true; }
    else if (argument === "--merge-base") result.mergeBase = cursor.value(argument);
    else if (argument === "--report") result.report = cursor.value(argument);
    else if (argument === "--markdown") result.markdown = cursor.value(argument);
    else if (argument === "--no-report") { result.report = false; result.markdown = false; }
    else if (argument === "--json") result.json = true;
    else failUnknown(argument);
  }
  if (requireMode && !result.modeSeen) throw new FoundationError("check requires one explicit gate mode", "invocation");
  if (result.mode !== "affected" && (result.changedSeen || result.mergeBase !== undefined)) {
    throw new FoundationError("--changed and --merge-base are only valid with --affected", "invocation");
  }
  if (result.changedSeen && result.mergeBase !== undefined) throw new FoundationError("--changed and --merge-base are mutually exclusive", "invocation");
  if (result.baseRevision !== undefined && (result.changedSeen || result.mergeBase !== undefined)) {
    throw new FoundationError("--changed and --merge-base are incompatible with --base <revision>", "invocation");
  }
  for (const revision of [result.mergeBase, result.baseRevision]) {
    if (revision !== undefined && (revision.startsWith("-") || /[\u0000-\u0020]/u.test(revision))) {
      throw new FoundationError("--base and --merge-base must be safe Git revisions", "invocation");
    }
  }
  if (result.mode === "full" && result.fast) {
    throw new FoundationError("--full and --fast conflict; a full audit executes every proof", "invocation");
  }
  return result;
}

async function gateCommand(args: readonly string[], io: CliIo): Promise<number> {
  const options = parseGate(args, false);
  const run = await guarded(options, () => runGate({
    root: options.root, mode: options.mode,
    ...(options.config === undefined ? {} : { configPath: options.config }),
    ...(options.changedSeen ? { changedPaths: options.changed } : {}),
    ...(options.mergeBase === undefined ? {} : { mergeBase: options.mergeBase }),
    ...(options.baseRevision === undefined ? {} : { baseRevision: options.baseRevision }),
    ...(options.fast || options.mode === "staged" ? { fast: true } : {}),
    ...(options.report === undefined ? {} : { reportPath: options.report }),
    ...(options.markdown === undefined ? {} : { markdownPath: options.markdown }),
    forwardOutput: !options.json,
  }));
  if (options.json) writeJson(io, run.receipt); else io.stdout.write(run.human);
  if (run.exitCode !== 0) io.stderr.write("STOP — check whether you are in a loop. Diagnose and verify a focused correction before retrying.\n");
  return run.exitCode;
}

interface AssetCli { root: string; dryRun: boolean; json: boolean; agentFiles: string[] }
function parseAssets(args: readonly string[]): AssetCli {
  const result: AssetCli = { root: process.cwd(), dryRun: false, json: false, agentFiles: [] };
  const cursor = new Arguments(args);
  for (let argument = cursor.next(); argument !== undefined; argument = cursor.next()) {
    if (argument === "--root") result.root = cursor.value(argument);
    else if (argument === "--dry-run") result.dryRun = true;
    else if (argument === "--json") result.json = true;
    else if (argument === "--agent-file") result.agentFiles.push(cursor.value(argument));
    else failUnknown(argument);
  }
  return result;
}

async function assetsCommand(operation: string | undefined, args: readonly string[], io: CliIo): Promise<number> {
  if (operation !== "init" && operation !== "sync") throw new FoundationError("foundations requires init or sync", "invocation");
  const options = parseAssets(args);
  const result = await installFoundationAssets({ root: options.root, operation, dryRun: options.dryRun, ...(options.agentFiles.length === 0 ? {} : { agentFiles: options.agentFiles }) });
  if (options.json) writeJson(io, result);
  else {
    io.stdout.write(`Foundations ${operation}${options.dryRun ? " (dry run)" : ""} — ${result.storage}
`);
    for (const action of result.actions) io.stdout.write(`  ${action.action.padEnd(9)} ${action.path}
`);
  }
  return 0;
}

interface RecordCli { root: string; json: boolean; dryRun: boolean; values: Record<string, string> }
function parseRecord(args: readonly string[]): RecordCli {
  const result: RecordCli = { root: process.cwd(), json: false, dryRun: false, values: {} };
  const cursor = new Arguments(args);
  const valued = new Set(["--root", "--id", "--title", "--statement", "--action", "--lesson", "--guidance", "--kind", "--violation"]);
  for (let argument = cursor.next(); argument !== undefined; argument = cursor.next()) {
    if (valued.has(argument)) {
      const value = cursor.value(argument);
      if (argument === "--root") result.root = value;
      else result.values[argument.slice(2)] = value;
    } else if (argument === "--json") result.json = true;
    else if (argument === "--dry-run") result.dryRun = true;
    else failUnknown(argument);
  }
  return result;
}
function required(values: Record<string, string>, name: string): string {
  const value = values[name];
  if (value === undefined) throw new FoundationError(`--${name} is required`, "invocation");
  return value;
}

function assertRecordOptions(options: RecordCli, allowed: readonly string[], allowDryRun: boolean): void {
  const extra = Object.keys(options.values).filter((key) => !allowed.includes(key));
  if (extra.length > 0) throw new FoundationError(`option --${extra.sort()[0]} is not valid for this operation`, "invocation");
  if (options.dryRun && !allowDryRun) throw new FoundationError("--dry-run is only valid for a mutating operation", "invocation");
}

async function csmCommand(family: CsmFamily, operation: string | undefined, args: readonly string[], io: CliIo): Promise<number> {
  const readOperations: Record<CsmFamily, readonly string[]> = { nwc: ["wake"], nya: ["recall", "replay"], rtw: ["guide"], wtw: ["explain"] };
  // WTW records are host-written through the shared CSM host tool after an authoritative source; the
  // agent-facing Node surface intentionally exposes no direct add/collect command.
  const writeOperation: Record<CsmFamily, string | undefined> = { nwc: "collect", nya: "spec", rtw: "add", wtw: undefined };
  const checkOperation: Record<CsmFamily, string> = { nwc: "check", nya: "check", rtw: "check", wtw: "guard" };
  const options = parseRecord(args);
  if (operation !== undefined && readOperations[family].includes(operation)) {
    assertRecordOptions(options, [], false);
    const result = await readCsmRecords(options.root, family);
    if (options.json) writeJson(io, result); else io.stdout.write(recordsHuman(family, result));
    return result.findings.length > 0 ? 1 : 0;
  }
  if (operation !== undefined && operation === writeOperation[family]) {
    const statementKey = family === "nwc" ? "action" : family === "nya" ? "lesson" : family === "rtw" ? "guidance" : "statement";
    const allowed = ["id", "title", statementKey, ...(statementKey === "statement" ? [] : ["statement"]), ...(family === "wtw" ? ["kind", "violation"] : [])];
    assertRecordOptions(options, allowed, true);
    const input: RecordInput = {
      id: required(options.values, "id"), title: required(options.values, "title"),
      statement: options.values[statementKey] ?? required(options.values, "statement"),
      ...(family === "wtw" ? { kind: required(options.values, "kind") as WtwKind } : {}),
      ...(options.values.violation === undefined ? {} : { violation: options.values.violation }),
    };
    if (family === "wtw" && input.kind !== "decision" && input.kind !== "invariant") throw new FoundationError("--kind must be decision or invariant", "invocation");
    const actions = await writeCsmRecord(options.root, family, input, options.dryRun);
    if (options.json) writeJson(io, { family, operation, dryRun: options.dryRun, actions });
    else for (const action of actions) io.stdout.write(`${action.action} ${action.path}${options.dryRun ? " (dry run)" : ""}
`);
    return 0;
  }
  if (family === "nwc" && operation === "resolve") {
    assertRecordOptions(options, ["id"], true);
    const actions = await resolveDeferment(options.root, required(options.values, "id"), options.dryRun);
    if (options.json) writeJson(io, { family, operation, dryRun: options.dryRun, actions });
    else for (const action of actions) io.stdout.write(`${action.action} ${action.path}${options.dryRun ? " (dry run)" : ""}
`);
    return 0;
  }
  if (operation === checkOperation[family]) {
    assertRecordOptions(options, [], false);
    const [records, assets] = await Promise.all([readCsmRecords(options.root, family), checkFoundationAssets(options.root)]);
    const findings = [...records.findings, ...assets];
    if (options.json) writeJson(io, { family, findings });
    else if (findings.length === 0) io.stdout.write(`${family.toUpperCase()} foundation is current.
`);
    else for (const finding of findings) io.stdout.write(`finding: ${finding}
`);
    return findings.length > 0 ? 1 : 0;
  }
  throw new FoundationError(`unknown ${family} operation '${operation ?? ""}'`, "invocation");
}

interface ContextCli { root: string; task?: string; paths: string[]; events: string[]; limit: number; json: boolean }
function parseContext(args: readonly string[]): ContextCli {
  const result: ContextCli = { root: process.cwd(), paths: [], events: [], limit: 8, json: false };
  const cursor = new Arguments(args);
  for (let argument = cursor.next(); argument !== undefined; argument = cursor.next()) {
    if (argument === "--root") result.root = cursor.value(argument);
    else if (argument === "--task") result.task = cursor.value(argument);
    else if (argument === "--path") result.paths.push(cursor.value(argument));
    else if (argument === "--event") result.events.push(cursor.value(argument));
    else if (argument === "--limit") result.limit = Number(cursor.value(argument));
    else if (argument === "--json") result.json = true;
    else failUnknown(argument);
  }
  if (result.task === undefined) throw new FoundationError("--task is required", "invocation");
  if (!Number.isInteger(result.limit) || result.limit < 1 || result.limit > 24) throw new FoundationError("--limit must be an integer from 1 to 24", "invocation");
  return result;
}

async function contextCommand(args: readonly string[], io: CliIo): Promise<number> {
  const options = parseContext(args);
  const result = await runContext({ root: options.root, task: options.task!, paths: options.paths, events: options.events, limit: options.limit });
  if (options.json) writeJson(io, result); else io.stdout.write(result.human);
  return result.exitCode;
}

async function checkCommand(args: readonly string[], io: CliIo): Promise<number> {
  const taskIndex = args.indexOf("--task");
  if (taskIndex < 0 || args[taskIndex + 1] === undefined || args[taskIndex + 1]!.startsWith("--")) throw new FoundationError("--task is required", "invocation");
  const task = args[taskIndex + 1]!;
  const gateArgs = args.filter((_, index) => index !== taskIndex && index !== taskIndex + 1);
  const options = parseGate(gateArgs, true);
  const result = await guarded(options, () => runFoundationCheck({
    root: options.root, task, mode: options.mode,
    ...(options.config === undefined ? {} : { configPath: options.config }),
    ...(options.changedSeen ? { changedPaths: options.changed } : {}),
    ...(options.mergeBase === undefined ? {} : { mergeBase: options.mergeBase }),
    ...(options.baseRevision === undefined ? {} : { baseRevision: options.baseRevision }),
    ...(options.fast || options.mode === "staged" ? { fast: true } : {}),
    ...(options.report === undefined ? {} : { reportPath: options.report }),
    ...(options.markdown === undefined ? {} : { markdownPath: options.markdown }),
    forwardOutput: !options.json,
  }));
  if (options.json) writeJson(io, result); else io.stdout.write(result.human);
  return result.exitCode;
}

function guarded<T extends { exitCode: number }>(options: GateCli, action: () => Promise<T>): Promise<T> {
  return options.fast || options.mode === "staged" ? action()
    : withAttempt(resolve(options.root, ".skies", "verification-attempt"), options.retryReview, action);
}

function routeFoundation(args: readonly string[], io: CliIo): Promise<number> {
  if (args[0] === "stack") return assetsCommand(args[1], args.slice(2), io);
  if (args[0] === "workflow" && args[1] === "context") return contextCommand(args.slice(2), io);
  if (args[0] === "workflow" && args[1] === "check") return checkCommand(args.slice(2), io);
  throw new FoundationError("foundation requires stack or workflow", "invocation");
}

export async function main(args: readonly string[], io: CliIo = process): Promise<number> {
  if (args.length === 0 || args[0] === "--help" || args[0] === "-h") { io.stdout.write(HELP); return 0; }
  if (args.length === 1 && (args[0] === "--version" || args[0] === "-v")) { io.stdout.write(`${FOUNDATION_VERSION}
`); return 0; }
  try {
    const [command, ...rest] = args;
    if (rest.includes("--help") || rest.includes("-h")) { io.stdout.write(HELP); return 0; }
    if (command === "inventory") return await inventoryCommand(rest, io);
    if (command === "matrix") return await matrixCommand(rest, io);
    if (command === "criteria") return await criteriaCommand(rest, io);
    if (command === "gate") return await gateCommand(rest, io);
    if (command === "foundations") return await assetsCommand(rest[0], rest.slice(1), io);
    if (command === "foundation") return await routeFoundation(rest, io);
    if (command === "nwc" || command === "nya" || command === "rtw" || command === "wtw") return await csmCommand(command, rest[0], rest.slice(1), io);
    if (command === "context") return await contextCommand(rest, io);
    if (command === "check") return await checkCommand(rest, io);
    throw new FoundationError(`unknown command '${command}'`, "invocation");
  } catch (error) {
    io.stderr.write(`skies-node-foundation: ${error instanceof Error ? error.message : String(error)}
`);
    return 2;
  }
}
