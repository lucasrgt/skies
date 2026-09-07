import { mkdtemp, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { parseConfig } from "./config.js";
import { runGate, selectProofs } from "./gate.js";
import type { CommandRequest, CommandResult, CommandRunner } from "./types.js";

const topology = {
  schemaVersion: 1,
  git: { base: "origin/main" },
  criteria: [
    { id: "c.unit", statement: "unit criterion" },
    { id: "c.integration", statement: "integration criterion" },
    { id: "c.journey", statement: "journey criterion" },
  ],
  lanes: [
    { id: "unit", command: ["node", "unit.js"], timeoutMs: 1_000 },
    { id: "integration", command: ["node", "integration.js"], timeoutMs: 2_000 },
    { id: "journey", command: ["node", "journey.js"], timeoutMs: 3_000 },
  ],
  proofs: [
    { id: "p-unit", kind: "unit", lane: "unit", criteria: ["c.unit"], sourceScopes: ["src/unit/**"] },
    { id: "p-integration", kind: "integration", lane: "integration", criteria: ["c.integration"], sourceScopes: ["src/shared/**"], dependsOn: ["p-unit"] },
    { id: "p-journey", kind: "journey", lane: "journey", criteria: ["c.journey"], sourceScopes: ["src/journey/**"] },
  ],
  ignoreScopes: ["docs/**"],
  forceFullScopes: ["package.json"],
};

function config() { return parseConfig(JSON.stringify(topology), "/repo/skies.node.json", "/repo"); }
function result(exitCode = 0, extra: Partial<CommandResult> = {}): CommandResult {
  return { exitCode, signal: null, timedOut: false, durationMs: 4, stdout: "out", stderr: "", ...extra };
}
async function workspace(value: unknown = topology): Promise<string> {
  const root = await mkdtemp(join(tmpdir(), "skies-foundation-gate-"));
  await writeFile(join(root, "skies.node.json"), JSON.stringify(value));
  return root;
}

describe("affected/staged/full selection", () => {
  it("selects transitive consumers of changed proofs without unrelated lanes", () => {
    const selection = selectProofs(config(), "affected", ["src/unit/a.ts"]);
    expect([...selection.selected]).toEqual(["p-unit", "p-integration"]);
    expect(selection.selected.has("p-journey")).toBe(false);
    expect(selection.reasons.join(" ")).toContain("consumes changed proof");
  });

  it("does not propagate a supporting dependency back into unrelated consumers", () => {
    const sibling = parseConfig(JSON.stringify({ ...topology, proofs: [...topology.proofs,
      { id: "p-sibling", kind: "unit", lane: "unit", criteria: ["c.unit"],
        sourceScopes: ["src/sibling/**"], dependsOn: ["p-unit"] },
    ] }), "/repo/skies.node.json", "/repo");
    expect([...selectProofs(sibling, "affected", ["src/shared/a.ts"]).selected])
      .toEqual(["p-integration", "p-unit"]);
  });
  it("selects direct scopes and their declared dependencies", () => {
    const selection = selectProofs(config(), "affected", ["src/shared/adapter.ts"]);
    expect([...selection.selected]).toEqual(["p-integration", "p-unit"]);
    expect(selection.reasons.join(" ")).toContain("requires");
  });

  it("keeps ignored changes explicit without selecting proofs", () => {
    const selection = selectProofs(config(), "affected", ["docs/readme.md"]);
    expect([...selection.selected]).toEqual([]);
    expect(selection.reasons).toEqual(["'docs/readme.md' is explicitly ignored"]);
  });

  it("widens unknown and force-full paths instead of returning a false green", () => {
    expect(selectProofs(config(), "affected", ["unknown.txt"]).selected.size).toBe(3);
    expect(selectProofs(config(), "affected", ["package.json"]).selected.size).toBe(3);
  });

  it("selects staged changes like affected and gives full the exhaustive semantics", () => {
    expect([...selectProofs(config(), "staged", ["src/shared/adapter.ts"]).selected]).toEqual(["p-integration", "p-unit"]);
    expect(selectProofs(config(), "full").selected.size).toBe(3);
  });

  it("fast defers exhaustive widening instead of returning a false green or a surprise full run", () => {
    const widened = selectProofs(config(), "affected", ["unknown.txt"]);
    expect(widened.selected.size).toBe(3);
    const fast = selectProofs(config(), "affected", ["unknown.txt"], true);
    expect(fast.selected.size).toBe(0);
    expect(fast.reasons.join(" ")).toContain("deferred by --fast");
    expect(selectProofs(config(), "affected", ["package.json"], true).selected.size).toBe(0);
    expect(selectProofs(config(), "affected", ["package.json"], true).reasons.join(" ")).toContain("deferred by --fast");
  });

  it("matches globstar at zero or multiple directory levels", () => {
    expect([...selectProofs(config(), "affected", ["src/unit/a.ts"]).selected]).toEqual(["p-unit", "p-integration"]);
    expect([...selectProofs(config(), "affected", ["src/unit/deep/a.ts"]).selected]).toEqual(["p-unit", "p-integration"]);
  });

  it("preserves direct proofs and dependencies when their path is also force-full in fast mode", () => {
    const overlap = parseConfig(JSON.stringify({ ...topology, forceFullScopes: ["src/shared/**"] }),
      "/repo/skies.node.json", "/repo");
    expect([...selectProofs(overlap, "staged", ["src/shared/adapter.ts"], true).selected])
      .toEqual(["p-integration", "p-unit"]);
  });
});

describe("gate execution and receipts", () => {
  it.each([
    { mode: "staged" as const },
    { mode: "affected" as const },
    { mode: "affected" as const, fast: true },
    { mode: "affected" as const, baseRevision: "missing" },
    { mode: "affected" as const, baseRevision: "missing", fast: true },
  ])("does not execute any lane when Git discovery is unavailable: %j", async (options) => {
    const root = await workspace();
    let executions = 0;
    const missing = async (): Promise<readonly string[]> => { throw new Error("missing ancestry"); };
    const run = await runGate({ root, ...options, reportPath: false }, {
      runner: async () => { executions++; return result(); },
      git: { changedPaths: missing, stagedPaths: missing, baseDiffPaths: missing },
    });
    expect(executions).toBe(0);
    expect(run.exitCode).not.toBe(0);
    expect(run.receipt.verdict).toBe("red");
  });
  it("runs each selected lane once with an argv array and maps success to selected proofs", async () => {
    const root = await workspace();
    const requests: CommandRequest[] = [];
    const runner: CommandRunner = async (request) => { requests.push(request); return result(); };
    const run = await runGate({ root, mode: "affected", changedPaths: ["src/shared/a.ts"], reportPath: false }, {
      runner, now: () => new Date("2025-01-01T00:00:00.000Z"),
    });
    expect(requests.map((request) => request.command)).toEqual([["node", "unit.js"], ["node", "integration.js"]]);
    expect(run.exitCode).toBe(0);
    expect(run.receipt.proofResults.map((proof) => proof.outcome)).toEqual(["pass", "pass", "not-affected"]);
    expect(run.receipt.matrix.map((row) => row.outcome)).toEqual(["pass", "pass", "not-affected"]);
  });

  it("makes nonzero, timeout, spawn error, and missing execution findings red", async () => {
    const root = await workspace();
    const answers = [result(0), result(0, { timedOut: true, exitCode: null, signal: "SIGTERM" })];
    const run = await runGate({ root, mode: "affected", changedPaths: ["src/shared/a.ts"], reportPath: false }, { runner: async () => answers.shift()! });
    expect(run.exitCode).toBe(1);
    expect(run.receipt.verdict).toBe("red");
    expect(run.receipt.proofResults[1]?.outcome).toBe("fail");
    expect(run.human).toContain("Gate verdict: RED");
  });

  it("uses injected Git discovery and rejects unavailable ancestry without executing unrelated lanes", async () => {
    const root = await workspace();
    const calls: string[] = [];
    const runner: CommandRunner = async (request) => { calls.push(request.command[1]!); return result(); };
    const normal = await runGate({ root, mode: "affected", reportPath: false }, {
      runner, git: {
        changedPaths: async (_root, base) => { expect(base).toBe("origin/main"); return ["src/unit/a.ts"]; },
        stagedPaths: async () => [], baseDiffPaths: async () => [],
      },
    });
    expect(normal.receipt.selectedProofs).toEqual(["p-unit", "p-integration"]);
    const widened = await runGate({ root, mode: "affected", reportPath: false }, {
      runner, git: { changedPaths: async () => { throw new Error("no ancestry"); }, stagedPaths: async () => [], baseDiffPaths: async () => [] },
    });
    expect(widened.receipt.selectedProofs).toHaveLength(0);
    expect(widened.receipt.selectionReasons[0]).toContain("no proof run started");
    expect(widened.exitCode).not.toBe(0);
    expect(calls).toEqual(["unit.js", "integration.js"]);
  });

  it("uses staged index discovery and records the bounded fast receipt", async () => {
    const root = await workspace();
    const run = await runGate({ root, mode: "staged", reportPath: false }, {
      runner: async () => result(),
      git: { changedPaths: async () => [], stagedPaths: async () => ["src/unit/a.ts"], baseDiffPaths: async () => [] },
    });
    expect(run.receipt.selectedProofs).toEqual(["p-unit", "p-integration"]);
    expect(run.receipt.fast).toBe(true);
    expect(run.exitCode).toBe(0);
  });

  it("freezes affected selection to an explicit base revision", async () => {
    const root = await workspace();
    const run = await runGate({ root, mode: "affected", baseRevision: "origin/main", reportPath: false }, {
      runner: async () => result(),
      git: {
        changedPaths: async () => [], stagedPaths: async () => [],
        baseDiffPaths: async (_root, base) => { expect(base).toBe("origin/main"); return ["src/journey/a.ts"]; },
      },
    });
    expect(run.receipt.baseRevision).toBe("origin/main");
    expect(run.receipt.selectedProofs).toEqual(["p-journey"]);
  });

  it("calls an explicitly empty affected change successful without calling it green", async () => {
    const root = await workspace();
    let calls = 0;
    const run = await runGate({ root, mode: "affected", changedPaths: [], reportPath: false }, {
      runner: async () => { calls++; return result(); },
    });
    expect(calls).toBe(0);
    expect(run.exitCode).toBe(0);
    expect(run.receipt.verdict).toBe("no-changes");
    expect(run.receipt.matrix.every((row) => row.outcome === "not-affected")).toBe(true);
  });

  it("writes a JSON receipt and full Markdown artifact transactionally", async () => {
    const root = await workspace();
    const run = await runGate({ root, mode: "full" }, { runner: async () => result(), now: () => new Date(0) });
    expect(run.exitCode).toBe(0);
    const receipt = JSON.parse(await readFile(join(root, "VERIFICATION.json"), "utf8")) as { verdict: string; lanes: unknown[] };
    expect(receipt).toMatchObject({ verdict: "green" });
    expect(receipt.lanes).toHaveLength(3);
    expect(await readFile(join(root, "VERIFICATION.md"), "utf8")).toContain("Gate verdict: GREEN");
  });

  it("blocks uncovered criteria even when every declared command exits zero", async () => {
    const broken = { ...topology, proofs: topology.proofs.filter((proof) => !proof.criteria.includes("c.journey")) };
    const root = await workspace(broken);
    const run = await runGate({ root, mode: "full", reportPath: false, markdownPath: false }, { runner: async () => result() });
    expect(run.exitCode).toBe(1);
    expect(run.receipt.matrix.find((row) => row.criterion === "c.journey")?.outcome).toBe("no-proof");
  });
});
