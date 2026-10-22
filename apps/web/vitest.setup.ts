import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach, vi } from "vitest";

afterEach(() => cleanup());

// i18n 客户端依赖 LocaleProvider —— 测试里 mock 成 zh-CN 整本字典
//
// 注意 currency 这一项：这里的 `useI18n` 是**整体替换**，Provider 收到的
// props 一律无效。少给 currency 时，凡读 `{magic}`（站点货币名占位）的文案
// 会在测试里渲染成「…undefined」，而组件本身完全没错（P-558 那批就是这样：
// 界面已修好、红账却指向组件）。所以按生产端 layout.tsx 的真实默认值补上。
const { zhCN } = await import("./i18n/zh-CN");
vi.mock("./i18n/client", async (importOriginal) => {
  const orig = await importOriginal<typeof import("./i18n/client")>();
  return {
    ...orig,
    useI18n: () => ({ dict: zhCN, locale: "zh-CN", currency: "魔力" }),
  };
});
