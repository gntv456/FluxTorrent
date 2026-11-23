import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";

import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";
import { RoomStage } from "@/app/(main)/games/bigsmall/_parts/room-stage";
import { RoomControls } from "@/app/(main)/games/bigsmall/_parts/room-controls";
import type { RoomState } from "@/app/(main)/games/bigsmall/_parts/room-types";

/** 猜大小房间子件：回落态渲染、截止禁用、上期回放与三骰结构。 */

const tg = zhCN.games.bigsmall;

function roomFixture(over: Partial<RoomState> = {}): RoomState {
  return {
    round: {
      id: 7, remaining: 23, window: 30, total_small: 120, total_big: 80,
    },
    ticket: 100,
    win_mult: 1.9,
    mine: [{ side: "small", amount: 100, payout: 0, net: -100 }],
    my_total: 100,
    last: { dice: [3, 4, 5], sum: 12, region: "big" },
    ...over,
  };
}

function mountStage(room: RoomState | null) {
  return render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      <RoomStage
        room={room}
        winMult={1.9}
        streak={0}
        todayPlays={3}
        limitLeft={5}
        err={null}
        unknown={false}
        onRetry={() => undefined}
      />
    </LocaleProvider>,
  );
}

function mountControls(closed: boolean, room: RoomState | null) {
  return render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      <RoomControls
        bet={100}
        onBet={() => undefined}
        maxBet={1000}
        busy={false}
        closed={closed}
        room={room}
        winMult={1.9}
        myTotal={100}
        onPlace={() => undefined}
      />
    </LocaleProvider>,
  );
}

describe("猜大小房间舞台", () => {
  it("房间数据缺失时回落等待态，仍渲染三骰", () => {
    mountStage(null);
    expect(screen.getByText(tg.bsDiceWait)).toBeTruthy();
    expect(document.querySelectorAll(".bs-die")).toHaveLength(3);
  });

  it("带上期开奖：回放上期结果与分区", () => {
    mountStage(roomFixture());
    const recent = document.querySelector(".bs-last-panel");
    expect(recent?.textContent).toContain(tg.roomLast);
    expect(recent?.textContent).toContain(tg.bsPickBig);
  });

  it("舞台下接在线玩家头像圈（静态演示 7 席）", () => {
    mountStage(roomFixture());
    expect(document.querySelector(".bs-rail")).toBeTruthy();
    expect(document.querySelectorAll(".bs-rail-player")).toHaveLength(7);
  });

  it("展示当前期号徽章", () => {
    mountStage(roomFixture());
    expect(
      screen.getByText(tg.roomRound.replace("{n}", "7")),
    ).toBeTruthy();
  });
});

describe("猜大小房间下注盘", () => {
  it("进行中：大/小按钮可点，豹子为展示位", () => {
    mountControls(false, roomFixture());
    const picks = document.querySelectorAll(".bs-pick");
    expect(picks).toHaveLength(3);
    const small = screen.getByText(tg.bsPickSmall).closest("button");
    const big = screen.getByText(tg.bsPickBig).closest("button");
    expect(small?.disabled).toBe(false);
    expect(big?.disabled).toBe(false);
    expect(
      document.querySelector('.bs-pick[aria-disabled="true"]'),
    ).toBeTruthy();
    expect(document.querySelectorAll(".bs-chip")).toHaveLength(3);
  });

  it("截止后：大/小下注按钮禁用", () => {
    mountControls(true, roomFixture());
    const small = screen.getByText(tg.bsPickSmall).closest("button");
    const big = screen.getByText(tg.bsPickBig).closest("button");
    expect(small?.disabled).toBe(true);
    expect(big?.disabled).toBe(true);
  });

  it("房间数据缺失时下注盘仍回落渲染，不崩", () => {
    mountControls(false, null);
    expect(screen.getByText(tg.bsPickSmall)).toBeTruthy();
    expect(screen.getByText(tg.bsPickBig)).toBeTruthy();
  });
});
