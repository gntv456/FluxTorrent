/** 签到页规则卡渲染（2026-11-10）：/attendance 透出的 rules 必须落成可见文案——
 *  里程碑行按日排序、下一档高亮、封顶与断签说明齐全。线上报障起因就是
 *  「连签 27 天每天 8 魔力，用户不知道 10/20/30 天有加送」。 */
import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";

// api.get 按调用路径返回：/attendance 带规则数据
vi.mock("@/lib/api-client", () => ({
  api: {
    get: vi.fn(async (url: string) => {
      if (url.includes("/attendance/calendar")) {
        return {
          ym: "2026-11",
          days: Array.from({ length: 30 }, (_, i) => ({
            date: `2026-11-${String(i + 1).padStart(2, "0")}`,
            done: i < 10,
            reward: 8,
          })),
        };
      }
      return {
        checked_today: true,
        streak: 27,
        recent: [],
        makeup_cards: 0,
        rules: {
          first: 15,
          streak: 8,
          daily_cap: 1000,
          milestones: [
            [10, 200],
            [20, 500],
            [30, 1000],
          ],
          cycle_days: 30,
          cycle_pos: 27,
          next_milestone_in_cycle: 30,
        },
      };
    }),
    post: vi.fn(),
  },
}));
vi.mock("@/i18n/client", () => ({
  useI18n: () => ({
    dict: {
      my: { checkinOk: "+{reward}", checkinBusy: "…" },
      home2: {
        streak: "连续签到",
        totalDays: "累计签到",
        attended: "今日已签到",
        checkinNow: "立即签到",
        checkinFail: "失败",
        rulesTitle: "签到规则",
        rulesFirst: "首次签到 +{n} 魔力",
        rulesDaily: "每日签到 +{n} 魔力",
        rulesMilestone: "连签 {day} 天当日加送 +{bonus}（当日合计 {total}）",
        rulesCap: "单日封顶 {n} 魔力",
        rulesBreak: "中断后连签重新计算",
        rulesNextBadge: "下一档 {day} 天",
        rulesCycle:
          "周期制：连签满 {cycle} 天后次日从第 1 天重新计算（当前周期第 {pos} 天）",
        yearMonth: "{y}年{m}月",
      },
      alm: {
        note: "",
        calTitle: "{y}-{m}",
        weekHead: ["一", "二", "三", "四", "五", "六", "日"],
        signed: "已签",
        today: "今天",
        termOrFest: "节气",
      },
      attResub2: {
        btn: "补签",
        prompt: "补签日期",
        note: "消耗补签卡",
        ok: "补签成功",
        holdSuffix: "（持有 {n} 张）",
      },
    },
    currency: "魔力",
  }),
}));
// 万年历 dynamic 包不进 jsdom 测试——占位即可
vi.mock("next/dynamic", () => ({
  default: () =>
    function Placeholder() {
      return null;
    },
}));

import { CheckinPage } from "@/components/checkin-page";

describe("签到页规则卡", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });
  it("渲染每日/首次/里程碑/封顶规则文案", async () => {
    render(<CheckinPage />);
    await waitFor(() =>
      expect(screen.getByText("签到规则")).toBeTruthy(),
    );
    expect(screen.getByText("首次签到 +15 魔力")).toBeTruthy();
    expect(screen.getByText("每日签到 +8 魔力")).toBeTruthy();
    expect(
      screen.getByText("连签 10 天当日加送 +200（当日合计 208）"),
    ).toBeTruthy();
    expect(
      screen.getByText("连签 30 天当日加送 +1000（当日合计 1000）"),
    ).toBeTruthy(); // 8+1000 触顶 1000
    expect(screen.getByText("单日封顶 1000 魔力")).toBeTruthy();
  });
  it("streak=27 时唯一高亮下一档=30天", async () => {
    render(<CheckinPage />);
    await waitFor(() =>
      expect(screen.getByText("下一档 30 天")).toBeTruthy(),
    );
    // 10/20 已过，不标下一档
    expect(screen.queryByText("下一档 10 天")).toBeNull();
    expect(screen.queryByText("下一档 20 天")).toBeNull();
  });
  it("周期制：渲染周期说明行", async () => {
    render(<CheckinPage />);
    await waitFor(() =>
      expect(
        screen.getByText(/连签满 30 天后次日从第 1 天重新计算/),
      ).toBeTruthy(),
    );
    expect(screen.getByText(/当前周期第 27 天/)).toBeTruthy();
  });
  it("满周期者（streak=63→位置3）高亮的是下一周期第 10 天", async () => {
    // 动态改 mock：cycle_pos=3, next=10（后端口径），前端必须高亮 10 而非 30
    const { api } = await import("@/lib/api-client");
    (api.get as ReturnType<typeof vi.fn>).mockImplementation(
      async (url: string) => {
        if (url.includes("/attendance/calendar")) {
          return { ym: "2026-11", days: [] };
        }
        return {
          checked_today: true,
          streak: 63,
          recent: [],
          makeup_cards: 0,
          rules: {
            first: 15,
            streak: 8,
            daily_cap: 1000,
            milestones: [
              [10, 200],
              [20, 500],
              [30, 1000],
            ],
            cycle_days: 30,
            cycle_pos: 3,
            next_milestone_in_cycle: 10,
          },
        };
      },
    );
    render(<CheckinPage />);
    await waitFor(() =>
      expect(screen.getByText("下一档 10 天")).toBeTruthy(),
    );
    expect(screen.getByText(/当前周期第 3 天/)).toBeTruthy();
  });
});
