import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen } from "@testing-library/react";

import { SeedStatsCard } from "@/components/seed-stats-card";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/** 保种统计卡：只有 403/2003 才是「没权限」，
 *  网络故障不能演成权限说明。 */

afterEach(() => vi.unstubAllGlobals());

function envelope(data: unknown, status = 200, code = 0) {
  const message = code === 0 ? "ok" : "无权限";
  return {
    ok: status < 400,
    status,
    headers: { get: () => "application/json" },
    json: async () => ({ code, message, data, request_id: "t" }),
  } as unknown as Response;
}

function mount() {
  return render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      <SeedStatsCard />
    </LocaleProvider>,
  );
}

const deniedCopy = /需「保种统计」权限/;

describe("保种统计卡失败不演权限不足", () => {
  it("网络失败：报加载失败，不报权限说明", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => {
        throw new Error("network down");
      }),
    );
    mount();
    expect(await screen.findByText(zhCN.common.loadFailed)).toBeTruthy();
    expect(screen.queryByText(deniedCopy)).toBeNull();
  });

  it("403/2003：仍显示权限说明", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => envelope(null, 403, 2003)),
    );
    mount();
    expect(await screen.findByText(deniedCopy)).toBeTruthy();
    expect(screen.queryByText(zhCN.common.loadFailed)).toBeNull();
  });

  it("成功：显示做种总览", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () =>
        envelope({
          seeders: 3,
          seeding_torrents: 9,
          avg_seed_hours: 12,
          top_by_count: [],
          top_by_hours: [],
        }),
      ),
    );
    mount();
    expect(await screen.findByText("做种用户")).toBeTruthy();
    expect(screen.queryByText(zhCN.common.loadFailed)).toBeNull();
  });
});
