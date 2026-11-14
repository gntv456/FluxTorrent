import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";

import { GachaDraw } from "@/components/gacha-draw";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/**
 * 抽卡主玩法（波1 P0）：后端 /gacha/draw 全链在位，此前 Web 端零调用。
 * 断言：单抽发正确载荷（banner_id/count/幂等键）、券不足禁十连、
 * 结果行渲染（星数/碎片/NEW 标记）、金档/保底不炸。
 */

const RARITIES = [
  {
    key: "SSR",
    label: "SSR",
    sort: 1,
    stars: 5,
    goldRank: 1,
    frame: null,
    frameHi: null,
    glow: null,
    bg1: null,
    bg2: null,
    ink: null,
    foil: 0,
    foilMask: "",
  },
  {
    key: "R",
    label: "R",
    sort: 3,
    stars: 2,
    goldRank: 3,
    frame: null,
    frameHi: null,
    glow: null,
    bg1: null,
    bg2: null,
    ink: null,
    foil: 0,
    foilMask: "",
  },
];

function res(payload: unknown, status = 200): Response {
  return {
    ok: status >= 200 && status < 300,
    status,
    headers: { get: () => "application/json" },
    json: async () => payload,
  } as unknown as Response;
}

let postBody: Record<string, unknown> | null = null;

function stubDraw(rows: unknown[]) {
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = String(input);
      if (url.includes("/gacha/draw") && init?.method === "POST") {
        postBody = JSON.parse(String(init.body)) as Record<string, unknown>;
        return res({
          code: 0,
          data: {
            replayed: false,
            bannerId: 1,
            rows,
            ticketBalance: 15,
            shardBalance: 3,
          },
          request_id: "t",
        });
      }
      return res({ code: 0, data: {}, request_id: "t" });
    }),
  );
}

function mount(tickets = 30) {
  return render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      <GachaDraw
        bannerId={1}
        ticketCost={25}
        ticketBalance={tickets}
        rarities={RARITIES}
      />
    </LocaleProvider>,
  );
}

beforeEach(() => {
  postBody = null;
});
afterEach(() => {
  vi.unstubAllGlobals();
});

describe("GachaDraw", () => {
  it("单抽发 banner_id=1/count=1/带幂等键", async () => {
    stubDraw([
      {
        seq: 1,
        type: "card",
        r: "R",
        shards: 0,
        pityAt: 3,
        wasPity: false,
        isNew: true,
        cardId: 7,
      },
    ]);
    const { container } = mount();
    const one = Array.from(container.querySelectorAll("button")).find((b) =>
      b.textContent.includes("单抽"),
    );
    expect(one).toBeTruthy();
    fireEvent.click(one!);
    await waitFor(() => expect(postBody).not.toBeNull());
    expect(postBody!["banner_id"]).toBe(1);
    expect(postBody!["count"]).toBe(1);
    expect(String(postBody!["idempotency_key"])).toContain("ui-gacha-");
    // 结果行渲染：R 卡两星 + NEW 标记
    await waitFor(() => {
      expect(container.textContent).toContain("★★");
      expect(container.textContent).toContain("NEW!");
    });
  });

  it("券不足时十连禁用", () => {
    stubDraw([]);
    const { container } = mount(10);
    const ten = Array.from(container.querySelectorAll("button")).find((b) =>
      b.textContent.includes("十连"),
    );
    expect(ten?.disabled).toBe(true);
    const one = Array.from(container.querySelectorAll("button")).find((b) =>
      b.textContent.includes("单抽"),
    );
    expect(one?.disabled).toBe(false);
  });

  it("十连载荷 count=10，碎片行渲染 + 数值", async () => {
    stubDraw([
      { seq: 1, type: "shard", r: null, shards: 12, pityAt: 4, wasPity: false },
      { seq: 2, type: "card", r: "SSR", shards: 0, pityAt: 0, wasPity: true },
    ]);
    const { container } = mount(300);
    const ten = Array.from(container.querySelectorAll("button")).find((b) =>
      b.textContent.includes("十连"),
    );
    fireEvent.click(ten!);
    await waitFor(() => expect(postBody).not.toBeNull());
    expect(postBody!["count"]).toBe(10);
    await waitFor(() => {
      expect(container.textContent).toContain("+12");
      expect(container.textContent).toContain("保底");
    });
  });
});
