import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen } from "@testing-library/react";

import { BigsmallRoadPage } from "@/components/game/bigsmall-road";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/** 猜大小路单：取数失败不能演成「还没有对局」+ 全零走势统计。 */

afterEach(() => vi.unstubAllGlobals());

function envelope(data: unknown) {
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
      <BigsmallRoadPage />
    </LocaleProvider>,
  );
}

describe("猜大小路单失败不演空", () => {
  it("取数失败：报加载失败，不显示空态与零统计", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => {
        throw new Error("network down");
      }),
    );
    mount();
    expect(await screen.findByText(zhCN.common.loadFailed)).toBeTruthy();
    expect(screen.queryByText(zhCN.games.sub.empty)).toBeNull();
    expect(screen.queryByText(zhCN.games.sub.statPlays)).toBeNull();
  });

  it("真实空列表：仍报「还没有对局」", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => envelope([])),
    );
    mount();
    expect(await screen.findByText(zhCN.games.sub.empty)).toBeTruthy();
    expect(screen.queryByText(zhCN.common.loadFailed)).toBeNull();
  });
});
