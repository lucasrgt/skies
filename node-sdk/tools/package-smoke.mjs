import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { access, mkdir, mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { promisify } from "node:util";

const execute = promisify(execFile);
const sdkRoot = path.resolve(import.meta.dirname, "..");
const npm = process.platform === "win32" ? "npm.cmd" : "npm";
const childEnvironment = { ...process.env, NO_COLOR: "1" };
delete childEnvironment.FORCE_COLOR;

function compiledFiles(modules, extras = []) {
  return [
    "README.md",
    ...modules.flatMap((module) => [
      `dist/${module}.d.ts`,
      `dist/${module}.d.ts.map`,
      `dist/${module}.js`,
      `dist/${module}.js.map`,
    ]),
    ...extras,
    "package.json",
  ];
}

const packageDefinitions = [
  { directory: "packages/core", name: "@skiesjs/core", files: compiledFiles([
    "index", "ordered-lifecycle", "page", "result", "scalar-codec", "validation",
  ]) },
  { directory: "packages/openapi", name: "@skiesjs/openapi", files: compiledFiles([
    "document", "index", "registry", "scalar", "types",
  ]) },
  { directory: "packages/express", name: "@skiesjs/express", files: compiledFiles([
    "http", "index", "map-slice", "serve-openapi",
  ]) },
  { directory: "packages/auth", name: "@skiesjs/auth", files: compiledFiles(["index"]) },
  { directory: "packages/auth-express", name: "@skiesjs/auth-express", files: compiledFiles([
    "index", "refresh-cookie", "require-jwt",
  ]) },
  { directory: "packages/socketio", name: "@skiesjs/socketio", files: compiledFiles([
    "adapter", "auth", "contract", "index",
  ]) },
  { directory: "packages/identity", name: "@skiesjs/identity", files: compiledFiles(["index"]) },
  { directory: "packages/mail", name: "@skiesjs/mail", files: compiledFiles(["index"]) },
  { directory: "packages/sms", name: "@skiesjs/sms", files: compiledFiles(["index"]) },
  { directory: "packages/storage", name: "@skiesjs/storage", files: compiledFiles(["index"]) },
  { directory: "packages/storage-express", name: "@skiesjs/storage-express", files: compiledFiles(["index"]) },
  { directory: "packages/rate-limit-express", name: "@skiesjs/rate-limit-express", files: compiledFiles(["index"]) },
  { directory: "packages/drizzle-postgres", name: "@skiesjs/drizzle-postgres", files: compiledFiles(["index", "mutation", "page", "raw-sql"]) },
  { directory: "packages/testing", name: "@skiesjs/testing", files: compiledFiles(["index", "reporter"]) },
  { directory: "packages/testing-postgres", name: "@skiesjs/testing-postgres", files: compiledFiles([
    "index", "internal", "runtime",
  ]) },
  { directory: "packages/doctor", name: "@skiesjs/doctor", files: compiledFiles([
    "base-rules", "contract-facts", "contract-rules", "error-code-rules", "index", "journey-rules",
    "proof-rules", "rule-types", "rule-utils", "rules", "scan", "types",
  ], ["bin/skies-node-doctor.mjs"]) },
  {
    directory: "packages/eslint-plugin-skies-node",
    name: "@skiesjs/eslint-plugin-node",
    files: [
      "README.md", "index.d.ts", "index.js", "lib/ast.js", "package.json",
      "rules/error-code-registry.js", "rules/explicit-slice-contract.js", "rules/file-size.js",
      "rules/no-repository.js", "rules/require-slice-test.js", "rules/slice-shape.js",
      "rules/tests-under-source.js", "rules/thin-map.js",
    ],
  },
  { directory: "packages/framework", name: "@skiesjs/framework", files: ["README.md", "package.json"] },
  { directory: "packages/foundation", name: "@skiesjs/foundation", files: compiledFiles([
    "assets", "attempt", "cli", "config", "csm", "gate", "index", "inventory", "runner", "safe-fs", "suppression", "types", "workflow",
  ], ["bin/skies-node-foundation.mjs"]) },
  { directory: "packages/cli", name: "@skiesjs/cli", files: compiledFiles([
    "file-plan", "generate-application", "generators-auth-augment", "generators-domain", "generators-hub",
    "generators-persistence", "generators", "index", "naming", "proof-registry", "registry", "slice-registry",
    "templates-application", "templates-auth-augment", "templates-auth-proofs", "templates-crud", "templates-domain",
    "templates-hub", "templates-persistence", "templates", "types",
  ], ["bin/skies-node.mjs"]) },
];

const externalPackages = [
  "express",
  "eslint",
  "@typescript-eslint/parser",
  "typescript",
  "@types/express",
  "@types/node",
  "zod",
  "jose",
  "cookie",
  "mime",
  "express-rate-limit",
  "socket.io",
  "drizzle-orm",
  "postgres",
  "@testcontainers/postgresql",
  "vitest",
  "supertest",
  "@types/supertest",
];

function commandText(file, args) {
  return [file, ...args].map((part) => JSON.stringify(part)).join(" ");
}

async function run(file, args, cwd, expectedExitCode = 0) {
  try {
    const result = await execute(file, args, {
      cwd,
      encoding: "utf8",
      env: childEnvironment,
      maxBuffer: 10 * 1024 * 1024,
      // Windows resolves npm only as a .cmd script, which requires a shell.
      shell: process.platform === "win32" && /\.(cmd|bat)$/i.test(file),
    });
    assert.equal(
      expectedExitCode,
      0,
      `${commandText(file, args)} exited successfully; expected exit code ${expectedExitCode}`,
    );
    return { exitCode: 0, stdout: result.stdout, stderr: result.stderr };
  } catch (caught) {
    if (typeof caught?.code === "number" && caught.code === expectedExitCode) {
      return {
        exitCode: caught.code,
        stdout: caught.stdout ?? "",
        stderr: caught.stderr ?? "",
      };
    }

    const stdout = typeof caught?.stdout === "string" ? caught.stdout : "";
    const stderr = typeof caught?.stderr === "string" ? caught.stderr : "";
    throw new Error(
      `${commandText(file, args)} failed${stdout ? `\nstdout:\n${stdout}` : ""}${stderr ? `\nstderr:\n${stderr}` : ""}`,
      { cause: caught },
    );
  }
}

async function writeConsumerFiles(consumerDirectory) {
  const files = {
    "package.json": `${JSON.stringify({ name: "skies-package-smoke-consumer", private: true, type: "module" }, null, 2)}\n`,
    "runtime.mjs": `import assert from "node:assert/strict";
import { ErrorKind, Errors, Result, Validation } from "@skiesjs/core";
import { createOpenApiRegistry, scalarSchema } from "@skiesjs/openapi";
import { endpoint, mapSlice, toHttp } from "@skiesjs/express";
import { AccessTokens } from "@skiesjs/auth";
import { RefreshCookie, requireJwt } from "@skiesjs/auth-express";
import { accessTokenAuthentication, createSocketIoAdapter, defineSocketEvent } from "@skiesjs/socketio";
import { FakeExternalIdentity } from "@skiesjs/identity";
import { ConsoleEmailSender } from "@skiesjs/mail";
import { ConsoleSmsSender } from "@skiesjs/sms";
import { LocalFileStorage } from "@skiesjs/storage";
import { mapLocalFiles } from "@skiesjs/storage-express";
import { createRateLimiter } from "@skiesjs/rate-limit-express";
import { defineRawSql, executeVersionedMutation, pagePolicy, toPage } from "@skiesjs/drizzle-postgres";
import { startTestHost } from "@skiesjs/testing";
import { PostgresTestHarness } from "@skiesjs/testing-postgres";
import { inspectWorkspace } from "@skiesjs/doctor";
import { buildInventory } from "@skiesjs/foundation";
import skiesNode from "@skiesjs/eslint-plugin-node";
import { generateSlice, run } from "@skiesjs/cli";

assert.deepEqual(Result.ok("ready"), { ok: true, value: "ready" });
assert.equal(Errors.notFound("smoke.missing", "Missing").kind, ErrorKind.NotFound);
assert.equal(new Validation().failed, false);
for (const value of [
  createOpenApiRegistry, scalarSchema, endpoint, mapSlice, toHttp, AccessTokens, RefreshCookie, requireJwt,
  accessTokenAuthentication, createSocketIoAdapter, defineSocketEvent, FakeExternalIdentity, ConsoleEmailSender, ConsoleSmsSender, LocalFileStorage, mapLocalFiles,
  createRateLimiter, defineRawSql, executeVersionedMutation, pagePolicy, toPage, startTestHost, PostgresTestHarness, inspectWorkspace, buildInventory, generateSlice, run,
]) assert.equal(typeof value, "function");
assert.equal(skiesNode.configs["flat/recommended"].name, "skies-node/recommended");
`,
    "consumer.ts": `import express, { type Router } from "express";
import type { Linter } from "eslint";
import { ErrorKind, Errors, Result, type Page, type SkiesError } from "@skiesjs/core";
import { defineContract, type OpenApiRegistry } from "@skiesjs/openapi";
import { endpoint, type ErrorBody, type ResultHandler } from "@skiesjs/express";
import type { AccessTokens, CurrentUser } from "@skiesjs/auth";
import type { AuthenticatedLocals, RefreshCookie } from "@skiesjs/auth-express";
import type { SocketEventContract, SocketIoAdapter } from "@skiesjs/socketio";
import type { ExternalIdentity } from "@skiesjs/identity";
import type { EmailSender } from "@skiesjs/mail";
import type { SmsSender } from "@skiesjs/sms";
import type { FileStorage } from "@skiesjs/storage";
import type { MapLocalFilesOptions } from "@skiesjs/storage-express";
import type { RateLimiterOptions } from "@skiesjs/rate-limit-express";
import type { OrderedPageRequest } from "@skiesjs/drizzle-postgres";
import type { JourneyDefinition, TestHost } from "@skiesjs/testing";
import type { PostgresTestHarness } from "@skiesjs/testing-postgres";
import type { InspectionResult } from "@skiesjs/doctor";
import type { FoundationConfig, GateReceipt } from "@skiesjs/foundation";
import skiesNode from "@skiesjs/eslint-plugin-node";
import { generateSlice, type GenerateSliceOptions } from "@skiesjs/cli";

const router: Router = express.Router();
const success = Result.ok({ message: "ready" });
const failure: SkiesError = Errors.notFound("smoke.missing", "Missing");
const handler: ResultHandler<{ readonly message: string }> = async () => success;
router.get("/smoke", endpoint(handler));
const config: Linter.Config = skiesNode.configs["flat/recommended"];
const body: ErrorBody = { error: ErrorKind.NotFound, code: failure.code, message: failure.message, fields: null };
const generatorOptions: GenerateSliceOptions = {
  cwd: ".", root: "src", module: "Billing", name: "CreateInvoice", method: "post", route: "/invoices",
};
const types: readonly unknown[] = [
  {} as Page<string>, {} as OpenApiRegistry, {} as AccessTokens, {} as CurrentUser,
  {} as AuthenticatedLocals, {} as RefreshCookie, {} as SocketEventContract, {} as SocketIoAdapter, {} as ExternalIdentity, {} as EmailSender,
  {} as SmsSender, {} as FileStorage, {} as MapLocalFilesOptions, {} as RateLimiterOptions,
  {} as OrderedPageRequest<unknown, unknown>, {} as JourneyDefinition, {} as TestHost<unknown>,
  {} as PostgresTestHarness, {} as InspectionResult, {} as FoundationConfig, {} as GateReceipt,
];
void [config, body, generatorOptions, generateSlice, defineContract, types];
`,
    "tsconfig.json": `${JSON.stringify(
      {
        compilerOptions: {
          target: "ES2022",
          module: "NodeNext",
          moduleResolution: "NodeNext",
          strict: true,
          noEmit: true,
          skipLibCheck: false,
        },
        include: ["consumer.ts", "src/**/*.slice.ts"],
        exclude: ["src/**/*.test.ts"],
      },
      null,
      2,
    )}\n`,
    "eslint.config.mjs": `import parser from "@typescript-eslint/parser";
import skiesNode from "@skiesjs/eslint-plugin-node";

export default [
  { files: ["**/*.ts"], languageOptions: { parser } },
  skiesNode.configs["flat/recommended"],
];
`,
    "doctor-app/src/modules.ts": `import * as Health from "./modules/health/health.module.js";
export function mapModules(router: object, openApi: object): void { Health.map(router, openApi); }
`,
    "doctor-app/src/modules/health/health.module.ts": `import * as Ping from "./ping.slice.js";
export function map(router: object, openApi: object): void { Ping.map(router, openApi); }
`,
    "doctor-app/src/modules/health/health.ctx.md": `## Boundaries
The module owns \`handle\` and \`contract\`.

## Design notes
Routes use \`map\` explicitly.
`,
    "doctor-app/src/modules/health/ping.slice.ts": `import type { Router } from "express";
import { Result } from "@skiesjs/core";
import { mapSlice } from "@skiesjs/express";
import { defineContract, type OpenApiRegistry } from "@skiesjs/openapi";
import { z } from "zod";
// @skies-criterion smoke.health
export const contract = defineContract({
  operationId: "Smoke.Health", method: "get", path: "/health", auth: "anonymous", kind: "internal",
  request: {}, success: { status: 200, output: z.object({ status: z.literal("ok") }) },
});
export type Input = Record<string, never>;
export interface Output { readonly status: "ok"; }
export async function handle(_input: Input): Promise<ReturnType<typeof Result.ok<Output>>> {
  return Result.ok({ status: "ok" });
}
export function map(router: Router, openApi: OpenApiRegistry): void {
  mapSlice(router, openApi, contract, { toInput: () => ({}), handle });
}
`,
    "doctor-app/src/modules/health/ping.slice.test.ts": `import { expect } from "vitest";
import { unit } from "@skiesjs/testing";
import { handle } from "./ping.slice.js";
// @skies-proof smoke.health
unit("Smoke.Health", async () => expect((await handle({})).ok).toBe(true));
`,
    "bad.slice.ts": `export const wrong = true;\n`,
  };

  await Promise.all(Object.entries(files).map(async ([name, contents]) => {
    const target = path.join(consumerDirectory, name);
    await mkdir(path.dirname(target), { recursive: true });
    await writeFile(target, contents, "utf8");
  }));
}

function lockedExternalSpecs(packageLock) {
  return externalPackages.map((name) => {
    const entry = packageLock.packages[`node_modules/${name}`];
    assert.equal(typeof entry?.version, "string", `${name} must have an exact version in package-lock.json`);
    return `${name}@${entry.version}`;
  });
}

async function packPackages(packDirectory) {
  const tarballs = [];

  for (const definition of packageDefinitions) {
    const { stdout } = await run(
      npm,
      ["pack", `./${definition.directory}`, "--json", "--pack-destination", packDirectory],
      sdkRoot,
    );
    const report = JSON.parse(stdout);
    assert.equal(report.length, 1, `${definition.name} must produce exactly one tarball`);
    assert.equal(report[0].name, definition.name);

    const actualFiles = report[0].files.map(({ path: file }) => file).sort();
    assert.deepEqual(actualFiles, [...definition.files].sort(), `${definition.name} tarball escaped its file allowlist`);

    const tarball = path.join(packDirectory, report[0].filename);
    await access(tarball);
    tarballs.push(tarball);
  }

  return tarballs;
}

async function main() {
  console.log("package smoke: build current packages");
  await run(npm, ["run", "build"], sdkRoot);

  const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), "skies-node-package-smoke-"));
  const packDirectory = path.join(temporaryRoot, "packs");
  const consumerDirectory = path.join(temporaryRoot, "consumer");

  try {
    await Promise.all([mkdir(packDirectory), mkdir(consumerDirectory)]);

    console.log("package smoke: pack and inspect public tarballs");
    const tarballs = await packPackages(packDirectory);
    await writeConsumerFiles(consumerDirectory);

    const packageLock = JSON.parse(await readFile(path.join(sdkRoot, "package-lock.json"), "utf8"));
    console.log("package smoke: install locked external tools");
    await run(
      npm,
      ["install", "--ignore-scripts", "--no-audit", "--no-fund", "--save-exact", ...lockedExternalSpecs(packageLock)],
      consumerDirectory,
    );

    console.log("package smoke: install local tarballs without registry access");
    await run(
      npm,
      ["install", "--offline", "--ignore-scripts", "--no-audit", "--no-fund", "--save-exact", ...tarballs],
      consumerDirectory,
    );

    console.log("package smoke: verify ESM exports and declarations");
    await run(process.execPath, ["runtime.mjs"], consumerDirectory);
    const typescript = path.join(consumerDirectory, "node_modules", "typescript", "bin", "tsc");
    await run(process.execPath, [typescript, "--project", "tsconfig.json"], consumerDirectory);

    console.log("package smoke: verify installed CLI");
    const help = await run(npm, ["exec", "--offline", "--", "skies-node", "--help"], consumerDirectory);
    assert.match(help.stdout, /Skies Node\.js/);
    assert.match(help.stdout, /Usage:/);
    const doctorHelp = await run(npm, ["exec", "--offline", "--", "skies-node-doctor", "--help"], consumerDirectory);
    assert.match(doctorHelp.stdout, /workspace doctor/);
    const foundationHelp = await run(npm, ["exec", "--offline", "--", "skies-node-foundation", "--help"], consumerDirectory);
    assert.match(foundationHelp.stdout, /Usage:/);
    await run(npm, ["exec", "--offline", "--", "skies-node-doctor", "doctor-app"], consumerDirectory);
    await run(
      npm,
      [
        "exec",
        "--offline",
        "--",
        "skies-node",
        "g",
        "slice",
        "Billing",
        "CreateInvoice",
        "--method",
        "post",
        "--route",
        "/invoices",
      ],
      consumerDirectory,
    );

    const generatedSlice = path.join("src", "modules", "billing", "slices", "create-invoice.slice.ts");
    const generatedTest = path.join("src", "modules", "billing", "slices", "create-invoice.slice.test.ts");
    assert.match(await readFile(path.join(consumerDirectory, generatedSlice), "utf8"), /defineContract\(\{/);
    assert.match(await readFile(path.join(consumerDirectory, generatedSlice), "utf8"), /mapSlice\(/);
    await access(path.join(consumerDirectory, generatedTest));
    await access(path.join(consumerDirectory, "src/modules/billing/slices/create-invoice.slice.journey.ts"));
    await run(process.execPath, [typescript, "--project", "tsconfig.json"], consumerDirectory);

    console.log("package smoke: verify installed flat ESLint preset");
    const eslint = path.join(consumerDirectory, "node_modules", "eslint", "bin", "eslint.js");
    await run(process.execPath, [eslint, generatedSlice], consumerDirectory);
    const badLint = await run(process.execPath, [eslint, "bad.slice.ts"], consumerDirectory, 1);
    assert.match(`${badLint.stdout}\n${badLint.stderr}`, /SKYN0001/);

    console.log("package smoke: verify installed fresh-application acceptance");
    await run(npm, ["exec", "--offline", "--", "skies-node", "new", "fresh-api"], consumerDirectory);
    const fresh = path.join(consumerDirectory, "fresh-api");
    await symlink(
      path.join(consumerDirectory, "node_modules"),
      path.join(fresh, "node_modules"),
      process.platform === "win32" ? "junction" : "dir",
    );
    await run(npm, ["run", "criteria"], fresh);
    await run(npm, ["run", "foundations:sync", "--", "--dry-run"], fresh);
    const fullGate = await run(npm, ["run", "gate:full"], fresh);
    assert.match(fullGate.stdout, /Gate verdict: GREEN/);
    assert.match(await readFile(path.join(fresh, "VERIFICATION.json"), "utf8"), /"verdict": "green"/);
    await access(path.join(fresh, ".skies/csm/lock.json"));

    console.log("package smoke: passed");
  } finally {
    await rm(temporaryRoot, { recursive: true, force: true, maxRetries: 3, retryDelay: 100 });
  }
}

await main();
