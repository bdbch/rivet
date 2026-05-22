import { defineConfig } from "vite-plus";

export default defineConfig({
  lint: {"jsPlugins":[{"name":"vite-plus","specifier":"vite-plus/oxlint-plugin"}],"rules":{"vite-plus/prefer-vite-plus-imports":"error"},"options":{"typeAware":true,"typeCheck":true}},
  staged: {
    "*.@(js|ts|tsx)": [
      "vp lint --fix"
    ],
    "*.@(js|ts|tsx|yml|yaml|md|json)": [
      "vp fmt"
    ],
    "*.toml": [
      "taplo format"
    ]
  },
  fmt: {
    arrowParens: "always",
    printWidth: 120,
    semi: false,
    singleQuote: true,
    trailingComma: "all",
    sortPackageJson: false,
    ignorePatterns: [
      "target",
      ".yarn",
      "index.js",
      "package-template.wasi-browser.js",
      "package-template.wasi.cjs",
      "wasi-worker-browser.mjs",
      "wasi-worker.mjs",
      ".yarnrc.yml",
    ],
  },
});
