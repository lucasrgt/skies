import { readFile } from "node:fs/promises";
import { isAbsolute, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const DEFAULT_TIMEOUT_MS = 30_000;
const DEFAULT_INTERVAL_MS = 250;
const HTTP_METHODS = new Set(["delete", "get", "head", "options", "patch", "post", "put", "trace"]);
const observations = new WeakSet();

/**
 * Poll a real HTTP endpoint and mark it ready for backend-bound Playwright cases.
 * The caller owns starting the API and database; this helper only proves the configured endpoint answered.
 */
export async function probeBackend({
  env = process.env,
  fetchImpl = globalThis.fetch,
  path = "/ping",
  timeoutMs = DEFAULT_TIMEOUT_MS,
  intervalMs = DEFAULT_INTERVAL_MS,
} = {}) {
  const baseUrl = configuredBackendUrl(env);
  const probeUrl = new URL(path, ensureTrailingSlash(baseUrl)).toString();
  const deadline = Date.now() + timeoutMs;
  let lastFailure;

  do {
    try {
      const response = await fetchImpl(probeUrl, { method: "GET", redirect: "manual" });
      if (response.ok) {
        env.PW_API_READY = "1";
        return baseUrl;
      }
      lastFailure = new Error(`HTTP ${response.status}`);
    } catch (error) {
      lastFailure = error;
    }

    if (Date.now() < deadline && intervalMs > 0) {
      await new Promise((resolveDelay) => setTimeout(resolveDelay, intervalMs));
    }
  } while (Date.now() < deadline);

  delete env.PW_API_READY;
  const detail = lastFailure instanceof Error ? lastFailure.message : String(lastFailure ?? "no response");
  throw new Error(`Skies E2E backend probe failed at ${probeUrl}: ${detail}`);
}

/** Create a Playwright globalSetup function backed by the canonical real-API probe. */
export function createBackendGlobalSetup(options = {}) {
  return async function backendGlobalSetup() {
    await probeBackend(options);
  };
}

/**
 * Observe the page's real HTTP responses and resolve them to slice names through the generated OpenAPI contract.
 * Call this before navigating or interacting, then close the proof with expectBackendSlices after the UI settles.
 */
export async function observeBackend(page, contract, env = process.env) {
  const backendUrl = requireReadyBackend(env);
  if (!page || typeof page.on !== "function") {
    throw new Error("observeBackend() requires the Playwright page used by the visible journey.");
  }

  const document = await readContract(contract);
  const operations = compileOperations(document);
  const seen = [];
  page.on("response", (response) => {
    const request = response.request();
    const operation = matchOperation(operations, request.method(), response.url(), backendUrl);
    if (operation) seen.push({ operationId: operation.operationId, status: response.status() });
  });

  const observation = { operations, seen };
  observations.add(observation);
  return observation;
}

/**
 * Prove that the visible browser journey reached each declared slice with the expected HTTP outcome.
 * The observation is branded inside this module, so a local namesake or plain object cannot manufacture evidence.
 */
export function expectBackendSlices(observation, slices, { status } = {}) {
  validateBackendExpectation(observation, slices, status, "expectBackendSlices");
  const missing = missingBackendSlices(observation, slices, status);
  if (missing.length === 0) return;

  throw missingBackendError(observation, missing, status);
}

/**
 * Wait until every named slice produces the expected HTTP outcome in the visible journey.
 * Use this when the terminal UI can render before parallel backend requests have settled.
 */
export async function waitForBackendSlices(
  observation,
  slices,
  { status, timeoutMs = DEFAULT_TIMEOUT_MS, intervalMs = DEFAULT_INTERVAL_MS } = {},
) {
  validateBackendExpectation(observation, slices, status, "waitForBackendSlices");
  if (!Number.isFinite(timeoutMs) || timeoutMs < 0) {
    throw new Error("waitForBackendSlices() timeoutMs must be a non-negative finite number.");
  }
  if (!Number.isFinite(intervalMs) || intervalMs < 0) {
    throw new Error("waitForBackendSlices() intervalMs must be a non-negative finite number.");
  }

  const deadline = Date.now() + timeoutMs;
  let missing = missingBackendSlices(observation, slices, status);
  while (missing.length > 0 && Date.now() < deadline) {
    await new Promise((resolveDelay) => setTimeout(resolveDelay, Math.min(intervalMs, deadline - Date.now())));
    missing = missingBackendSlices(observation, slices, status);
  }
  if (missing.length > 0) throw missingBackendError(observation, missing, status);
}

function validateBackendExpectation(observation, slices, status, caller) {
  if (!observations.has(observation)) {
    throw new Error(`${caller}() requires the observation returned by observeBackend().`);
  }
  if (!Array.isArray(slices) || slices.length === 0 || slices.some((slice) => typeof slice !== "string" || !slice)) {
    throw new Error(`${caller}() requires one or more slice names.`);
  }
  if (new Set(slices).size !== slices.length) {
    throw new Error(`${caller}() does not accept duplicate slice names.`);
  }
  if (status !== "success" && status !== "error") {
    throw new Error(`${caller}() status must be "success" or "error".`);
  }

  const known = new Set(observation.operations.map((operation) => operation.operationId));
  const unknown = slices.filter((slice) => !known.has(slice));
  if (unknown.length > 0) {
    throw new Error(`OpenAPI contract does not declare slice operation(s): ${unknown.join(", ")}.`);
  }
}

function missingBackendSlices(observation, slices, status) {
  const matchesStatus = status === "success"
    ? (code) => code >= 200 && code < 300
    : (code) => code >= 400 && code < 600;
  return slices.filter((slice) => !observation.seen.some(
    (entry) => entry.operationId === slice && matchesStatus(entry.status),
  ));
}

function missingBackendError(observation, missing, status) {
  const evidence = observation.seen.length === 0
    ? "no contract operation was observed"
    : observation.seen.map((entry) => `${entry.operationId}:${entry.status}`).join(", ");
  return new Error(
    `Browser journey did not prove ${status} response(s) for: ${missing.join(", ")}. Observed: ${evidence}.`,
  );
}

function requireReadyBackend(env) {
  const url = configuredBackendUrl(env);
  if (env.PW_API_READY !== "1") {
    throw new Error(
      "Skies E2E backend is not ready. Run probeBackend() from Playwright globalSetup before the suite.",
    );
  }
  return url;
}

async function readContract(source) {
  if (source && typeof source === "object" && !(source instanceof URL)) return validateContract(source);
  if (typeof source !== "string" && !(source instanceof URL)) {
    throw new Error("observeBackend() requires an OpenAPI document path, URL, or object.");
  }

  const path = source instanceof URL
    ? fileURLToPath(source)
    : isAbsolute(source) ? source : resolve(process.cwd(), source);
  let parsed;
  try {
    parsed = JSON.parse(await readFile(path, "utf8"));
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    throw new Error(`Could not read OpenAPI contract at ${path}: ${detail}`);
  }
  return validateContract(parsed);
}

function validateContract(document) {
  if (!document || typeof document !== "object" || !document.paths || typeof document.paths !== "object") {
    throw new Error("OpenAPI contract must contain a paths object.");
  }
  return document;
}

function compileOperations(document) {
  const operations = [];
  const ids = new Set();
  for (const [path, item] of Object.entries(document.paths)) {
    if (!item || typeof item !== "object") continue;
    for (const [method, operation] of Object.entries(item)) {
      if (!HTTP_METHODS.has(method.toLowerCase()) || !operation || typeof operation !== "object") continue;
      const operationId = typeof operation.operationId === "string" ? operation.operationId.trim() : "";
      if (!operationId) continue;
      if (ids.has(operationId)) throw new Error(`OpenAPI operationId is duplicated: ${operationId}.`);
      ids.add(operationId);
      operations.push({
        operationId,
        method: method.toUpperCase(),
        path: pathPattern(path),
        specificity: pathSpecificity(path),
      });
    }
  }
  if (operations.length === 0) throw new Error("OpenAPI contract declares no named operations.");
  return operations.sort((left, right) => right.specificity - left.specificity);
}

function matchOperation(operations, method, url, backendUrl) {
  let pathname;
  try {
    const responseUrl = new URL(url);
    const backend = new URL(backendUrl);
    if (responseUrl.origin !== backend.origin) return undefined;

    const basePath = backend.pathname.replace(/\/$/, "");
    if (basePath && responseUrl.pathname !== basePath && !responseUrl.pathname.startsWith(`${basePath}/`)) {
      return undefined;
    }
    pathname = decodeURI(basePath ? responseUrl.pathname.slice(basePath.length) || "/" : responseUrl.pathname);
  } catch {
    return undefined;
  }
  return operations.find((operation) => operation.method === method.toUpperCase() && operation.path.test(pathname));
}

function pathPattern(path) {
  const escaped = path
    .split(/(\{[^}]+\})/g)
    .map((part) => part.startsWith("{") ? "[^/]+" : escapeRegex(part))
    .join("");
  return new RegExp(`^${escaped}/?$`);
}

function pathSpecificity(path) {
  return path
    .split("/")
    .filter(Boolean)
    .reduce((score, segment) => score + (/^\{[^}]+\}$/.test(segment) ? 0 : 1_000 + segment.length), 0);
}

function escapeRegex(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function configuredBackendUrl(env) {
  const value = env.PW_API_URL?.trim();
  if (!value) {
    throw new Error("PW_API_URL must name the real API used by backend-bound Playwright cases.");
  }
  try {
    return new URL(value).toString().replace(/\/$/, "");
  } catch {
    throw new Error(`PW_API_URL is not an absolute URL: ${value}`);
  }
}

function ensureTrailingSlash(value) {
  return value.endsWith("/") ? value : `${value}/`;
}
