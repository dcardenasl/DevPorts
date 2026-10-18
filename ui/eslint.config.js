import js from "@eslint/js";
import reactHooks from "eslint-plugin-react-hooks";
import globals from "globals";
import tseslint from "typescript-eslint";

export default tseslint.config(
  { ignores: ["dist", "node_modules", "src/bindings.ts"] },
  js.configs.recommended,
  ...tseslint.configs.strictTypeChecked,
  {
    languageOptions: {
      globals: globals.browser,
      parserOptions: { projectService: true, tsconfigRootDir: import.meta.dirname },
    },
    plugins: { "react-hooks": reactHooks },
    rules: {
      ...reactHooks.configs.recommended.rules,
      "@typescript-eslint/no-explicit-any": "error",
      "@typescript-eslint/no-non-null-assertion": "error",
      "@typescript-eslint/restrict-template-expressions": ["error", { allowNumber: true }],
    },
  },
  {
    // Everything the person reads lives in the catalogs (ADR-041): no literal text in JSX
    // and none in the attributes read out by assistive technology.
    files: ["src/**/*.tsx"],
    ignores: ["src/**/*.test.tsx"],
    rules: {
      "no-restricted-syntax": [
        "error",
        {
          selector: "JSXText[value=/\\p{L}/u]",
          message: "Interface text belongs in src/i18n (ADR-041).",
        },
        {
          selector:
            "JSXAttribute[name.name=/^(aria-label|aria-description|aria-roledescription|aria-placeholder|aria-valuetext|title|placeholder|alt)$/] > Literal",
          message: "Interface text belongs in src/i18n (ADR-041).",
        },
      ],
    },
  },
  {
    files: ["*.config.js", "*.config.ts", "vitest.setup.ts"],
    ...tseslint.configs.disableTypeChecked,
  },
);
