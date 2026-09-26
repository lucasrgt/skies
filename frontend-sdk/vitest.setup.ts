// The framework's own test setup: only what every React test under jsdom needs. The sample's stand-in backend (its
// MSW server and handlers) belongs to the sample's specs, in examples/sample-app/.specs/web.setup.ts, so an app
// feature never edits a framework file to add a route.
(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
