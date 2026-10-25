// Configures Next.js and TypeScript linting for the dashboard source tree.

import { FlatCompat } from "@eslint/eslintrc";
import { dirname } from "node:path";
import { fileURLToPath } from "node:url";

const currentDirectory = dirname(fileURLToPath(import.meta.url));
const compatibility = new FlatCompat({ baseDirectory: currentDirectory });

export default [...compatibility.extends("next/core-web-vitals", "next/typescript")];
