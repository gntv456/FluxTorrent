"use client";

import { useI18n } from "@/i18n/client";

/**
 * 票根册：一叠齿孔邮票纸。
 *
 * 未获得的那几张刻意不画问号 —— 一整排「❔」读起来像图挂了；改用名字首字做
 * 压灰雕版，「这里有东西、你还没拿到」才读得出来。计数牌挂在纸下缘，
 * 灰底上要换成浅底暖字，否则暗牌压深字等于没字。
 */

export interface ArcadeStubView {
  code: string;
  name: string;
  descr: string;
  obtained: boolean;
  holders: number;
}

export function ArcadeAlbum({
  stubs,
}: {
  stubs: { owned: number; total: number; items: ArcadeStubView[] };
}) {
  const { dict } = useI18n();
  const t = dict.games.arcade;
  return (
    <section className="arc-sec">
      <h3>
        {t.albumTitle}{" "}
        <span className="num">
          {stubs.owned}/{stubs.total}
        </span>
      </h3>
      <div className="arc-album">
        {stubs.items.map((s) => (
          <div
            key={s.code}
            className={`arc-stub${s.obtained ? "" : " locked"}`}
            title={s.descr}
          >
            <div className={"ic" + (s.obtained ? "" : " mono")}>
              {s.obtained ? "🎟️" : (s.name || s.code).trim().slice(0, 1)}
            </div>
            <div className="nm">{s.name}</div>
            <div className="ct">
              {t.holders.replace("{n}", String(s.holders))}
            </div>
          </div>
        ))}
      </div>
    </section>
  );
}
