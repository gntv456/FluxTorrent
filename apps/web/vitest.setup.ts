import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach, vi } from "vitest";

afterEach(() => cleanup());

// i18n 客户端依赖 LocaleProvider —— 测试里 mock 成 zh-CN 整本字典
const { zhCN } = await import("./i18n/zh-CN");
vi.mock("./i18n/client", async (importOriginal) => {
  const orig = await importOriginal<typeof import("./i18n/client")>();
  return {
    ...orig,
    useI18n: () => ({ dict: zhCN, locale: "zh-CN" }),
  };
});
