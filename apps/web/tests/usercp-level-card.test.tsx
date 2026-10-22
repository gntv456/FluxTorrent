import { describe, it, expect } from "vitest";
import { render } from "@testing-library/react";

import { SummaryRow } from "@/components/usercp-overview-panels";
import type { Overview } from "@/components/usercp";
import { levelProgress, type NextClass } from "@/lib/usercp-level";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/**
 * 会员中心「等级进度」卡（P-516）。现网踩过：分子写的是「在做种数×100」的
 * 假积分，分母却是下一等级的**上传量门槛**（字节），两个量纲相除 ⇒ 进度条
 * 恒 0%、「距 LV2 还差」永远等于整个门槛。现在按 class_adj.rs 的四项权威
 * 条件折算，卡上显示其中最慢的那一项。门槛取自现网 class_rules 的 LV2 行
 * （50GB / 完成 241 / 做种 240h / 账龄 28 天）。
 */

const GB = 1024 ** 3;
const P = zhCN.userProfile2;

const LV2: NextClass = {
  name: "LV2",
  uploaded: 20 * GB,
  uploaded_need: 50 * GB,
  download_count: 500,
  download_count_need: 241,
  seed_hours: 900,
  seed_hours_need: 240,
  account_age_days: 400,
  account_age_days_need: 28,
};

const D = {
  up: P.upProgress,
  dl: P.dlProgress,
  seed: P.seedProgress,
  age: P.ageProgress,
  hours: P.hours,
  days: P.days,
};

describe("等级进度折算", () => {
  it("四项里最慢的一项决定进度（此处上传量 40%）", () => {
    const lv = levelProgress(LV2, D);
    expect(lv?.label).toBe(P.upProgress);
    expect(lv?.pct).toBe(40);
    expect(lv?.gap).toBe("30.0 GB");
  });

  it("做种时长更慢时换成做种时长那一组数", () => {
    const lv = levelProgress({ ...LV2, uploaded: 49 * GB, seed_hours: 12 }, D);
    expect(lv?.label).toBe(P.seedProgress);
    expect(lv?.text).toBe(`12 ${P.hours} / 240 ${P.hours}`);
  });

  it("门槛为 0 视为该项已达标（与公开主页 prog() 同式），不抢最低项", () => {
    const lv = levelProgress({ ...LV2, uploaded_need: 0, seed_hours: 10 }, D);
    expect(lv?.label).toBe(P.seedProgress);
    expect(lv?.pct).toBe(4);
  });

  it("四项全达标 ⇒ 满格且差值按 0 收，不出负数", () => {
    const lv = levelProgress(
      { ...LV2, uploaded: 60 * GB, seed_hours: 300 },
      D,
    );
    expect(lv?.pct).toBe(100);
    expect(lv?.gap).not.toMatch(/-/);
  });

  it("没有下一档（职员档）⇒ null，由界面显示已达最高级", () => {
    expect(levelProgress(null, D)).toBeNull();
    expect(levelProgress(undefined, D)).toBeNull();
  });

  it("旧契约（没有四项键）不许把 NaN 画进进度条", () => {
    const legacy = { name: "LV2", required: 50 * GB } as unknown as NextClass;
    expect(levelProgress(legacy, D)).toBeNull();
    // 只有一项缺键时其余三项照常折算，最慢的仍是做种时长 4%
    const partial = {
      ...LV2,
      uploaded_need: undefined,
      seed_hours: 10,
    } as unknown as NextClass;
    expect(levelProgress(partial, D)?.label).toBe(P.seedProgress);
  });
});

function ovFixture(over: Partial<Overview> = {}): Overview {
  return {
    id: 1,
    username: "tester",
    email: "t@e.st",
    class_name: "User",
    avatar_url: null,
    created_at: "2026-01-01T00:00:00Z",
    uploaded: 20 * GB,
    downloaded: 10 * GB,
    ratio: 2,
    seeding: 7,
    leeching: 0,
    uploads: 1,
    comments: 0,
    spark_balance: 0,
    invites_pending: 0,
    invites_used: 0,
    medals: 0,
    parked: false,
    privacy: "normal",
    totp_enabled: false,
    passkey: "pk",
    login_trend_30d: [],
    login_days_30d: 0,
    login_total_30d: 0,
    last_login: null,
    next_class: LV2,
    ...over,
  };
}

function renderCard(ov: Overview) {
  return render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      <SummaryRow
        ov={ov}
        t={zhCN.usercp.overview}
        dict={zhCN}
        level={levelProgress(ov.next_class, D)}
      />
    </LocaleProvider>,
  );
}

describe("等级进度卡渲染", () => {
  it("显示最慢条件的同量纲对照，不再出现「做种积分」", () => {
    const { container, queryByText } = renderCard(ovFixture());
    expect(container.textContent).toContain(
      `${P.upProgress} 20.0 GB / 50.0 GB`,
    );
    expect(container.textContent).toContain("距 LV2 还差30.0 GB");
    // 旧文案：做种积分 700 / 50.0G（假积分 ÷ 字节门槛）
    expect(queryByText(/做种积分/)).toBeNull();
  });

  it("等级名取后端下发的 class_name（P-515 恒空时这里是「—」）", () => {
    const { container } = renderCard(ovFixture());
    expect(container.textContent).toContain("User");
  });

  it("已达最高级时显示满格与「最高级」文案", () => {
    const { container } = renderCard(ovFixture({ next_class: null }));
    expect(container.innerHTML).toContain("width: 100%");
    expect(container.textContent).toContain(P.maxLevel);
  });
});
