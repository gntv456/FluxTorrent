import { describe, it, expect } from "vitest";

import {
  examCurrent,
  examSummary,
  examTarget,
} from "@/app/(main)/me/exams/_parts/exam-metrics";

/** 考核页折算逻辑（从路由页抽出，页面当时正踩 300 行门禁）。
 *  这里钉的是「单位口径」：现值与目标值必须同一套单位，否则表格会
 *  渲染成「1.2 GB / 4320」这种一边字节一边秒数的对照。 */

describe("考核指标折算", () => {
  it("字节键走 formatBytes，秒键折算小时并带单位", () => {
    expect(examCurrent("upload_delta", { uploaded: 1024 ** 3 }, "h")).toBe(
      "1.0 GB",
    );
    expect(
      examCurrent("seed_seconds_delta", { seed_seconds: 7200 }, "小时"),
    ).toBe("2.0 小时");
  });

  it("download_delta 有现值就报字节，没有才回「—」（P-527 补第五源）", () => {
    const five = 5 * 1024 ** 3;
    expect(examCurrent("download_delta", { downloaded: five }, "h")).toBe(
      "5.0 GB",
    );
    // 现值与目标值必须同一套单位，否则表格又是「5.0 GB / 5368709120」
    expect(examCurrent("download_delta", { downloaded: five }, "h")).toBe(
      examTarget("download_delta", five, "h"),
    );
    // 后端没下发 downloaded（旧契约）时不许编数
    expect(examCurrent("download_delta", { uploaded: 999 }, "h")).toBe("—");
    expect(examCurrent("download_delta", {}, "h")).toBe("—");
  });

  it("认不出的键、缺字段的键都回「—」而不是 undefined/NaN", () => {
    expect(examCurrent("mystery", { uploaded: 5 }, "h")).toBe("—");
    expect(examCurrent("upload_delta", {}, "h")).toBe("—");
    expect(examTarget("mystery", 0, "h")).toBe("0");
  });

  it("现值与目标值同单位：seed_seconds 两边都折小时", () => {
    const cur = examCurrent("seed_seconds_delta", { seed_seconds: 3600 }, "h");
    const tgt = examTarget("seed_seconds_delta", 3600, "h");
    expect(cur).toBe("1.0 h");
    expect(tgt).toBe("1 h");
    expect(cur.endsWith("h")).toBe(tgt.endsWith("h"));
  });
});

describe("考核顶部两指标", () => {
  const e = (status: number, deadline: string | null) => ({ status, deadline });
  const NOW = Date.parse("2026-10-18T00:00:00Z");

  it("达标率四舍五入；空组算 0 而不是 NaN", () => {
    expect(examSummary([e(1, null), e(1, null), e(0, null)], NOW).rate).toBe(
      67,
    );
    expect(examSummary([], NOW)).toEqual({ rate: 0, daysLeft: null });
  });

  it("剩余天数取最早那条未达标项，已达标的期限不算", () => {
    const rows = [
      e(0, "2026-10-23T00:00:00Z"),
      e(0, "2026-10-29T00:00:00Z"),
      e(1, "2026-10-20T00:00:00Z"),
    ];
    expect(examSummary(rows, NOW).daysLeft).toBe(5);
  });

  it("已过期不报负数（夹到 0），没有未完成项就是 null", () => {
    const past = examSummary([e(0, "2026-10-01T00:00:00Z")], NOW);
    expect(past.daysLeft).toBe(0);
    const none = examSummary(
      [e(0, null), e(1, "2026-11-01T00:00:00Z")],
      NOW,
    );
    expect(none.daysLeft).toBeNull();
  });

  it("坏日期字符串不会把整页顶成 NaN", () => {
    expect(examSummary([e(0, "not-a-date")], NOW).daysLeft).toBeNull();
  });
});
