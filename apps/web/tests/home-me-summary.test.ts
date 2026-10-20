import { describe, it, expect } from "vitest";

import { homeMeSummary } from "@/app/(main)/_parts/home-me-summary";
import { formatBytes, formatRatio } from "@/lib/format";

/** 首页移动数据条（M3）的四个数。
 *
 * 钉的是「手机与电脑必须是同一个数」：这一格的上传/下载此前不走站内的
 * `formatBytes`，而是自己按 1000³ 折算再贴一个 "G"，于是同一账号在两个
 * 断点读出两个值。批二百廿三（2026-10-20）把它并回共用格式化函数。
 */

const GIB = 1024 ** 3;
const MIB = 1024 ** 2;
const TIB = 1024 ** 4;

describe("homeMeSummary（首页移动数据条）", () => {
  it("手机与电脑读同一支 formatBytes（5 GiB 不再报成 5.4G）", () => {
    const s = homeMeSummary(
      { uploaded: 5 * GIB, downloaded: 2 * GIB }, "zh-CN");
    expect(s?.uploaded).toBe(formatBytes(5 * GIB));
    expect(s?.uploaded).toBe("5.0 GB");
    // 旧口径：`${(5 * GIB) / 1e9}`.toFixed(1) === "5.4G"（按 1000³ 折算却标 G）
    expect(s?.uploaded).not.toMatch(/^5\.4G$/);
  });

  it("下载过 30 MiB 不许显示成 0", () => {
    const s = homeMeSummary({ uploaded: 0, downloaded: 30 * MIB }, "zh-CN");
    expect(s?.downloaded).toBe("30.0 MB");
    // 旧口径给的是 "0.0G"：明明下载过，屏上是没有
    expect(s?.downloaded).not.toBe("0.0G");
  });

  it("TB 级不进位到一长串 G", () => {
    const s = homeMeSummary({ uploaded: 3 * TIB, downloaded: 0 }, "zh-CN");
    expect(s?.uploaded).toBe("3.0 TB");
  });

  it("分享率与右上角同一支函数（同一账号同一个文本）", () => {
    const up = 500 * GIB;
    const down = 2 * GIB; // 250 倍——共用函数按既有口径给 INF 档
    const s = homeMeSummary({ uploaded: up, downloaded: down }, "zh-CN");
    expect(s?.ratio).toBe(formatRatio(up, down));
    const s2 = homeMeSummary(
      { uploaded: 2 * GIB, downloaded: 1 * GIB }, "zh-CN");
    expect(s2?.ratio).toBe("2.00");
  });

  it("没下载过 = 无界，不是 0.00", () => {
    const s = homeMeSummary({ uploaded: 1 * GIB, downloaded: 0 }, "zh-CN");
    expect(s?.ratio).toBe(formatRatio(1 * GIB, 0));
    expect(s?.ratio).not.toBe("0.00");
  });

  it("魔力数仍按传入 locale 分组；缺字段按 0 计", () => {
    const s = homeMeSummary({ spark_balance: 1234567 }, "en-US");
    expect(s?.spark).toBe("1,234,567");
    const z = homeMeSummary({}, "zh-CN");
    expect(z).toEqual({
      uploaded: formatBytes(0),
      downloaded: formatBytes(0),
      ratio: formatRatio(0, 0),
      spark: (0).toLocaleString("zh-CN"),
    });
  });

  it("取数失败（null）仍回落 null：只渲染问候语", () => {
    expect(homeMeSummary(null, "zh-CN")).toBeNull();
  });
});
