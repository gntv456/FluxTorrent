import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen } from "@testing-library/react";

import OffersPage from "@/app/(main)/offers/_inner";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/** 候选列表取数失败必须报错，不能演成「暂无候选」。
 *
 *  旧实现 .catch(() => setRows([])) 把网络/鉴权失败与「没人求种」压成同一
 *  画面：会员在候选页看到的是一张空表，会以为功能没人用，也没有重试口。
 *  四态里失败必须走 ErrorPanel（error 优先于 empty 是 StatePanel 的约定）。
 */

function stub(fetchImpl: () => Promise<unknown>) {
  vi.stubGlobal(
    "fetch",
    vi.fn(async () => fetchImpl() as Promise<Response>),
  );
}

function okResponse(data: unknown) {
  return {
    ok: true,
    status: 200,
    headers: { get: () => "application/json" },
    json: async () => ({ code: 0, message: "ok", data, request_id: "t" }),
  } as unknown as Response;
}

function mount() {
  return render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      <OffersPage />
    </LocaleProvider>,
  );
}

afterEach(() => vi.unstubAllGlobals());

describe("候选列表失败不演成空态", () => {
  it("请求失败：报加载失败并给重试，不显示「暂无候选」", async () => {
    stub(async () => {
      throw new Error("network down");
    });
    mount();
    expect(await screen.findByText(zhCN.ui.errorTitle)).toBeTruthy();
    expect(screen.queryByText(zhCN.offers2.empty)).toBeNull();
    expect(screen.getByText(zhCN.common.retry)).toBeTruthy();
  });

  it("真实空结果：仍显示「暂无候选」，不误报错", async () => {
    stub(async () => okResponse([]));
    mount();
    expect(await screen.findByText(zhCN.offers2.empty)).toBeTruthy();
    expect(screen.queryByText(zhCN.ui.errorTitle)).toBeNull();
  });
});
