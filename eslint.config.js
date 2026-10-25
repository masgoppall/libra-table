// Lint rules: typed TypeScript, React hooks, and a hard 100-column limit.

import js from "@eslint/js";
import reactHooks from "eslint-plugin-react-hooks";
import globals from "globals";
import tseslint from "typescript-eslint";

export default tseslint.config(
  { ignores: ["dist", "src-tauri", "node_modules"] },
  {
    files: ["src/**/*.{ts,tsx}"],
    extends: [js.configs.recommended, ...tseslint.configs.recommended],
    languageOptions: { globals: globals.browser },
    plugins: { "react-hooks": reactHooks },
    rules: {
      ...reactHooks.configs.recommended.rules,
      // Effects here deliberately run on a subset of their inputs (load on key change).
      "react-hooks/exhaustive-deps": "off",
      "@typescript-eslint/no-explicit-any": "error",
      "@typescript-eslint/consistent-type-imports": ["error", { fixStyle: "inline-type-imports" }],
      "@typescript-eslint/no-unused-vars": [
        "error",
        { argsIgnorePattern: "^_", varsIgnorePattern: "^_" },
      ],
      "max-len": ["error", { code: 100, ignoreUrls: true, ignoreRegExpLiterals: true }],
      "max-lines": ["warn", { max: 300, skipBlankLines: true, skipComments: true }],
      "no-console": "warn",
      eqeqeq: ["error", "smart"],
    },
  },
  {
    files: ["src/**/*.test.{ts,tsx}", "src/test/**"],
    rules: { "max-lines": "off" },
  },
);
