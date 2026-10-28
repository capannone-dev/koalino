// Configures Next.js and TypeScript linting for the dashboard source tree.

import { FlatCompat } from "@eslint/eslintrc";
import { dirname } from "node:path";
import { fileURLToPath } from "node:url";

const currentDirectory = dirname(fileURLToPath(import.meta.url));
const compatibility = new FlatCompat({ baseDirectory: currentDirectory });

const eslintConfig = [
  // Generated output: without this, `npm run lint` after `npm run build` reports the compiled bundle.
  { ignores: [".next/**", "node_modules/**", "next-env.d.ts"] },
  ...compatibility.extends("next/core-web-vitals", "next/typescript")
];

export default eslintConfig;
