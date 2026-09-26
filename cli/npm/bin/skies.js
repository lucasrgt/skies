#!/usr/bin/env node
// Resolves the prebuilt binary from the platform package npm installed as an optional dependency and runs it
// with the caller's arguments and stdio, so `npx skies` behaves exactly like the native executable.
// npm installs only the optional dependency whose `os`/`cpu` match this machine, so the platform package is found
// by name, never downloaded here: there is no postinstall step to fail behind a proxy or an `--ignore-scripts`.
"use strict";

const { spawnSync } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");

// Must match the optionalDependencies in ../package.json and the matrix in .github/workflows/publish.yml.
const SUPPORTED = ["linux-x64", "linux-arm64", "darwin-x64", "darwin-arm64", "win32-x64"];

const target = `${process.platform}-${process.arch}`;
const platformPackage = `@skiesjs/cli-${target}`;
const executable = process.platform === "win32" ? "skies.exe" : "skies";

function fail(message) {
  console.error(`skies: ${message}`);
  process.exit(2);
}

if (!SUPPORTED.includes(target)) {
  fail(
    `no prebuilt binary for ${target} (built for ${SUPPORTED.join(", ")}).\n` +
      "Install from source instead: cargo install skies-cli",
  );
}

let binary;
try {
  binary = path.join(path.dirname(require.resolve(`${platformPackage}/package.json`)), executable);
} catch {
  fail(
    `${platformPackage} is not installed, so there is no binary for ${target}.\n` +
      "It is an optional dependency: reinstall without --no-optional / --omit=optional, " +
      "or install from source: cargo install skies-cli",
  );
}
if (!fs.existsSync(binary)) {
  fail(`${platformPackage} is installed but has no ${executable} (${binary}); reinstall @skiesjs/cli.`);
}

const result = spawnSync(binary, process.argv.slice(2), { stdio: "inherit" });
if (result.error) {
  fail(`could not run ${binary}: ${result.error.message}`);
}
if (result.signal) {
  process.kill(process.pid, result.signal);
}
process.exit(result.status ?? 2);
