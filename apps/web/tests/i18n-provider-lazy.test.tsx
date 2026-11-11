// 2026-11-11 第二轮瘦身回归：LocaleProvider 无 dict prop 时的挂起契约。
//
// 生产链路：layout 不再传整本字典（曾按 174KB/页序列化进 flight payload），
// 浏览器首帧 resolveDict 返回 null → Provider 抛 loadDict 的 promise →
// layout 的 Suspense 接住；SSR 侧由 dict-impl.server.ts 同步供给、零挂起。
// 本测试用 mock 实现模拟浏览器懒加载时序，锁住三件事：
//   1) 字典未就绪时渲染 fallback（不渲染 children——useI18n 会炸 null ctx）
//   2) promise 落定后 Provider 不再挂起（impl 需缓存，否则无限循环）
//   3) dict prop 逃生门直供，完全不触达供给侧
// 注意：vitest.setup 全局把 useI18n mock 成 zh-CN 整本（本 setup 的既定口径），
// 所以断言走「供给侧调用与否」而不是渲染文案的字典来源。
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { Suspense } from "react";
import { afterEach, expect, it, vi } from "vitest";
import { LocaleProvider } from "@/i18n/client";
import type { Dict } from "@/i18n/zh-CN";

// 受控假实现：带缓存的懒加载（与 dict-impl.client.ts 同语义——loadDict
// 落定后 resolveDict 必须能拿到，否则 Provider 会无限再挂起）
let loaded: Dict | null = null;
let resolveLoad: (d: Dict) => void = () => {};
const fakeDict = { common: {} } as unknown as Dict;

vi.mock("@/i18n/dict-impl", () => ({
  resolveDict: vi.fn(() => loaded),
  loadDict: vi.fn(
    () =>
      new Promise<Dict>((res) => {
        resolveLoad = (d) => {
          loaded = d;
          res(d);
        };
      }),
  ),
}));

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  loaded = null;
});

it("无 dict：先挂起（fallback），字典落定后渲染 children", async () => {
  render(
    <Suspense fallback={<div>loading</div>}>
      <LocaleProvider locale="zh-CN">
        <div>ready</div>
      </LocaleProvider>
    </Suspense>,
  );
  expect(screen.getByText("loading")).toBeTruthy();
  await waitFor(() => resolveLoad(fakeDict));
  await waitFor(() => {
    expect(screen.getByText("ready")).toBeTruthy();
  });
});

it("dict 逃生门：直供字典，供给侧零调用", async () => {
  render(
    <LocaleProvider dict={fakeDict} locale="zh-CN">
      <div>ready</div>
    </LocaleProvider>,
  );
  expect(screen.getByText("ready")).toBeTruthy();
  // 逃生门不应触发供给侧（mock 工厂里的 vi.fn 可查调用次数）
  const impl = await import("@/i18n/dict-impl");
  expect(impl.resolveDict).not.toHaveBeenCalled();
  expect(impl.loadDict).not.toHaveBeenCalled();
});
