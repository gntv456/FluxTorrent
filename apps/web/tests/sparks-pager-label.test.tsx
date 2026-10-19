import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";

import MySparksPage from "@/app/(main)/me/sparks/page";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";
import { sparkKindLabels } from "@/lib/spark-kind-labels";

/**
 * 火花流水页的翻页条必须同时报出「第几页」和「共几页」。
 *
 * 该页用的是 Pager 的 compact 排版——这个排版有意不显示「共 N 条」，
 * 于是不传 posLabel 时整块只剩「上一页 / 第 2 页 / 下一页」：重度用户
 * 翻到第二页就不知道自己一共有多少页，也判断不了还剩多少没看。
 * 2026-10-17 浏览器实测就是这样，补 posLabel 后才对齐样图。
 */

const PER = 50;

function rows(n: number) {
  return Array.from({ length: n }, (_, i) => ({
    amount: i % 3 === 0 ? -5 : 20,
    kind: "forum",
    balance_after: 1000 + i,
    created_at: "2026-10-16T07:27:28Z",
    item_name: null,
    ref_type: null,
    ref_id: null,
  }));
}

function stubLedger(total: number) {
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: RequestInfo | URL) => {
      const url = String(input);
      const data = url.includes("/me/spark/ledger")
        ? {
            items: rows(Math.min(PER, total)),
            total,
            page: 1,
            per: PER,
          }
        : { balance: 1234 };
      return {
        ok: true,
        status: 200,
        headers: { get: () => "application/json" },
        json: async () => ({ code: 0, message: "ok", data, request_id: "t" }),
      } as unknown as Response;
    }),
  );
}

function mount() {
  return render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      <MySparksPage />
    </LocaleProvider>,
  );
}

afterEach(() => vi.unstubAllGlobals());

describe("流水页翻页条的页数信息", () => {
  it("450 条：标签同时写当前页与总页数", async () => {
    stubLedger(450);
    mount();
    await waitFor(() => expect(document.body.textContent).toContain("论坛"));
    const pager = document.querySelector(".ui-pager");
    const label = (pager?.textContent ?? "").replace(/\s+/g, " ");
    expect(label).toMatch(/第 1 页 \/ 9/);
  });

  it("不满一页：不画翻页条（否则只有一页也报页数）", async () => {
    stubLedger(20);
    mount();
    await waitFor(() => expect(document.body.textContent).toContain("论坛"));
    expect(document.querySelector(".ui-pager")).toBeNull();
  });
});


describe("kind 标签必须来自调用方字典（P-425 接线）", () => {
  // 只测接线本身：旧实现把标签表硬绑 zh-CN 源字典，于是 i18n 的段级回落机制
  // 对这张表完全不起作用（将来补任何语种都不会生效）。这里不依赖词典内容，
  // 免得「某个语种恰好没翻」被读成「接线坏了」。
  it("传入字典优先于源字典；不传时回落 zh-CN", () => {
    expect(sparkKindLabels({ kinds: { shop: "Shop purchase" } }).shop).toBe(
      "Shop purchase",
    );
    const fallback = sparkKindLabels();
    expect(Object.keys(fallback).length).toBeGreaterThan(30);
    expect(fallback.shop).toBe(zhCN.my.kinds.shop);
  });
  it("两个语种给同一批键时各取自己的文案", () => {
    const a = sparkKindLabels({ kinds: { forum: "AAA" } });
    const b = sparkKindLabels({ kinds: { forum: "BBB" } });
    expect(a.forum).toBe("AAA");
    expect(b.forum).toBe("BBB");
  });
});
