import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen } from "@testing-library/react";

import { SubtitleAwardsBoard } from "@/components/subtitle-awards-board";
import { GameRecordsPanel } from "@/components/game/game-records";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/** 榜单/战绩：取数失败不能演成「本期没有评选结果」「还没有对局」，
 *  更不能把零统计卡当成真实战绩。 */

afterEach(() => vi.unstubAllGlobals());

function envelope(data: unknown) {
  return {
    ok: true,
    status: 200,
    headers: { get: () => "application/json" },
    json: async () => ({ code: 0, message: "ok", data, request_id: "t" }),
  } as unknown as Response;
}

function mount(node: React.ReactNode) {
  return render(
    <LocaleProvider dict={zhCN} locale="zh-CN">{node}</LocaleProvider>,
  );
}

describe("评选榜失败不演空榜", () => {
  it("awards 取数失败：报加载失败，不报「暂无评选结果」", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn((url: string) => {
        if (url.includes("/site-profile")) {
          return Promise.resolve(envelope({ subtitle_award: true }));
        }
        return Promise.reject(new Error("network down"));
      }),
    );
    mount(<SubtitleAwardsBoard />);
    expect(await screen.findByText(zhCN.common.loadFailed)).toBeTruthy();
    expect(screen.queryByText(zhCN.subtitles.awardEmpty)).toBeNull();
  });
});

describe("游戏战绩失败不演空记录", () => {
  it("rounds 取数失败：报加载失败，不显示零统计卡", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn((url: string) => {
        if (url.includes("/games/rounds")) {
          return Promise.reject(new Error("network down"));
        }
        return Promise.resolve(envelope({ got: 0, total: 0, entries: [] }));
      }),
    );
    mount(<GameRecordsPanel game="wheel" />);
    expect(await screen.findByText(zhCN.common.loadFailed)).toBeTruthy();
    expect(screen.queryByText(zhCN.games.sub.empty)).toBeNull();
    expect(screen.queryByText(zhCN.games.sub.statPlays)).toBeNull();
  });
});
