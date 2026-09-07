export function applicationPackageSource(name: string): string {
  return `${JSON.stringify({
    name,
    version: "0.1.0",
    private: true,
    type: "module",
    engines: { node: ">=24" },
    scripts: {
      build: "tsc -p tsconfig.json",
      typecheck: "tsc -p tsconfig.test.json",
      lint: "eslint .",
      test: "vitest run",
      doctor: "skies-node-doctor .",
      check: "npm run lint && npm run typecheck && npm test && npm run doctor && npm run build",
      proofs: "skies-node-foundation inventory",
      criteria: "skies-node-foundation criteria check",
      "gate:affected": "skies-node-foundation gate --affected",
      "gate:staged": "skies-node-foundation gate --staged",
      "gate:base": "skies-node-foundation gate --base origin/main --fast",
      "gate:full": "skies-node-foundation gate --full",
      "foundations:sync": "skies-node-foundation foundations sync",
      "hooks:install": "git config core.hooksPath .githooks",
      start: "node dist/server.js",
    },
    dependencies: {
      "@skiesjs/auth": "0.1.0",
      "@skiesjs/auth-express": "0.1.0",
      "@skiesjs/core": "0.1.0",
      "@skiesjs/drizzle-postgres": "0.1.0",
      "@skiesjs/express": "0.1.0",
      "@skiesjs/openapi": "0.1.0",
      "@skiesjs/storage": "0.1.0",
      "@skiesjs/storage-express": "0.1.0",
      "drizzle-orm": "^0.45.2",
      express: "^5.0.0",
      postgres: "^3.4.9",
      zod: "^4.0.0",
    },
    devDependencies: {
      "@skiesjs/doctor": "^0.1.1",
      "@skiesjs/foundation": "0.1.2",
      "@skiesjs/testing": "0.1.0",
      "@types/express": "^5.0.0",
      "@types/node": "^24.0.0",
      "@types/supertest": "^6.0.0",
      "@typescript-eslint/parser": "^8.0.0",
      eslint: "^9.0.0",
      "@skiesjs/eslint-plugin-node": "^0.1.1",
      supertest: "^7.0.0",
      typescript: "^5.7.0",
      vitest: "^4.0.0",
    },
  }, undefined, 2)}
`;
}

export function applicationTsconfigSource(): string {
  return `${JSON.stringify({
    compilerOptions: {
      target: "ES2022",
      module: "NodeNext",
      moduleResolution: "NodeNext",
      strict: true,
      noUncheckedIndexedAccess: true,
      exactOptionalPropertyTypes: true,
      verbatimModuleSyntax: true,
      declaration: true,
      sourceMap: true,
      skipLibCheck: true,
      rootDir: "src",
      outDir: "dist",
      types: ["node"],
    },
    include: ["src/**/*.ts"],
    exclude: ["src/**/*.test.ts", "dist", "node_modules"],
  }, undefined, 2)}
`;
}

export function applicationTestTsconfigSource(): string {
  return `${JSON.stringify({
    extends: "./tsconfig.json",
    compilerOptions: { noEmit: true, types: ["node", "vitest/globals"] },
    include: ["src/**/*.ts"],
    exclude: ["dist", "node_modules"],
  }, undefined, 2)}
`;
}

export function eslintConfigSource(): string {
  return `import tsParser from "@typescript-eslint/parser";
import skiesNode from "@skiesjs/eslint-plugin-node";

export default [
  {
    ...skiesNode.configs["flat/recommended"],
    files: ["src/**/*.ts"],
    languageOptions: { parser: tsParser },
  },
];
`;
}


export function vitestConfigSource(): string {
  return `import { SkiesProofReporter } from "@skiesjs/testing/reporter";
import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    maxWorkers: 2,
    include: ["src/**/*.{test,spec,proof,avp,journey}.ts"],
    reporters: [
      "default",
      new SkiesProofReporter({
        requireMetadata: true,
        outputFile: ".skies/foundation/vitest-receipt.json",
      }),
    ],
  },
});
`;
}

export function applicationReadmeSource(name: string): string {
  return `# ${name}

A plain NodeNext TypeScript, Express, and Skies Node.js application.

\`\`\`bash
npm install
npm run hooks:install
npm run check
npm run gate:affected -- --changed src/modules/health/ping.slice.ts
npm run gate:full
npm start
\`\`\`

The service exposes \`GET /health\` and its complete OpenAPI document at \`GET /openapi/v1.json\`.
Routes are registered explicitly through \`src/modules.ts\`; there is no discovery or generated runtime behavior.
The checked-in \`skies.node.json\` is the closed proof inventory. Full gates write \`VERIFICATION.json\` and
\`VERIFICATION.md\`; repository-local CSM assets are pinned under \`.skies/csm\` and can be updated only with
\`npm run foundations:sync\`. Commit the lockfile created by the first \`npm install\`.
`;
}

export function appSource(title: string): string {
  return `import express from "express";
import { serveOpenApi } from "@skiesjs/express";
import { createOpenApiRegistry } from "@skiesjs/openapi";
import { mapModules } from "./modules.js";

export const app = express();
export const openApi = createOpenApiRegistry({ title: ${JSON.stringify(title)}, version: "0.1.0" });

app.use(express.json());
mapModules(app, openApi);
app.get("/openapi/v1.json", serveOpenApi(openApi));
`;
}

export function serverSource(): string {
  return `import { app } from "./app.js";

const port = Number(process.env["PORT"] ?? "3000");
if (!Number.isSafeInteger(port) || port < 1 || port > 65_535) {
  throw new Error("PORT must be an integer from 1 through 65535");
}

app.listen(port, () => {
  console.log(\`listening on http://localhost:\${port}\`);
});
`;
}

export function modulesSource(): string {
  return `import type { Express } from "express";
import type { OpenApiRegistry } from "@skiesjs/openapi";
import * as Health from "./modules/health/health.module.js";

export function mapModules(app: Express, openApi: OpenApiRegistry): void {
  Health.map(app, openApi);
}
`;
}

export function healthModuleSource(): string {
  return `import { Router, type Express } from "express";
import type { OpenApiRegistry } from "@skiesjs/openapi";
import * as Ping from "./ping.slice.js";

export function map(app: Express, openApi: OpenApiRegistry): void {
  const router = Router();
  Ping.map(router, openApi);
  app.use(router);
}
`;
}

export function healthSliceSource(): string {
  return `import type { Router } from "express";
import { Result } from "@skiesjs/core";
import { mapSlice } from "@skiesjs/express";
import { defineContract, type OpenApiRegistry } from "@skiesjs/openapi";
import { z } from "zod";

// @skies-criterion health.ping.responds
export const contract = defineContract({
  operationId: "HealthPing",
  method: "get",
  path: "/health",
  auth: "anonymous",
  kind: "internal",
  request: {},
  success: { status: 200, output: z.object({ status: z.literal("ok") }) },
});

export type Input = Record<string, never>;

export interface Output {
  readonly status: "ok";
}

export async function handle(_input: Input): Promise<Result<Output>> {
  return Result.ok({ status: "ok" });
}

export function map(router: Router, openApi: OpenApiRegistry): void {
  mapSlice(router, openApi, contract, { toInput: () => ({}), handle });
}
`;
}

export function healthSliceTestSource(): string {
  return `import request from "supertest";
import { expect } from "vitest";
import { e2e, unit } from "@skiesjs/testing";
import { app } from "../../app.js";
import * as Ping from "./ping.slice.js";

// @skies-proof health.ping.responds
unit("runs without HTTP", async () => {
  await expect(Ping.handle({})).resolves.toEqual({ ok: true, value: { status: "ok" } });
});

e2e("maps the route and publishes its explicit contract", async () => {
  const response = await request(app).get("/health");
  const document = await request(app).get("/openapi/v1.json");

  expect(response.status).toBe(200);
  expect(response.body).toEqual({ status: "ok" });
  expect(document.body.paths["/health"].get.operationId).toBe("HealthPing");
});
`;
}

export function healthContextSource(): string {
  return `# Health

Health owns the service liveness response and nothing else.

## Boundaries

- **Inside:** Report that the process can serve requests.
- **Outside:** Keep dependency health and business readiness with their owning modules.

## Design notes

The response is deliberately static so it has no hidden infrastructure dependency.
`;
}


export function proofManifestSource(): string {
  return `${JSON.stringify({
    schemaVersion: 1,
    git: { base: "origin/main" },
    criteria: [{ id: "health.ping.responds", statement: "The service reports liveness through its explicit contract." }],
    lanes: [{
      id: "application-check",
      command: ["npm", "run", "check"],
      timeoutMs: 120000,
      cwd: ".",
      env: { NODE_ENV: "test" },
    }],
    proofs: [{
      id: "health-ping-unit",
      kind: "unit",
      lane: "application-check",
      criteria: ["health.ping.responds"],
      sourceScopes: ["src/modules/health/**", "src/app.ts", "src/modules.ts"],
      dependsOn: [],
      description: "Direct handler and real HTTP/OpenAPI proof",
    }],
    ignoreScopes: ["README.md"],
    forceFullScopes: [
      "package.json", "package-lock.json", "skies.node.json", "tsconfig*.json", "eslint.config.js",
      "vitest.config.ts", "src/modules.ts",
    ],
  }, undefined, 2)}
`;
}

export function nodeCiSource(): string {
  return `name: node-api

on:
  workflow_call:
    inputs:
      full:
        description: run the exhaustive release gate
        required: false
        default: false
        type: boolean
  pull_request:
  push:
    branches: [main]
  workflow_dispatch:
    inputs:
      full:
        description: run the exhaustive release gate
        required: false
        default: false
        type: boolean

jobs:
  verify:
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, windows-latest, macos-latest]
    runs-on: \${{ matrix.os }}
    steps:
      - uses: actions/checkout@v7
        with:
          fetch-depth: 0
      - uses: actions/setup-node@v7
        with:
          node-version: "24"
          cache: npm
      - run: npm ci
      - name: affected verification
        if: \${{ !inputs.full }}
        run: npm run gate:affected
      - name: exhaustive release verification
        if: \${{ inputs.full }}
        run: npm run gate:full
      - if: runner.os == 'Linux'
        uses: actions/upload-artifact@v6
        with:
          name: verification
          path: |
            VERIFICATION.json
            VERIFICATION.md
          if-no-files-found: error
`;
}

export function preCommitHookSource(): string {
  return `#!/usr/bin/env sh
set -eu
npm run gate:staged
`;
}

export function prePushHookSource(): string {
  return `#!/usr/bin/env sh
set -eu
npm run gate:base
`;
}
