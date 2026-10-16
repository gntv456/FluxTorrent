import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";

import { TorrentListClient } from "@/components/torrent-list-client";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/**
 * 列表被截断时必须自己说出口（批六 D1 之一）。
 *
 * `/me/torrentlist` 后端按 limit=100 截断且没有 cursor。取满 100 条时，
 * 页面看着就像「我只有这些做种记录」——上传过几百种的老用户据此以为自己
 * 丢了一堆种子。这里钉住：取满才提示、没取满不许提示（否则每个不满一页的
 * 用户都看到一句无意义的话）。
 */

vi.mock("@/lib/hooks/use-media", () => ({ useIsCompact: () => false }));

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

function stubRows(data: unknown) {
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

function mount() {
  return render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      <TorrentListClient kind="seeding" emptyText="暂无" />
    </LocaleProvider>,
  );
}

afterEach(() => vi.unstubAllGlobals());

describe("截断提示", () => {
  it("取满 100 条：说明只到这儿", async () => {
    stubRows(rows(100));
    mount();
    await waitFor(() =>
      expect(document.body.textContent).toContain(
        zhCN.mytl.truncated.replace("{n}", "100"),
      ),
    );
  });

  it("没取满：不许出现这句", async () => {
    stubRows(rows(3));
    mount();
    await waitFor(() =>
      expect(document.body.textContent).toContain("种 0"),
    );
    expect(document.body.textContent).not.toContain("仅显示最近");
  });
});
