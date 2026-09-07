import { randomUUID } from "node:crypto";
import { mkdir, open, readFile, unlink, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { FoundationError } from "./types.js";

const stop = "STOP — check whether you are in a loop. One automatic expensive attempt is allowed. Inspect the failure, correct its cause and run focused verification before supplying --retry-review <json-file> with PreviousAttemptId, Diagnosis, Correction and FocusedVerification.";

interface Receipt { Id: string; Status: string }

/** Enforces one automatic attempt, preserving interrupted runs and requiring a fresh diagnostic review for retries. */
export async function withAttempt<T extends { exitCode: number }>(
  directory: string, reviewPath: string | undefined, action: () => Promise<T>,
): Promise<T> {
  await mkdir(directory, { recursive: true });
  await writeFile(join(directory, ".gitignore"), "*\n");
  const lockPath = join(directory, "active.lock");
  const lease = await open(lockPath, "wx").catch(() => {
    throw new FoundationError(`${stop} Another attempt owns ${lockPath}; after an interruption, verify that its process has stopped before removing the lock.`, "invocation");
  });
  try {
    await lease.writeFile(String(process.pid));
    const path = join(directory, "attempt.json");
    const previousText = await readFile(path, "utf8").catch((error: NodeJS.ErrnoException) => {
      if (error.code === "ENOENT") return undefined;
      throw error;
    });
    const previous: Receipt | undefined = previousText === undefined ? undefined : JSON.parse(previousText) as Receipt;
    if (previousText !== undefined && (!previous || typeof previous.Id !== "string" || typeof previous.Status !== "string")) {
      throw new FoundationError(`${stop} Attempt receipt is malformed.`, "invocation");
    }
    let review: unknown;
    if (previous && previous.Status !== "passed") {
      if (reviewPath === undefined) throw new FoundationError(`${stop} Previous attempt: ${previous.Id} (${previous.Status}).`, "invocation");
      review = JSON.parse(await readFile(reviewPath, "utf8")) as unknown;
      const audit = review as Record<string, unknown> | null;
      if (!audit || audit.PreviousAttemptId !== previous.Id || ["Diagnosis", "Correction", "FocusedVerification"].some((key) => typeof audit[key] !== "string" || audit[key].trim().length === 0)) {
        throw new FoundationError(`${stop} Review must match attempt ${previous.Id} and contain all evidence fields.`, "invocation");
      }
    }
    const receipt = { Id: randomUUID(), Status: "interrupted-or-running", Started: new Date().toISOString(), Review: review };
    await writeFile(path, JSON.stringify(receipt));
    const result = await action();
    await writeFile(path, JSON.stringify({ ...receipt, Status: result.exitCode === 0 ? "passed" : `failed-exit-${result.exitCode}` }));
    return result;
  } finally {
    await lease.close();
    await unlink(lockPath);
  }
}
