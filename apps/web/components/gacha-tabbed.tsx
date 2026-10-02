"use client";

/**
 * 抽卡页的 tab 壳（样图⑧：卡池 / 概率 / 我的卡册 三 tab）。
 * /gacha 页面本体是 RSC（SSR 拉数据），tab 切换是客户端行为——
 * 三块内容都由 RSC 预渲染好传进来，这里只负责显隐。
 */
import { useState, type ReactNode } from "react";
import { GameTabs } from "@/components/game/game-tabs";

export function GachaTabbed({
  labels,
  pool,
  rates,
  album,
}: {
  labels: { pool: string; rates: string; album: string };
  pool: ReactNode;
  rates: ReactNode;
  album: ReactNode;
}) {
  const [tab, setTab] = useState("pool");
  return (
    <>
      <GameTabs
        tabs={[
          { key: "pool", label: labels.pool, icon: "🃏" },
          { key: "rates", label: labels.rates, icon: "📊" },
          { key: "album", label: labels.album, icon: "📖" },
        ]}
        active={tab}
        onChange={setTab}
      />
      {tab === "pool" && pool}
      {tab === "rates" && rates}
      {tab === "album" && album}
    </>
  );
}
