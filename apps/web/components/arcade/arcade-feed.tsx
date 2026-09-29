"use client";

/** 全服公示：谁点亮了票根 / 领了奖励（社交钩子）。
 *  事件由后端结构化下发（kind + name），文案在本组件用 i18n 合成 ——
 *  避免后端硬编码中文（后端纯函数拿不到站点语言）。 */

import { useI18n } from "@/i18n/client";

export interface ArcadeFeedItem {
  who: string;
  kind: string;
  name: string;
}

function text(
  f: ArcadeFeedItem,
  t: { feedStub: string; feedQuest: string; feedSeason: string },
): string {
  const tpl =
    f.kind === "stub"
      ? t.feedStub
      : f.kind === "quest"
        ? t.feedQuest
        : t.feedSeason;
  return tpl.replace("{who}", f.who).replace("{name}", f.name);
}

export function ArcadeFeed({ items }: { items: ArcadeFeedItem[] }) {
  const { dict } = useI18n();
  const t = dict.games.arcade;
  return (
    <section className="arc-sec">
      <h3>{t.feedTitle}</h3>
      <div className="arc-checks">
        {items.map((f, i) => (
          <div key={i} className="arc-chk">
            <span className="dot" />
            <span>{text(f, t)}</span>
          </div>
        ))}
        {items.length === 0 && (
          <div className="arc-note">{t.feedEmpty}</div>
        )}
      </div>
    </section>
  );
}
