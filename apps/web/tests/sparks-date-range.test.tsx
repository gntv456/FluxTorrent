import { describe, it, expect, vi, afterEach } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";

import MySparksPage from "@/app/(main)/me/sparks/page";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/** 火花流水页的时间范围：「起」≤「止」是唯一合法形状。
 *  原实现给「止」输入写的是 max={from}（上限=起始日），
 *  浏览器会把起始日之后的每一天都判为越界——正常范围根本选不出来。 */

function stubApi() {
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: RequestInfo | URL) => {
      const url = String(input);
      const data = url.includes("/me/spark/ledger")
        ? { items: [], total: 0, page: 1, per: 50 }
        : { balance: 1234 };
      return {
        ok: true,
        status: 200,
        headers: { get: () => "application/json" },
        json: async () => ({ code: 0, message: "ok", data, request_id: "t" }),
      } as unknown as Response;
    }),
  );
}

function mount() {
  return render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      <MySparksPage />
    </LocaleProvider>,
  );
}

afterEach(() => vi.unstubAllGlobals());

describe("火花流水的时间范围控件", () => {
  it("「止」以「起」为下界，且不被起始日封顶", () => {
    stubApi();
    mount();
    const from = screen.getByLabelText(zhCN.ledger.dateFrom);
    fireEvent.change(from, { target: { value: "2026-10-01" } });
    const to = screen.getByLabelText(zhCN.ledger.dateTo);
    expect(to.getAttribute("min")).toBe("2026-10-01");
    expect(to.getAttribute("max")).toBeNull();
  });

  it("起始日之后的止日是可选的（不触发越界）", () => {
    stubApi();
    mount();
    fireEvent.change(screen.getByLabelText(zhCN.ledger.dateFrom), {
      target: { value: "2026-10-01" },
    });
    const to = screen.getByLabelText(
      zhCN.ledger.dateTo,
    ) as HTMLInputElement;
    fireEvent.change(to, { target: { value: "2026-10-20" } });
    expect(to.value).toBe("2026-10-20");
    expect(to.checkValidity()).toBe(true);
  });
});
