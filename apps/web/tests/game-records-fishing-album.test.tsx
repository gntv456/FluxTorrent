import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen } from "@testing-library/react";

import { GameRecordsPage } from "@/components/game/game-records";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/**
 * 钓鱼战绩页的图鉴块。
 *
 * 通用图鉴端点只认 capsule/jgg/scratch/wheel，对 fishing 回 400；界面上
 * 旧的写法把错误静默吞掉，于是「对局记录 + 奖档图鉴整页」永远只剩记录。
 * 钓鱼的真图鉴在 /games/fishing/collection（标准塘 + 渔汛塘合并）。
 */

const SUB = zhCN.games.sub;
const FISH = zhCN.games.fishing;
const urls: string[] = [];

function res(payload: unknown, status = 200): Response {
  return {
    ok: status >= 200 && status < 300,
    status,
    headers: { get: () => "application/json" },
    json: async () => payload,
  } as unknown as Response;
}

function stub() {
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: RequestInfo | URL) => {
      const url = String(input);
      urls.push(url);
      if (url.includes("/games/rounds")) {
        return res({
          code: 0,
          data: [
            {
              game: "fishing",
              bet: 100,
              payout: 0,
              net: -100,
              at: "2026-10-21T00:00:00Z",
            },
          ],
          request_id: "t",
        });
      }
      if (url.includes("/games/fishing/collection")) {
        return res({
          code: 0,
          data: {
            got: 2,
            total: 10,
            fishes: [
              { label: "鲫鱼", rarity: 1, event: false, count: 3, mult: 1.2 },
              { label: "锦鲤", rarity: 4, event: true, count: 1, mult: 10 },
            ],
          },
          request_id: "t",
        });
      }
      if (url.includes("/games/collection/capsule")) {
        return res({
          code: 0,
          data: {
            got: 1,
            total: 2,
            entries: [
              { label: "银票", rarity: 2, count: 4, mult: 1.5, value: 150 },
              { label: "金票", rarity: 4, count: 0, mult: 5, value: 500 },
            ],
          },
          request_id: "t",
        });
      }
      return res(
        {
          code: 1002,
          message: "参数校验失败: 「fishing」没有图鉴（可选 capsule/jgg/scratch/wheel）",
          data: null,
          request_id: "t",
        },
        400,
      );
    }),
  );
}

beforeEach(() => {
  urls.length = 0;
});

afterEach(() => {
  vi.unstubAllGlobals();
});

function mount(game: string) {
  return render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      <GameRecordsPage game={game} />
    </LocaleProvider>,
  );
}

describe("战绩页图鉴端点选择", () => {
  it("钓鱼走专属图鉴端点，并把 fishes 渲染成图鉴", async () => {
    stub();
    mount("fishing");
    expect(await screen.findByText(FISH.albumTitle)).toBeTruthy();
    expect(screen.getByText("鲫鱼")).toBeTruthy();
    expect(screen.getByText("锦鲤")).toBeTruthy();
    expect(urls.some((u) => u.includes("/games/collection/fishing"))).toBe(
      false,
    );
    expect(urls.some((u) => u.includes("/games/fishing/collection"))).toBe(
      true,
    );
  });

  it("扭蛋等通用游戏仍走 /games/collection/{game}", async () => {
    stub();
    mount("capsule");
    expect(await screen.findByText(SUB.albumTitle)).toBeTruthy();
    expect(screen.getByText("银票")).toBeTruthy();
    expect(screen.queryByText("金票")).toBeNull();
    expect(urls.some((u) => u.includes("/games/collection/capsule"))).toBe(
      true,
    );
  });
});
