import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen } from "@testing-library/react";

import JixiaoPage from "@/app/(main)/jixiao/_inner";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/** 绩效页两处「失败演成正常」：
 *  1. 岗位/记录取数失败时渲染「未被分配岗位」「暂无领取记录」——职员会
 *     误以为自己没被考核、也没领过钱，而不是网络出了问题；
 *  2. 紧凑档空态被 sr-only 藏掉，手机上「领取记录」只剩一块空白面板。
 */

const media = vi.hoisted(() => ({ compact: false }));
vi.mock("@/lib/hooks/use-media", () => ({
  useIsCompact: () => media.compact,
}));

function envelope(data: unknown) {
  return {
    ok: true,
    status: 200,
    headers: { get: () => "application/json" },
    json: async () => ({ code: 0, message: "ok", data, request_id: "t" }),
  } as unknown as Response;
}

const EMPTY_ME = {
  period: "2026-10",
  metrics: {},
  claim_window_days: 7,
  assigned: [],
};

function mount() {
  return render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      <JixiaoPage />
    </LocaleProvider>,
  );
}

beforeEach(() => {
  media.compact = false;
});
afterEach(() => vi.unstubAllGlobals());

describe("绩效页失败与空态", () => {
  it("取数失败：报加载失败，不报「未被分配岗位」", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => {
        throw new Error("network down");
      }),
    );
    mount();
    expect(await screen.findByText(zhCN.common.loadFailed)).toBeTruthy();
    expect(screen.queryByText(zhCN.jixiao2.noAssignment)).toBeNull();
    expect(screen.queryByText(zhCN.jixiao2.myEmpty)).toBeNull();
  });

  it("紧凑档真实空记录：空态文案可见（不被 sr-only 藏掉）", async () => {
    media.compact = true;
    vi.stubGlobal(
      "fetch",
      vi.fn(async (input: RequestInfo | URL) => {
        const url = String(input);
        return envelope(url.includes("/jixiao/my") ? [] : EMPTY_ME);
      }),
    );
    mount();
    const cell = await screen.findByText(zhCN.jixiao2.myEmpty);
    expect(cell.closest("table")?.className ?? "").not.toContain("sr-only");
  });
});
