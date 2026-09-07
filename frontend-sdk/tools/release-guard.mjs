#!/usr/bin/env node
// Release guard — the enforcement half of "a published version is immutable". Run at publish time (tag push),
// it fails when a publishable unit's shipped content changed since the last release tag without its version
// moving: the "0.10.0 published != 0.10.0 canonical" drift, where source slid under a number that was already
// out (the publish step then silently skips it, so the change never ships and local diverges from the registry
// forever). The fix is always a bump, never a re-skip. The decision is pure (`violations`); the CLI feeds it git.

import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { FRONTEND_PACKAGE_VERSIONS } from "./package-versions.mjs";

/** Flutter versions mirrored here so the published release guard remains self-contained. */
export const FLUTTER_RELEASE_VERSIONS = Object.freeze([
  Object.freeze({ name: "skies-flutter", version: "4.1.24" }),
  Object.freeze({ name: "skies_flutter", version: "4.1.22" }),
]);

// The publishable surface. A unit "changed" if any shipped path differs since the tag; `version` is the file
// whose number must move when that happens. The NuGet packages share one number (the library props); each npm
// package carries its own. SelfHarness is framework-dev-only and never shipped, so it is not a unit here.
const NODE_PACKAGES = Object.freeze([
  ["core", "@skiesjs/core"],
  ["openapi", "@skiesjs/openapi"],
  ["express", "@skiesjs/express"],
  ["auth", "@skiesjs/auth"],
  ["auth-express", "@skiesjs/auth-express"],
  ["socketio", "@skiesjs/socketio"],
  ["identity", "@skiesjs/identity"],
  ["mail", "@skiesjs/mail"],
  ["sms", "@skiesjs/sms"],
  ["storage", "@skiesjs/storage"],
  ["storage-express", "@skiesjs/storage-express"],
  ["rate-limit-express", "@skiesjs/rate-limit-express"],
  ["drizzle-postgres", "@skiesjs/drizzle-postgres"],
  ["testing", "@skiesjs/testing"],
  ["testing-postgres", "@skiesjs/testing-postgres"],
  ["doctor", "@skiesjs/doctor"],
  ["eslint-plugin-skies-node", "@skiesjs/eslint-plugin-node"],
  ["foundation", "@skiesjs/foundation"],
  ["framework", "@skiesjs/framework"],
  ["cli", "@skiesjs/cli"],
]);

function nodeReleaseUnit([directory, name]) {
  const root = `node-sdk/packages/${directory}`;
  const version = `${root}/package.json`;
  return { name, version, paths: [root, version] };
}

export const RELEASE_UNITS = Object.freeze([
  {
    name: "Skies.Framework.* (nuget)",
    version: "build/Skies.Framework.Library.props",
    paths: ["src", ":(exclude)src/Skies.Framework.Cli", "analyzers/Skies.Framework.Doctor"],
  },
  {
    name: "skies-framework-cli",
    version: "src/Skies.Framework.Cli/Skies.Framework.Cli.csproj",
    paths: ["src/Skies.Framework.Cli"],
  },
  {
    name: "@skiesjs/react",
    version: "frontend-sdk/packages/skies-react/package.json",
    paths: ["frontend-sdk/packages/skies-react/src", "frontend-sdk/packages/skies-react/package.json"],
  },
  {
    name: "@skiesjs/eslint-plugin",
    version: "frontend-sdk/packages/eslint-plugin/package.json",
    paths: [
      "frontend-sdk/packages/eslint-plugin/index.cjs",
      "frontend-sdk/packages/eslint-plugin/README.md",
      "frontend-sdk/packages/eslint-plugin/package.json",
    ],
  },
  {
    name: "@skiesjs/frontend-sdk",
    version: "frontend-sdk/package.json",
    paths: ["frontend-sdk/tools", "frontend-sdk/README.md", "frontend-sdk/package.json"],
  },
  {
    name: "skies-flutter",
    version: "flutter-sdk/package.json",
    paths: ["flutter-sdk/tools", "flutter-sdk/parity", "flutter-sdk/README.md", "flutter-sdk/package.json"],
  },
  {
    name: "skies_flutter",
    version: "flutter-sdk/packages/skies_flutter/pubspec.yaml",
    paths: ["flutter-sdk/packages/skies_flutter"],
  },
  ...NODE_PACKAGES.map(nodeReleaseUnit),
]);

/**
 * The pure decision: a unit that changed since the last release without bumping its version is a violation.
 * @param {ReadonlyArray<{name: string, changed: boolean, versionBumped: boolean}>} units
 * @returns {string[]} one message per violation (empty array = clean to publish)
 */
export function violations(units) {
  return units
    .filter((unit) => unit.changed && !unit.versionBumped)
    .map((unit) =>
      `release-guard: ${unit.name} changed since the last release but its version did not move — bump it before `
      + "tagging. A published version is immutable; shipping new content under the same number is the "
      + "'0.x published != 0.x canonical' drift.",
    );
}

// Where each canonical entry's own package.json lives (repo-root relative, like RELEASE_UNITS).
const CANONICAL_MANIFESTS = Object.freeze({
  "@skiesjs/frontend-sdk": "frontend-sdk/package.json",
  "assay-design": "frontend-sdk/node_modules/assay-design/package.json",
  "avp-assay": "frontend-sdk/node_modules/avp-assay/package.json",
  "@skiesjs/react": "frontend-sdk/packages/skies-react/package.json",
  "@skiesjs/eslint-plugin": "frontend-sdk/packages/eslint-plugin/package.json",
  "skies-flutter": "flutter-sdk/package.json",
  "skies_flutter": "flutter-sdk/packages/skies_flutter/pubspec.yaml",
});

/**
 * The canonical table must agree with the packages it describes: FRONTEND_PACKAGE_VERSIONS ships INSIDE
 * @skiesjs/frontend-sdk and is what the framework sync command holds every pilot to — a release where the
 * table lags its own package (0.1.2 shipped saying "canonical is 0.1.1") turns the sync gate red in every
 * pilot for the wrong reason. Caught here, at publish time, where the fix is one line.
 * @param {ReadonlyArray<{name: string, version: string}>} canonical
 * @param {(path: string) => string} readManifest
 * @returns {string[]}
 */
export function canonicalDrift(canonical, readManifest) {
  return canonical.flatMap(({ name, version }) => {
    const path = CANONICAL_MANIFESTS[name];
    if (!path) return [`release-guard: ${name} is in FRONTEND_PACKAGE_VERSIONS but has no known manifest path.`];
    const content = readManifest(path);
    const actual = path.endsWith(".yaml") ? versionOf(content, path) : JSON.parse(content).version;
    return actual === version
      ? []
      : [
        `release-guard: FRONTEND_PACKAGE_VERSIONS says ${name} is ${version} but ${path} says ${actual} — `
        + "the canonical table ships inside the sdk and gates every pilot; keep it equal to the versions being published.",
      ];
  });
}

/** @param {string} content @param {string} versionPath */
function versionOf(content, versionPath) {
  if (/\.(?:props|csproj)$/.test(versionPath)) return content.match(/<Version>([^<]+)<\/Version>/)?.[1] ?? "";
  if (/\.ya?ml$/.test(versionPath)) return content.match(/^version:\s*['"]?([^'"\s]+)['"]?\s*$/m)?.[1] ?? "";
  try {
    return JSON.parse(content).version ?? "";
  } catch {
    return "";
  }
}

/** The file's content at a git ref, or "" when it did not exist there. @param {string} ref @param {string} path */
function fileAt(ref, path) {
  try {
    return execFileSync("git", ["show", `${ref}:${path}`], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
    });
  } catch {
    return "";
  }
}

/** True when any of `paths` differs between `ref` and HEAD. @param {string} ref @param {string[]} paths */
function changedSince(ref, paths) {
  try {
    execFileSync("git", ["diff", "--quiet", ref, "HEAD", "--", ...paths]);
    return false; // exit 0 = no differences
  } catch {
    return true; // non-zero exit = differences
  }
}

const invokedDirectly = process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1];
if (invokedDirectly) {
  // Compare against the most recent release tag reachable from HEAD's parent, so a freshly-pushed v* tag at HEAD
  // measures against the PREVIOUS release rather than itself.
  let base = process.argv[2];
  if (!base) {
    try {
      base = execFileSync("git", ["describe", "--tags", "--abbrev=0", "--match", "v*", "HEAD^"], { encoding: "utf8" }).trim();
    } catch {
      console.log("release-guard: no prior v* tag to compare against — first release, nothing to guard.");
      process.exit(0);
    }
  }

  const units = RELEASE_UNITS.map((unit) => ({
    name: unit.name,
    changed: changedSince(base, unit.paths),
    versionBumped: versionOf(fileAt(base, unit.version), unit.version) !== versionOf(readFileSync(unit.version, "utf8"), unit.version),
  }));

  const messages = [
    ...violations(units),
    ...canonicalDrift([...FRONTEND_PACKAGE_VERSIONS, ...FLUTTER_RELEASE_VERSIONS], (path) => readFileSync(path, "utf8")),
  ];
  for (const message of messages) console.error(message);
  if (messages.length === 0) console.log(`release-guard: every changed publishable unit was bumped since ${base}.`);
  process.exit(messages.length ? 1 : 0);
}
