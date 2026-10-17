import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";

import { TorrentListClient } from "@/components/torrent-list-client";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/**
 * 「我的种子」列表的三态与翻页（批六 D1 的延续）。
 *
 * 2026-10-17：后端从「裸数组 + limit=100 截断」改成带 total 的分页信封，
 * 于是「取满要说清后面还有」这条判据换了形态——现在它等价于
 * 「还有下一页才提示，最后一页不许提示」。同时补上失败档：
 * 取档失败必须说「没取到」，不能演成「你还没有做种记录」。
 */

vi.mock("@/lib/hooks/use-media", () => ({ useIsCompact: () => false }));

const PER = 50;

function rows(n: number) {
  return Array.from({ length: n }, (_, i) => ({
    torrent_id: 1000 + i,
    name: "种 " + i,
    size: 1024,
    seeders: 1,
    leechers: 0,
    seeding: true,
    leeching: false,
    completed_at: null,
    done: 0,
  }));
}

function envelope(n: number, total: number, page = 1) {
  return { rows: rows(n), total, page, per: PER };
}

function stub(data: unknown) {
  vi.stubGlobal(
    "fetch",
    vi.fn(async () =>
      ({
        ok: true,
        status: 200,
        headers: { get: () => "application/json" },
        json: async () => ({ code: 0, message: "ok", data, request_id: "t" }),
      }) as unknown as Response,
    ),
  );
}

function stubFailing() {
  vi.stubGlobal(
    "fetch",
    vi.fn(async () =>
      ({
        ok: false,
        status: 500,
        headers: { get: () => "application/json" },
        json: async () => ({ code: 1000, message: "boom", data: null }),
      }) as unknown as Response,
    ),
  );
}

function mount() {
  return render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      <TorrentListClient kind="seeding" emptyText="暂无" />
    </LocaleProvider>,
  );
}

afterEach(() => vi.unstubAllGlobals());

describe("翻页与截断提示", () => {
  it("还有下一页：说清只到第几页，并画出翻页条", async () => {
    stub(envelope(PER, PER * 3));
    mount();
    await waitFor(() =>
      expect(document.body.textContent).toContain(
        zhCN.mytl.truncated.replace("{n}", String(PER)),
      ),
    );
    expect(screen.getByText(zhCN.common.nextPage)).toBeInTheDocument();
  });

  it("最后一页：不许出现那句提示", async () => {
    stub(envelope(3, PER * 2 + 3, 3));
    mount();
    await waitFor(() => expect(document.body.textContent).toContain("种 0"));
    expect(document.body.textContent).not.toContain("仅显示最近");
  });

  it("不满一页：既不提示截断，也不画翻页条", async () => {
    stub(envelope(3, 3));
    mount();
    await waitFor(() => expect(document.body.textContent).toContain("种 0"));
    expect(document.body.textContent).not.toContain("仅显示最近");
    expect(screen.queryByText(zhCN.common.nextPage)).toBeNull();
  });

  it("取档失败：说「没取到」，不演成「暂无记录」", async () => {
    stubFailing();
    mount();
    await waitFor(() =>
      expect(document.body.textContent).toContain(zhCN.readState.failTitle),
    );
    expect(document.body.textContent).not.toContain("暂无");
  });
});
