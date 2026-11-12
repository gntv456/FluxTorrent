import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import MySparkPage from "@/app/(main)/my-spark/page";
import { LocaleProvider } from "@/i18n/client";

/** 我的{magic}页标签化（2026-11-13）：统计条/CTA/排除提示常驻，正文
 *  三域（流水默认 / 收益计算 / 每日凭证）进标签组；挂载门卫——未访问
 *  的 tab 零渲染零请求（Merkle 挂载即拉 manifest）。 */

function envelope(data: unknown) {
  return {
    ok: true,
    status: 200,
    headers: { get: () => "application/json" },
    json: async () => ({ code: 0, message: "ok", data, request_id: "t" }),
  } as unknown as Response;
}

/** fetch 桩：记录 manifest 拉取（Merkle 挂载信号） */
function stubFetch() {
  const calls: string[] = [];
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: RequestInfo | URL) => {
      const url = String(input);
      calls.push(url);
      if (url.includes("/me/spark/ledger/manifest")) {
        return envelope({ built: false, date: "2026-11-12", rows: [] });
      }
      if (url.includes("/me/spark/ledger")) {
        return envelope({
          items: [
            {
              amount: 20,
              kind: "forum",
              balance_after: 1020,
              created_at: "2026-11-12T00:00:00Z",
              item_name: null,
            },
          ],
          total: 1,
          page: 1,
          per: 50,
        });
      }
      if (url.includes("/invites/status")) {
        return envelope({ redeem_price: 15000 });
      }
      // /me/spark（info）与 /me（PageHeader 等）统一回最小 info
      return envelope({
        balance: 1000,
        seeding_count: 6,
        earning_seeding_count: 4,
        hourly_estimate: 12,
      });
    }),
  );
  return calls;
}

afterEach(() => {
  vi.unstubAllGlobals();
  document.cookie = "flux.session=1; path=/";
});

describe("my-spark 标签化", () => {
  it("默认流水 tab：统计条常驻 + 流水表可见，calc/merkle 未挂载", async () => {
    stubFetch();
    document.cookie = "flux.session=1; path=/";
    render(
      <LocaleProvider locale="zh-CN">
        <MySparkPage />
      </LocaleProvider>,
    );
    // 流水行出现（默认 tab 激活）
    expect(await screen.findByText(/论坛/)).toBeTruthy();
    // tab 条三枚
    const tabs = screen.getAllByRole("tab");
    expect(tabs).toHaveLength(3);
    expect(tabs[0].getAttribute("aria-selected")).toBe("true");
    // 挂载门卫：Merkle 的 manifest 未拉（未访问该 tab）
    const manifestCalls = vi
      .mocked(fetch)
      .mock.calls.map((c) => String(c[0]))
      .filter((u) => u.includes("manifest"));
    expect(manifestCalls).toHaveLength(0);
  });

  it("切到每日凭证：Merkle 挂载并拉 manifest；切回流水不重拉", async () => {
    const calls = stubFetch();
    document.cookie = "flux.session=1; path=/";
    render(
      <LocaleProvider locale="zh-CN">
        <MySparkPage />
      </LocaleProvider>,
    );
    await screen.findByText(/论坛/);
    await userEvent.click(
      screen.getByRole("tab", { name: "每日结算凭证" }),
    );
    // manifest 恰拉一次（挂载即拉）
    await screen.findByText(/sha256-merkle/);
    expect(calls.filter((u) => u.includes("manifest"))).toHaveLength(1);
    // 切回流水：Merkle 面板隐藏但保持挂载（不重拉）
    await userEvent.click(screen.getByRole("tab", { name: /流水/ }));
    expect(calls.filter((u) => u.includes("manifest"))).toHaveLength(1);
    // 流水表仍可见
    expect(screen.getByText(/论坛/)).toBeTruthy();
  });
});
