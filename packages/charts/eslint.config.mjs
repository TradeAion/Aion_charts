// Flat ESLint config for the @aion/charts façade.
// Style/formatting is left to the TypeScript compiler + editor; lint focuses on correctness.
import js from "@eslint/js";
import tseslint from "typescript-eslint";

export default tseslint.config(
  { ignores: ["dist/", "pkg/", "node_modules/"] },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    rules: {
      // The public API is deliberately snake_case (project-wide convention).
      "@typescript-eslint/naming-convention": "off",
      "@typescript-eslint/no-explicit-any": "error",
      // The wasm pkg import is a build artifact: the @ts-ignore on it is load-bearing before
      // `build:wasm` has run, and would be an unused @ts-expect-error after it.
      "@typescript-eslint/ban-ts-comment": ["error", { "ts-ignore": "allow-with-description" }],
      "@typescript-eslint/no-unused-vars": ["error", { argsIgnorePattern: "^_" }],
    },
  },
);
