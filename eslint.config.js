// @ts-check
import js from "@eslint/js";
import tseslint from "typescript-eslint";
import react from "eslint-plugin-react";
import reactHooks from "eslint-plugin-react-hooks";
import globals from "globals";
import prettier from "eslint-config-prettier";

export default tseslint.config(
  {
    ignores: [
      "**/dist/**",
      "**/target/**",
      "**/node_modules/**",
      "**/test-results/**",
      "**/playwright-report/**",
      "apps/desktop/src-tauri/runtime/**",
      // Generated from Rust; linting is the generator's responsibility.
      "apps/desktop/src/lib/bindings.ts",
    ],
  },
  js.configs.recommended,

  // Type-checked, React-aware rules for application source only.
  {
    files: ["apps/desktop/src/**/*.{ts,tsx}"],
    extends: [...tseslint.configs.strictTypeChecked],
    languageOptions: {
      parserOptions: {
        projectService: true,
        tsconfigRootDir: import.meta.dirname,
      },
      globals: { ...globals.browser, ...globals.es2022 },
    },
    plugins: {
      react,
      "react-hooks": reactHooks,
    },
    settings: { react: { version: "detect" } },
    rules: {
      ...react.configs.recommended.rules,
      ...reactHooks.configs.recommended.rules,
      "react/react-in-jsx-scope": "off",
      // Engineering rule: no `any` unless explicitly documented via eslint-disable.
      "@typescript-eslint/no-explicit-any": "error",
    },
  },

  // Untyped lint for workspace-package stubs and TS config files (not in a tsconfig).
  {
    files: ["packages/**/*.{ts,tsx}", "apps/desktop/e2e/**/*.ts", "**/*.config.{ts,mts,cts}"],
    extends: [...tseslint.configs.recommended],
    languageOptions: {
      globals: { ...globals.node },
    },
  },

  // Node scripts and JS config files.
  {
    files: ["scripts/**/*.{js,mjs,cjs}", "**/*.{js,mjs,cjs}"],
    languageOptions: {
      sourceType: "module",
      globals: { ...globals.node },
    },
  },

  prettier,
);
