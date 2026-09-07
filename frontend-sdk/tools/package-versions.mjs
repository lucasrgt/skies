// Published frontend packages that every Skies pilot with a frontend must consume together.
// This file ships inside @skiesjs/frontend-sdk, making the sync check independent of a sibling
// framework checkout and therefore effective in CI as well as on a developer machine.
export const FRONTEND_PACKAGE_VERSIONS = Object.freeze([
  Object.freeze({ name: "@skiesjs/frontend-sdk", version: "4.1.17" }),
  Object.freeze({ name: "assay-design", version: "0.7.3" }),
  Object.freeze({ name: "avp-assay", version: "0.4.0" }),
  Object.freeze({ name: "@skiesjs/react", version: "4.0.5" }),
  Object.freeze({ name: "@skiesjs/eslint-plugin", version: "4.0.5" }),
]);
