import { describe, it, expect } from "vitest";
import { deepMergeDict } from "@/i18n/merge";
import { zhCN } from "@/i18n/zh-CN";
import { ja } from "@/i18n/ja";
import type { Dict } from "@/i18n/zh-CN";

/** 段级回落语言（E10）的深合并语义：翻到的覆盖、缺的回落、类型完整。 */
describe("deepMergeDict（ja 段级回落）", () => {
  const merged: Dict = deepMergeDict(zhCN, ja);

  it("翻到的段生效（nav.home 为日语）", () => {
    expect(merged.nav.home).toBe("ホーム");
  });

  it("未翻的键回落 zh-CN（nav 里 ja 没翻的键）", () => {
    // ja.nav 只翻了一部分：未翻键（如 bank/donate）应保持中文
    expect(merged.nav.bank).toBe(zhCN.nav.bank);
    expect(merged.nav.donate).toBe(zhCN.nav.donate);
  });

  it("未翻的整段回落 zh-CN（ja 没碰的顶层段）", () => {
    expect(merged.forums).toEqual(zhCN.forums);
    expect(merged.gomoku).toEqual(zhCN.gomoku);
  });

  it("段级部分覆盖不丢同段兄弟键", () => {
    // common 段被部分覆盖，但 zh-CN.common 的全部键仍在
    for (const k of Object.keys(zhCN.common)) {
      expect(merged.common).toHaveProperty(k);
    }
  });

  it("合并结果是完整的 Dict（每个顶层段都在）", () => {
    for (const k of Object.keys(zhCN)) {
      expect(merged).toHaveProperty(k);
    }
  });

  it("占位符在译文中保留（nav.spark 的 {magic}）", () => {
    expect(merged.nav.spark).toContain("{magic}");
  });
});
