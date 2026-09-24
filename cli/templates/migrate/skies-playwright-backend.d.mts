/** Environment contract shared by the Playwright global setup and test workers. */
export interface BackendEnvironment {
  PW_API_URL?: string;
  PW_API_READY?: string;
  [name: string]: string | undefined;
}

/** The small fetch surface needed by the backend preflight. */
export type BackendProbeFetch = (
  url: string,
  init: { method: "GET"; redirect: "manual" },
) => Promise<{ ok: boolean; status: number }>;

/** Configuration for the canonical real-backend preflight. */
export interface BackendProbeOptions {
  env?: BackendEnvironment;
  fetchImpl?: BackendProbeFetch;
  path?: string;
  timeoutMs?: number;
  intervalMs?: number;
}

/** The Playwright response surface used to collect contract evidence without depending on Playwright at runtime. */
export interface BackendPage {
  on(event: "response", listener: (response: {
    request(): { method(): string };
    status(): number;
    url(): string;
  }) => void): void;
}

/** Minimal OpenAPI shape used to resolve browser responses to slice operation names. */
export interface BackendOpenApiDocument {
  paths: Record<string, Record<string, { operationId?: string } | unknown>>;
}

/** Opaque evidence collected from one real browser page. */
export interface BackendObservation {
  readonly __backendObservation?: never;
}

/** Poll the configured API and mark it ready only after a successful HTTP response. */
export function probeBackend(options?: BackendProbeOptions): Promise<string>;

/** Build the Playwright globalSetup function for the configured real API. */
export function createBackendGlobalSetup(options?: BackendProbeOptions): () => Promise<void>;

/** Start collecting real page responses and resolve them through the checked-in OpenAPI contract. */
export function observeBackend(
  page: BackendPage,
  contract: string | URL | BackendOpenApiDocument,
  env?: BackendEnvironment,
): Promise<BackendObservation>;

/** Assert that every named slice produced the expected success or error response in the visible journey. */
export function expectBackendSlices(
  observation: BackendObservation,
  slices: readonly string[],
  options: { status: "success" | "error" },
): void;

/** Wait for parallel browser requests to produce the expected success or error responses. */
export function waitForBackendSlices(
  observation: BackendObservation,
  slices: readonly string[],
  options: { status: "success" | "error"; timeoutMs?: number; intervalMs?: number },
): Promise<void>;
