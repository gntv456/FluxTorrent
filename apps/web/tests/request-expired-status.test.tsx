import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { zhCN } from "@/i18n/zh-CN";
import { LocaleProvider } from "@/i18n/client";
import { api } from "@/lib/api-client";
import { RequestBoard } from "@/components/request-board";
import { RequestCardList } from "@/components/request-card-list";

/** P-631：`finished=ing` 映射的是 requests.status=2，而这一态只由 worker
 *  过期回收写入；旧界面把这一档叫「解决中」，过期单还带应种按钮，点下去
 *  后端回「该求种已处理」。 */

const rows = [
  {
    id: 1,
    username: "alice",
    title: "已过期的求种",
    descr: null,
    bounty: 50,
    latest_bounty: null,
    comments: null,
    bids: null,
    status: 2,
    fulfilled_torrent_id: null,
    created_at: "2026-09-01T00:00:00Z",
  },
  {
    id: 2,
    username: "bob",
    title: "还开放的求种",
    descr: null,
    bounty: 30,
    latest_bounty: null,
    comments: null,
    bids: null,
    status: 0,
    fulfilled_torrent_id: null,
    created_at: "2026-10-01T00:00:00Z",
  },
  {
    id: 3,
    username: "carol",
    title: "已解决的求种",
    descr: null,
    bounty: 10,
    latest_bounty: null,
    comments: null,
    bids: null,
    status: 1,
    fulfilled_torrent_id: 99,
    created_at: "2026-10-02T00:00:00Z",
  },
];

function wrap(ui: React.ReactNode) {
  return render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      {ui}
    </LocaleProvider>,
  );
}

beforeEach(() => {
  vi.restoreAllMocks();
});

describe("求种过期状态渲染", () => {
  it("桌面表：过期单显示已过期且不给应种按钮", async () => {
    vi.spyOn(api, "get").mockResolvedValue(rows);
    wrap(<RequestBoard initialFinished="all" initialSearch="" />);
    expect(await screen.findByText("已过期的求种")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: zhCN.requests.filterIng }),
    ).toBeInTheDocument();
    expect(screen.getByText(zhCN.invites.expired)).toBeInTheDocument();
    // 旧实现这里会是 2 个（过期单 + 开放单）；开放单仍须保留入口
    expect(
      screen.getAllByRole("button", {
        name: zhCN.requests.fulfillBtn,
      }),
    ).toHaveLength(1);
    expect(screen.getAllByText(zhCN.requests.pending)).toHaveLength(1);
  });

  it("窄屏卡片：过期单显示已过期且不给应种按钮", () => {
    wrap(<RequestCardList rows={rows} onChanged={vi.fn()} />);
    expect(screen.getByText(zhCN.invites.expired)).toBeInTheDocument();
    expect(
      screen.getAllByRole("button", {
        name: zhCN.requests.fulfillBtn,
      }),
    ).toHaveLength(1);
    expect(screen.getAllByText(zhCN.requests.pending)).toHaveLength(1);
  });
});
