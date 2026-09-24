import tseslint from "typescript-eslint";
import skies from "@skiesjs/eslint-plugin";

// The SKYFE architecture rules and the jsx-a11y floor, as `@skiesjs/eslint-plugin` recommends them. `skies doctor`
// runs this through the package's `lint` script. Add your own rules in a later config object.
export default [
  { ignores: ["dist/", "src/client.gen/"] },
  {
    files: ["**/*.{ts,tsx}"],
    languageOptions: {
      parser: tseslint.parser,
      parserOptions: { ecmaFeatures: { jsx: true } },
    },
  },
  { ...skies.configs.recommended, files: ["**/*.{ts,tsx}"] },
];
