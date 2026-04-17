const tseslintPlugin = require("@typescript-eslint/eslint-plugin");
const tsParser = require("@typescript-eslint/parser");

module.exports = [
  {
    ignores: ["out/**", "dist/**", "**/*.d.ts"]
  },
  {
    files: ["src/**/*.ts"],
    languageOptions: {
      parser: tsParser,
      parserOptions: {
        ecmaVersion: 2015,
        sourceType: "module"
      }
    },
    plugins: {
      "@typescript-eslint": tseslintPlugin
    },
    rules: {
      ...tseslintPlugin.configs.recommended.rules,
      "@typescript-eslint/no-unused-vars": ["error", {
        "caughtErrors": "none",
        "argsIgnorePattern": "^_",
        "varsIgnorePattern": "^_"
      }],
      "@typescript-eslint/naming-convention": [
        "warn",
        {
          "selector": "import",
          "format": ["camelCase", "PascalCase"]
        }
      ],
      "curly": "warn",
      "eqeqeq": "warn",
      "no-throw-literal": "warn",
      "semi": "off",
      "no-useless-escape": "off"
    }
  }
];
