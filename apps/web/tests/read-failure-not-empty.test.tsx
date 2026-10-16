import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";

import { TeamPanel } from "@/components/team-panel";
import { NoticePrefsCard } from "@/components/notice-prefs";
import { RiskWatchlist } from "@/components/risk-watchlist";
import { CheatEventsTable } from "@/components/cheat-events-table";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/**
 * 读失败不得演成假状态（批四 B1）。
 *
 * 这批面板原先把 GET 失败写成「拿到了一份空数据」：战队说「本站未开放」、
 * 通知偏好说「你九项全开着」（用户据此一改就写回错误基线）、风控名单与
 * 作弊事件表显示「暂无」、用户详情整块消失。接口挂了和用户本来就没数据
 * 是两件事，界面必须分开说。
 *
 * 负控：把 catch 换回旧的假数据回落，本文件四条断言全红。
 */

function failOnce(message: string) {
  vi.stubGlobal(
    "fetch",
    vi.fn(async () =>
      ({
        ok: true,
        status: 200,
        headers: { get: () => "application/json" },
        json: async () => ({
          code: 9999,
          message,
          data: null,
          request_id: "t",
        }),
      }) as unknown as Response,
    ),
  );
}

function mount(node: React.ReactNode) {
  return render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      {node}
    </LocaleProvider>,
  );
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("读失败要报失败", () => {
  it("战队面板：不演「本站未开放」", async () => {
    failOnce("战队列表读取失败");
    mount(<TeamPanel />);
    // 战队面板是 300 行门禁的存量件，这里复用它已有的提示槽说失败，
    // 不再多开一个错误态——断言也跟着取那句原文。
    await waitFor(() =>
      expect(document.body.textContent).toContain("战队列表读取失败"),
    );
    expect(document.body.textContent).not.toContain(zhCN.teams.disabled);
  });

  it("通知偏好：不演「九项全开着」，开关一律不给点", async () => {
    failOnce("偏好读取失败");
    mount(<NoticePrefsCard />);
    await waitFor(() => expect(screen.getByRole("alert")).toBeTruthy());
    expect(screen.queryAllByRole("switch")).toHaveLength(0);
  });

  it("风控观察名单：不演「暂无」", async () => {
    failOnce("风控分读取失败");
    mount(<RiskWatchlist />);
    await waitFor(() => expect(screen.getByRole("alert")).toBeTruthy());
    expect(document.body.textContent).not.toContain(
      zhCN.cheaterbox.riskEmpty,
    );
  });

  it("作弊事件表：不演「暂无」", async () => {
    failOnce("作弊事件读取失败");
    mount(<CheatEventsTable />);
    await waitFor(() => expect(screen.getByRole("alert")).toBeTruthy());
    expect(document.body.textContent).not.toContain(
      zhCN.cheaterbox.eventsEmpty,
    );
  });
});
