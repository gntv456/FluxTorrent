import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";

import MySparkPage from "@/app/(main)/my-spark/page";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/** 「近期收入/近期支出」必须锚在最近 50 笔上，不能随翻页漂移。
 *
 *  旧实现从**当前页** rows 归约：翻到第 2 页，这两个 KPI 和第 N 页
 *  表头合计一起变成第 51–100 笔的合计——同一组标签在两页显示两套
 *  「近期」数字，用户无法据此判断最近到底收支多少。
 */

const row = (amount: number, at: string) => ({
  amount,
  kind: "forum",
  balance_after: 1000,
  created_at: at,
  item_name: null,
});

function envelope(data: unknown) {
  return {
    ok: true,
    status: 200,
    headers: { get: () => "application/json" },
    json: async () => ({ code: 0, message: "ok", data, request_id: "t" }),
  } as unknown as Response;
}

function stub() {
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: RequestInfo | URL) => {
      const url = String(input);
      if (url.includes("/me/spark/ledger/manifest")) {
        return envelope({ date: "2026-10-22", root: "ab", count: 0 });
      }
      if (url.includes("/me/spark/ledger")) {
        const p2 = url.includes("page=2");
        return envelope({
          items: [
            p2
              ? row(-7, "2026-09-01T00:00:00Z")
              : row(100, "2026-10-22T00:00:00Z"),
          ],
          total: 60,
          page: p2 ? 2 : 1,
          per: 50,
        });
      }
      if (url.includes("/invites/status")) {
        return envelope({ redeem_price: 15000 });
      }
      return envelope({ balance: 1000, seeding_count: 3, hourly_estimate: 5 });
    }),
  );
}

const kpi = (i: number) =>
  Array.from(document.querySelectorAll(".pgstats .v"))[i]?.textContent?.trim();

beforeEach(() => {
  document.cookie = "flux.session=test-token";
  stub();
});
afterEach(() => {
  vi.unstubAllGlobals();
  document.cookie =
    "flux.session=; expires=Thu, 01 Jan 1970 00:00:00 GMT";
});

function mount() {
  return render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      <MySparkPage />
    </LocaleProvider>,
  );
}

describe("近期收支固定锚在最近一页", () => {
  it("翻到第 2 页：KPI 仍是第 1 页合计，表格换成第 2 页行", async () => {
    mount();
    // 表头和流水行都带 +100，用 All 版避免命中歧义
    expect((await screen.findAllByText("+100")).length).toBeGreaterThan(0);
    expect(kpi(1)).toBe("100");
    expect(kpi(2)).toBe("0");

    fireEvent.click(
      screen.getByRole("button", { name: zhCN.common.nextPage }),
    );
    expect((await screen.findAllByText("-7")).length).toBeGreaterThan(0);
    expect(kpi(1)).toBe("100");
    expect(kpi(2)).toBe("0");
  });
});
