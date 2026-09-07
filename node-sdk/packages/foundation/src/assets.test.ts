import { mkdtemp, mkdir, readFile, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { FOUNDATION_INSTRUCTIONS, checkFoundationAssets, installFoundationAssets } from "./assets.js";
import { readCsmRecords, resolveDeferment, writeCsmRecord } from "./csm.js";
import { runContext } from "./workflow.js";

async function workspace(): Promise<string> { return mkdtemp(join(tmpdir(), "skies-foundation-assets-")); }

describe("transactional foundation stack", () => {
  it("dry-run preflights every asset without writing", async () => {
    const root = await workspace();
    const result = await installFoundationAssets({ root, operation: "init", dryRun: true });
    expect(result.actions.every((action) => action.action === "create")).toBe(true);
    await expect(readFile(join(root, "csm.toml"), "utf8")).rejects.toMatchObject({ code: "ENOENT" });
  });

  it("installs the complete Node-focused stack and is idempotent", async () => {
    const root = await workspace();
    await writeFile(join(root, "AGENTS.md"), "# Local instructions\n");
    const first = await installFoundationAssets({ root, operation: "init" });
    expect(first.actions.some((action) => action.path.endsWith("wtw/SKILL.md"))).toBe(true);
    expect(await readFile(join(root, "AGENTS.md"), "utf8")).toContain(`# Local instructions\n\n${FOUNDATION_INSTRUCTIONS}`);
    const wtw = await readFile(join(root, ".skies/csm/wtw/SKILL.md"), "utf8");
    expect(wtw).toContain("host through the shared CSM host collection");
    expect(wtw).not.toContain("- `skies-node-foundation wtw collect`");
    expect(await readFile(join(root, "csm.toml"), "utf8")).toContain('[storage]');
    expect(await checkFoundationAssets(root)).toEqual([]);
    const second = await installFoundationAssets({ root, operation: "init" });
    expect(second.actions.every((action) => action.action === "unchanged")).toBe(true);
  });

  it("sync repairs an owned stale skill but refuses unmanaged replacement", async () => {
    const root = await workspace();
    await installFoundationAssets({ root, operation: "init" });
    const skill = join(root, ".skies/csm/rtw/SKILL.md");
    await writeFile(skill, "<!-- managed by @skiesjs/foundation -->\nstale\n");
    const sync = await installFoundationAssets({ root, operation: "sync" });
    expect(sync.actions.find((action) => action.path.endsWith("rtw/SKILL.md"))?.action).toBe("update");
    expect(await readFile(skill, "utf8")).toContain("rtw add");
    const lock = join(root, ".skies/csm/lock.json");
    await writeFile(lock, '{"schemaVersion":1,"managedBy":"@skiesjs/foundation","version":"old"}\n');
    await installFoundationAssets({ root, operation: "sync" });
    expect(await readFile(lock, "utf8")).toContain('"version": "0.1.2"');
    await writeFile(skill, "unmanaged instructions\n");
    await expect(installFoundationAssets({ root, operation: "sync" })).rejects.toThrow("unmanaged");
  });

  it("deduplicates explicit agent files case-insensitively while preserving the first path", async () => {
    const root = await workspace();
    const result = await installFoundationAssets({ root, operation: "init", dryRun: true, agentFiles: ["GEMINI.md", "gemini.md"] });
    expect(result.actions.some((action) => action.path === "GEMINI.md")).toBe(true);
    expect(result.actions.some((action) => action.path === "gemini.md")).toBe(false);
  });

  it("consolidates a managed instruction block rather than duplicating it", async () => {
    const root = await workspace();
    await installFoundationAssets({ root, operation: "init" });
    await installFoundationAssets({ root, operation: "sync" });
    const agents = await readFile(join(root, "AGENTS.md"), "utf8");
    expect(agents.split("<!-- skies-node:foundations:start -->")).toHaveLength(2);
  });

  it("rejects unsafe paths and symlink traversal before mutation", async () => {
    const root = await workspace();
    await expect(installFoundationAssets({ root, operation: "init", agentFiles: ["../AGENTS.md"] })).rejects.toThrow("parent");
    const outside = await workspace();
    await symlink(outside, join(root, ".skies"), process.platform === "win32" ? "junction" : "dir");
    await expect(installFoundationAssets({ root, operation: "init" })).rejects.toThrow("symbolic link");
  });

  it("detects malformed managed markers without partially installing", async () => {
    const root = await workspace();
    await writeFile(join(root, "AGENTS.md"), "<!-- skies-node:foundations:start -->\nbroken\n");
    await expect(installFoundationAssets({ root, operation: "init" })).rejects.toThrow("malformed");
    await expect(readFile(join(root, "csm.toml"), "utf8")).rejects.toMatchObject({ code: "ENOENT" });
  });
});

describe("repository-local CSM records", () => {
  it("writes, updates and reads explicit WTW, RTW, NYA and NWC text records", async () => {
    const root = await workspace();
    await installFoundationAssets({ root, operation: "init" });
    await writeCsmRecord(root, "wtw", { id: "server-authority", title: "Server authority", statement: "Server owns totals.", kind: "invariant", violation: "Client totals accepted." });
    await writeCsmRecord(root, "rtw", { id: "test-location", title: "Co-locate tests", statement: "Tests stay under src." });
    await writeCsmRecord(root, "nya", { id: "magic-color", title: "Magic color", statement: "Use a semantic token." });
    await writeCsmRecord(root, "nwc", { id: "retry", title: "Retry work", statement: "Finish retry handling." });
    expect((await readCsmRecords(root, "wtw")).records[0]).toMatchObject({ id: "server-authority", kind: "invariant" });
    expect((await readCsmRecords(root, "rtw")).records[0]?.statement).toContain("under src");
    expect((await readCsmRecords(root, "nya")).records).toHaveLength(1);
    expect((await readCsmRecords(root, "nwc")).records[0]?.status).toBe("open");
    await resolveDeferment(root, "retry");
    expect((await readCsmRecords(root, "nwc")).records[0]?.status).toBe("resolved");
  });

  it("supports idempotent dry-run records and rejects unsafe identifiers", async () => {
    const root = await workspace();
    await installFoundationAssets({ root, operation: "init" });
    const actions = await writeCsmRecord(root, "rtw", { id: "safe", title: "Safe", statement: "Stay safe." }, true);
    expect(actions[0]?.action).toBe("create");
    expect((await readCsmRecords(root, "rtw")).records).toHaveLength(0);
    await expect(writeCsmRecord(root, "rtw", { id: "../escape", title: "Bad", statement: "Bad." })).rejects.toThrow("record id");
  });

  it("context retrieves WTW, RTW, NYA, NWC in bounded order without occurrence bulk", async () => {
    const root = await workspace();
    await installFoundationAssets({ root, operation: "init" });
    await writeCsmRecord(root, "wtw", { id: "retry-policy", title: "Retry policy", statement: "Retry once.", kind: "decision" });
    await writeCsmRecord(root, "nwc", { id: "retry-work", title: "Retry work", statement: "Add retry status." });
    const context = await runContext({ root, task: "implement retry", limit: 1 });
    expect(context.steps.map((step) => step.id)).toEqual(["wtw", "rtw", "nya", "nwc"]);
    expect(context.steps[0]?.records[0]?.id).toBe("retry-policy");
    expect(context.steps[3]?.records[0]?.id).toBe("retry-work");
    expect(context.exitCode).toBe(0);
  });
});
