import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";

import { tokenSecondsLeft } from "@/lib/session-ttl";

/**
 * 「记住登录」的前端半边（2026-11-01 手机版登录页重设计）。
 *
 * 会话标记 cookie `flux.session` 是 middleware 判登录态的**唯一**依据
 * （它不看 HttpOnly 的 flux_token）。旧版把它写死 12h，于是后端把票
 * 拉到 30 天之后，第 13 个小时人就被当游客弹回登录页——remember 形同
 * 虚设。修法不是在前端再写一个 30 天常量（两处数字迟早漂），而是读
 * 服务端刚发回来那枚票的 exp。
 *
 * 关键性质：解不出的一律 undefined，让调用方回落到 12h——判不准时取
 * 窄的那头，绝不凭空放宽。
 */

const b64url = (s: string) =>
  btoa(s).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");

const jwt = (payload: unknown) =>
  ["e30", b64url(JSON.stringify(payload)), "c2ln"].join(".");

// 冻结时钟：不冻的话「exp - now」会跨秒边界抖 1 秒，断言就成了偶发红
beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(new Date("2026-11-01T00:00:00Z"));
});
afterEach(() => vi.useRealTimers());

const NOW = Math.floor(new Date("2026-11-01T00:00:00Z").getTime() / 1000);

describe("tokenSecondsLeft（记住登录的前端半边）", () => {
  it("正控：读的是 exp，改 exp 结果必须跟着动", () => {
    expect(tokenSecondsLeft(jwt({ exp: NOW + 3600 }))).toBe(3600);
    expect(tokenSecondsLeft(jwt({ exp: NOW + 7200 }))).toBe(7200);
    expect(tokenSecondsLeft(jwt({ exp: NOW + 30 * 86400 }))).toBe(
      30 * 86400,
    );
  });

  it("已过期 → undefined（不给负数，也不放宽）", () => {
    expect(tokenSecondsLeft(jwt({ exp: NOW - 10 }))).toBeUndefined();
  });

  it("解不出的形状一律 undefined", () => {
    expect(tokenSecondsLeft(undefined)).toBeUndefined();
    expect(tokenSecondsLeft("")).toBeUndefined();
    expect(tokenSecondsLeft("不是JWT")).toBeUndefined();
    expect(tokenSecondsLeft("e30")).toBeUndefined(); // 没有 payload 段
    expect(tokenSecondsLeft("e30.!!!not-base64!!!")).toBeUndefined();
    expect(tokenSecondsLeft(jwt({ sub: 1 }))).toBeUndefined(); // 有票无 exp
    expect(tokenSecondsLeft(jwt({ exp: "3600" }))).toBeUndefined(); // 非数字
  });

  it("payload 不是合法 JSON 时被 catch 兜住，不抛错", () => {
    // 坏票必须安静回落到默认 12h：登录已经成功，不能因为解不开
    // 自报到期时间就在前端抛异常
    expect(
      tokenSecondsLeft(["e30", b64url("hello"), "c2ln"].join(".")),
    ).toBeUndefined();
    expect(
      tokenSecondsLeft(["e30", b64url("[1,2"), "c2ln"].join(".")),
    ).toBeUndefined();
  });
});
