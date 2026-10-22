import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";

import { CheaterBox } from "@/components/cheater-box";
import { ApiError } from "@/lib/api-client";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/**
 * P-615：GET /admin/cheaters 失败（非 2003）曾被落成空数组 ⇒
 * 「可疑会话」显示 0 条 + 本页为空，复核人员会以为没有可疑流量。
 * 现在失败显示加载失败；真实空列表仍是「本页为空」；2003 仍走无权限。
 *
 * 负控：回放 HEAD cheater-box ⇒ 第一条红在「找不到 加载失败」，
 * 快照里旧空态 t.empty 仍在；后两条正控两边都绿。
 */

const { get } = vi.hoisted(() => ({ get: vi.fn() }));

vi.mock("@/lib/api-client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/api-client")>();
  return { ...actual, api: { get, post: vi.fn() } };
});

const row = {
  user_id: 42,
  username: "buggy_user",
  torrent_id: null,
  name: null,
  upspeed: 0,
  uploaded_delta: 0,
  announced_at: "2026-10-20T00:00:00Z",
};

function stub(cheaters: () => Promise<unknown>) {
  get.mockImplementation(async (path: string) => {
    if (path.startsWith("/api/v1/admin/cheaters")) return cheaters();
    if (path.startsWith("/api/v1/admin/cheat-events")) return { items: [] };
    if (path.startsWith("/api/v1/admin/cheat-cases")) return { items: [] };
    if (path.startsWith("/api/v1/admin/risk-scores")) return { items: [] };
    if (path === "/api/v1/admin/cheat-appeals") return [];
    if (path.startsWith("/api/v1/admin/cheat-accuracy"))
      return { days: 90, families: [], sanctions: [] };
    throw new Error(`unexpected ${path}`);
  });
}

function mount() {
  render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      <CheaterBox />
    </LocaleProvider>,
  );
}

beforeEach(() => {
  get.mockReset();
});

describe("作弊者信箱取数失败（P-615）", () => {
  it("失败显示加载失败，不再显示本页为空", async () => {
    stub(() =>
      Promise.reject(new ApiError(5000, "服务暂时不可用", null, 500, "t")),
    );
    mount();
    expect(await screen.findByText(zhCN.common.loadFailed)).toBeTruthy();
    expect(screen.queryByText(zhCN.cheaterbox.empty)).toBeNull();
  });

  it("正控：真实空列表仍显示本页为空", async () => {
    stub(() => Promise.resolve([]));
    mount();
    expect(await screen.findByText(zhCN.cheaterbox.empty)).toBeTruthy();
    expect(screen.queryByText(zhCN.common.loadFailed)).toBeNull();
  });

  it("正控：成功时正常渲染可疑会话行", async () => {
    stub(() => Promise.resolve([row]));
    mount();
    expect(await screen.findByText("buggy_user")).toBeTruthy();
    expect(screen.queryByText(zhCN.cheaterbox.empty)).toBeNull();
  });
});
