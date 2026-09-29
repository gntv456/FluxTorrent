import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import path from "node:path";

export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    setupFiles: ["./vitest.setup.ts"],
    include: ["tests/**/*.test.{ts,tsx}"],
    globals: true,
    // E14 覆盖率基线：只统计共享层（组件/i18n/lib——纯函数与叶子组件）。
    // app/ 页面与 RSC 由 e2e 脚本+Playwright 覆盖，不进单测覆盖率口径。
    coverage: {
      provider: "v8",
      reporter: ["text", "html", "lcov-only"],
      reportsDirectory: "coverage",
      // 口径：被测触达的共享层文件（不带 all——未被 import 的文件不按 0 稀释）
      include: [
        "components/**/*.{ts,tsx}",
        "i18n/**/*.{ts,tsx}",
        "lib/**/*.{ts,tsx}",
      ],
    },
  },
  resolve: {
    alias: { "@": path.resolve(__dirname, "./") },
  },
});
