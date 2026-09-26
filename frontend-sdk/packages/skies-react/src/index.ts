// @skiesjs/react — the framework's frontend spine (router- and design-system-agnostic React primitives the app
// composes; the front-side parallel of the .NET packages). The canonical unit a Skies screen is built from:
// a ViewModel (data door) exposes its resource as AsyncState<T>; the View renders it through <Resource>. Auth/nav
// primitives (SessionState, safeBack) graduate here from app pilots as they stabilize.
export {
  type AsyncState,
  type QueryLike,
  toAsyncState,
  mapAsyncState,
  combineAsyncStates,
} from "./async-state";
export { Resource } from "./Resource";
export { type SessionState, type SessionQueryLike, toSessionState } from "./session";
export {
  type AuthTokens,
  type SessionSeam,
  type SessionSeamPorts,
  createSessionSeam,
} from "./session-seam";
export { useSession } from "./useSession";
export { singleFlight } from "./single-flight";
export { type GuardOutcome, type GuardOptions, guardSession } from "./guard";
export { type BackRouter, safeBack } from "./nav";
export { type HandleSubmitLike, type SubmitOrRevealOptions, submitOrReveal } from "./submit";
export { type RequiredParam, requiredParam } from "./params";
export { type ApiErrorCopyOptions, type I18nLike, apiErrorCode, apiErrorCopy } from "./api-error";
export { type Page, type PageInfo, toPageInfo } from "./page";
export { type Pager, type PagerOptions, type PagerPageControl, usePager } from "./usePager";
export {
  type AccumulatedPages,
  type AccumulatedPagesOptions,
  type Accumulation,
  useAccumulatedPages,
} from "./useAccumulatedPages";
