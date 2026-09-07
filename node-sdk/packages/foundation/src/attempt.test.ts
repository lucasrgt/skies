import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { withAttempt } from "./attempt.js";

const roots: string[] = [];
async function workspace(): Promise<string> {
  const root = await mkdtemp(join(tmpdir(), "skies-attempt-"));
  roots.push(root);
  return root;
}
afterEach(async () => { await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true }))); });

describe("expensive verification attempts", () => {
  it("blocks automatic retries and accepts each diagnostic review only once", async () => {
    const root = await workspace();
    const failure = async () => ({ exitCode: 1 });
    await withAttempt(root, undefined, failure);
    await expect(withAttempt(root, undefined, failure)).rejects.toThrow("loop");
    const receipt = JSON.parse(await readFile(join(root, "attempt.json"), "utf8")) as { Id: string };
    const review = join(root, "review.json");
    await writeFile(review, JSON.stringify({ PreviousAttemptId: receipt.Id, Diagnosis: "Expired fixture", Correction: "Renew session", FocusedVerification: "Renewal regression passed" }));
    await withAttempt(root, review, failure);
    await expect(withAttempt(root, review, failure)).rejects.toThrow("Review must match");
  });

  it("blocks parallel execution and preserves an interrupted attempt", async () => {
    const root = await workspace();
    await expect(withAttempt(root, undefined, async () => {
      await expect(withAttempt(root, undefined, async () => ({ exitCode: 0 }))).rejects.toThrow("Another attempt");
      throw new Error("interrupted");
    })).rejects.toThrow("interrupted");
    await expect(withAttempt(root, undefined, async () => ({ exitCode: 0 }))).rejects.toThrow("interrupted-or-running");
  });

  it("allows future verification after a pass", async () => {
    const root = await workspace();
    await withAttempt(root, undefined, async () => ({ exitCode: 0 }));
    expect(await withAttempt(root, undefined, async () => ({ exitCode: 0 }))).toEqual({ exitCode: 0 });
  });
});
