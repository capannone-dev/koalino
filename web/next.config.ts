// Defines the intentionally minimal Next.js configuration for the local dashboard. With
// KOALINO_WEB_STANDALONE=1 the build also writes a self-contained server (`.next/standalone`), which
// tools/cross/package-linux-server.sh ships in the server package (#125).

import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  reactStrictMode: true,
  output: process.env.KOALINO_WEB_STANDALONE === "1" ? "standalone" : undefined
};

export default nextConfig;
