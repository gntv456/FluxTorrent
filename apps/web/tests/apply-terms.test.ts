import { describe, it, expect } from "vitest";
import {
  applyTerms,
  applyTermsText,
  sortRules,
  subtitleRules,
  type TermRule,
} from "@/i18n/apply-terms";

/**
 * 术语出口（0205 / 四审 L7）的前端侧语义测试。
 *
 * 这些用例是 `apps/api/src/terms.rs` 里那组单测的**逐条镜像**：
 * 改写发生在两个出口（前端字典、后端错误信封），只要有一边的语义漂了，
 * 同一句话就会在界面与提示语里长得不一样。改任何一边都必须同时改另一边。
 */
const R = (pairs: [string, string][]): TermRule[] =>
  sortRules(pairs.map(([canonical, replacement]) => ({
    canonical,
    replacement,
  })));

describe("applyTermsText（与 Rust terms::apply 对齐）", () => {
  it("零规则恒等：不改写、不重新分配", () => {
    const s = "今天已经签到过啦";
    expect(applyTermsText(s, [])).toBe(s);
  });

  it("同一句里多处出现都要改", () => {
    const rules = R([["种子", "资源"]]);
    expect(applyTermsText("该种子不是悬赏帖", rules)).toBe("该资源不是悬赏帖");
    expect(applyTermsText("种子种子", rules)).toBe("资源资源");
  });

  it("长词优先：短词不能把长词咬坏", () => {
    // 给乱序，sortRules 必须排成「种子文件」先于「种子」
    const rules = R([
      ["种子", "资源"],
      ["种子文件", "资源包"],
    ]);
    expect(applyTermsText("请上传种子文件", rules)).toBe("请上传资源包");
  });

  it("互指规则不级联：替换出的文本不再被扫描", () => {
    const rules = R([
      ["资源", "种子"],
      ["种子", "资源"],
    ]);
    expect(applyTermsText("种子", rules)).toBe("资源");
    expect(applyTermsText("资源的种子", rules)).toBe("种子的资源");
  });

  it("中英混排与 emoji 不被切碎", () => {
    const rules = R([["保种", "留存"]]);
    expect(applyTermsText("keep 保种🌱 协作", rules)).toBe("keep 留存🌱 协作");
  });

  it("{magic} / {n} 占位符是原子，撕不得", () => {
    const rules = R([
      ["magic", "积分"],
      ["n", "数量"],
    ]);
    expect(applyTermsText("获得 {magic} 与 {n}", rules)).toBe(
      "获得 {magic} 与 {n}",
    );
    // 占位符外面的同名词照常被改
    expect(applyTermsText("magic 与 {magic}", rules)).toBe("积分 与 {magic}");
  });

  it("字幕区改名（0146/0208）：复合词是原子，副产物不许出现", () => {
    // 音乐站「字幕 → 歌词」是 B1 的口径；真实文案取自 zh-CN.ts
    const rules = sortRules(subtitleRules("歌词"));
    expect(applyTermsText("字幕区", rules)).toBe("歌词区");
    expect(applyTermsText("字幕已更新", rules)).toBe("歌词已更新");
    // 复合词整体不拆：字幕组 / 字幕人（金字幕人靠「字幕人」保护）
    expect(applyTermsText("(译者或字幕组名，展示在字幕标题旁)", rules)).toBe(
      "(译者或字幕组名，展示在歌词标题旁)",
    );
    expect(applyTermsText("认证字幕人", rules)).toBe("认证字幕人");
    expect(applyTermsText("金字幕人（评选获奖）", rules)).toBe(
      "金字幕人（评选获奖）",
    );
    // 繁体字形同样受保护（替换文本是站长设的字样本身，不分语种）
    expect(applyTermsText("(譯者或字幕組名，展示在字幕標題旁)", rules)).toBe(
      "(譯者或字幕組名，展示在歌词標題旁)",
    );
  });
});

describe("applyTerms（整本字典）", () => {
  it("只动字符串叶子，key 与结构原样", () => {
    const dict = {
      nav: { torrents: "种子", count: "{n} 个种子" },
      list: ["做种", 7, true],
    };
    const rules = R([
      ["种子", "资源"],
      ["做种", "分享"],
    ]);
    expect(applyTerms(dict, rules)).toEqual({
      nav: { torrents: "资源", count: "{n} 个资源" },
      list: ["分享", 7, true],
    });
  });

  it("零规则时返回同一个引用（不复制对象树）", () => {
    const dict = { a: "种子" };
    expect(applyTerms(dict, [])).toBe(dict);
  });
});
