import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, cleanup, act } from "@testing-library/react";
import { readFileSync } from "node:fs";
import path from "node:path";
import {
  SPARK_EVENT,
  notifySparkChanged,
  useSparkSync,
} from "@/lib/spark-event";

/**
 * 「签到加了魔力，顶栏数字不动」的回归闸门。
 *
 * 背景（2026-10-21 以普通用户身份实测）：首页「立即签到」点下去，后端真的
 * 入账——`/api/v1/attendance/checkin` 回 code:0 reward:8，库里
 * `users.spark_balance` 8339→8347、`attendance` 落了今天；但页面**不刷新**
 * 时魔力读数仍停在 8,339。原因：顶栏 chip 的余额是 `user-menu` 自己
 * 进页面时读 `/me`+`/me/spark` 存进本地 state 的，首页签到成功只重拉了
 * `/api/v1/home`（签到卡自己那块变了），没有任何机制通知顶栏 ⇒ 每天第一个
 * 动作的「成功回执」在界面上看不见，用户只会以为没签到成功。
 *
 * 修法沿用仓里的事件惯例（`REPLY_EVENT` / `THANK_EVENT`）：写操作成功后
 * `notifySparkChanged()`，顶栏用 `useSparkSync(refreshSpark)` 回读。
 *
 * 本文件两类断言各管一头：
 *  ① 原语承重（删掉 useSparkSync 里的监听 / 让 notify 不发事件 ⇒ 立刻红）；
 *  ② 两侧接线在册（首页派发、顶栏订阅；任一侧被摘掉或事件模块被换掉 ⇒ 红）。
 * ②是源码级判据：先剥行注释与块注释再扫，避免注释里的词造假冒绿。
 */

const stripComments = (src: string): string =>
  src
    .replace(/\/\*[\s\S]*?\*\//g, "")
    .replace(/(^|[^:])\/\/[^\n]*/g, "$1");

const read = (rel: string): string =>
  stripComments(
    readFileSync(path.resolve(process.cwd(), "components", rel), "utf-8")
  );

beforeEach(() => cleanup());

describe("魔力余额广播（签到成功要能让顶栏回读）", () => {
  it("T1 事件名沿用 flux: 前缀的仓内惯例", () => {
    expect(SPARK_EVENT).toBe("flux:spark-changed");
  });

  it("T2 notifySparkChanged 真把事件发到 window，且订阅方收到", () => {
    const on = vi.fn();
    renderHook(() => useSparkSync(on));
    notifySparkChanged();
    expect(on).toHaveBeenCalledTimes(1);
    notifySparkChanged();
    expect(on).toHaveBeenCalledTimes(2);
  });

  it("T3 走原生 dispatchEvent 也要被订阅到（不只认封装）", () => {
    const on = vi.fn();
    renderHook(() => useSparkSync(on));
    act(() => {
      window.dispatchEvent(new Event(SPARK_EVENT));
    });
    expect(on).toHaveBeenCalledTimes(1);
  });

  it("T4 卸载后不再回调（防监听泄漏导致重复回读）", () => {
    const on = vi.fn();
    const { unmount } = renderHook(() => useSparkSync(on));
    notifySparkChanged();
    expect(on).toHaveBeenCalledTimes(1);
    unmount();
    notifySparkChanged();
    expect(on).toHaveBeenCalledTimes(1);
  });

  it("T5 回调换成新函数时不重复绑定监听", () => {
    const first = vi.fn();
    const second = vi.fn();
    const { rerender } = renderHook(
      ({ cb }: { cb: () => void }) => useSparkSync(cb),
      { initialProps: { cb: first } }
    );
    rerender({ cb: second });
    notifySparkChanged();
    expect(second).toHaveBeenCalledTimes(1);
    // 旧引用若还挂着，同一次广播会被算两遍——正是重复请求的成因
    expect(first).not.toHaveBeenCalled();
  });

  it("T6 首页签到成功后确实广播（派发侧在册）", () => {
    const src = read("home-sections.tsx");
    const body = src.slice(src.indexOf("async function checkin"));
    expect(body).toContain("notifySparkChanged(");
    expect(src).toContain('from "@/lib/spark-event"');
  });

  it("T7 顶栏确实订阅（消费侧在册，且指向回读余额的那个回调）", () => {
    const src = read("user-menu.tsx");
    expect(src).toContain("useSparkSync(refreshSpark)");
    expect(src).toContain('from "@/lib/spark-event"');
  });

  it("T8 反证：签到卡自身的 load() 不能替代广播（两者不同链路）", () => {
    const src = read("home-sections.tsx");
    const body = src.slice(src.indexOf("async function checkin"));
    const seg = body.slice(0, body.indexOf("} catch"));
    // 广播必须在成功分支里、且与 load() 并存——只留 load() 就是原缺陷
    expect(seg).toContain("notifySparkChanged(");
    expect(seg).toContain("load()");
  });
});
