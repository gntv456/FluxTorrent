import Link from "next/link";
import { api } from "@/lib/api-client";
import { getDict } from "@/i18n/server";
import { getSiteProfile } from "@/lib/site-profile";
import { requireModule } from "@/components/module-gate";
import { fmtMult, scratchPoolText, type ScratchPrize } from "@/lib/games";
import { GAMES, type GameEntry, type GameGroup } from "@/lib/games-registry";
import {
  ArcadeMeta,
  type ArcadeMeta as ArcadeMetaData,
} from "@/components/arcade/arcade-meta";

interface HallOverview {
  scratch?: { ticket?: number; prizes: ScratchPrize[] };
  jgg?: { ticket: number; prizes: { payout: number; value?: number }[] };
  bigsmall?: { win_mult: number };
  farm?: { unit?: number; prizes?: { payout: number; value?: number }[] };
  capsule?: { ticket: number; prizes: { payout: number; value?: number }[] };
  wheel?: { ticket: number; prizes: { payout: number; value?: number }[] };
  fishing?: { ticket: number; prizes: { payout: number; value?: number }[] };
  /** 后端下发的大厅清单（arcade_games）；空则用前端注册表兜底 */
  registry?: RegRow[];
}

/** 后端清单行：展示字段，与 lib/games-registry 的 GameEntry 一一对应 */
interface RegRow {
  key: string;
  title_key: string;
  icon: string;
  href: string;
  tone: string;
  grp: string;
  module: string;
  badge: string;
  hot: boolean;
}

function toEntry(r: RegRow): GameEntry {
  return {
    key: r.key,
    href: r.href,
    icon: r.icon,
    tone: r.tone as GameEntry["tone"],
    group: r.grp as GameEntry["group"],
    module: r.module || undefined,
    live: true, // 查询已过滤 enabled
    badge: (r.badge || undefined) as GameEntry["badge"],
    hot: r.hot,
  };
}

/**
 * 娱乐屋大厅：只做「选哪个玩」，不在这里直接下注 ——
 * 降低误触与冲动下注，也让每个玩法有自己的舞台（专注页）。
 *
 * 卡片来自 `lib/games-registry`（单一来源）：新增玩法零改动本页。
 */
export default async function GamesPage() {
  // U1 模块页守卫：关闭时渲染统一空态
  const gate = await requireModule("games");
  if (gate) return gate;

  const { dict, currency } = await getDict();
  const profile = await getSiteProfile();
  // 模块开关口径与 requireModule / 页脚一致：缺键视为开
  const mod = (k: string) => profile.modules[k] !== false;
  const ov = await api.get<HallOverview>("/api/v1/games").catch(() => null);

  /** 角标要的是「最高等值」而不是「最高魔力倍数」：0245 之后物品位 payout 恒为 0，
   *  按 payout 排会把 120 倍等值的免考核卡整个漏掉。三个玩法同一把尺，只留一份算法。 */
  const topOf = (
    prizes: { payout: number; value?: number }[] | undefined,
    tk: number,
    none: number,
  ) =>
    prizes?.length
      ? Math.max(...prizes.map((p) => (p.value ?? p.payout * tk) / tk))
      : none;
  const scratchTop = topOf(ov?.scratch?.prizes, ov?.scratch?.ticket ?? 1, 10);
  const jggTop = topOf(ov?.jgg?.prizes, ov?.jgg?.ticket ?? 100, 50);
  const capsuleTop = topOf(ov?.capsule?.prizes, ov?.capsule?.ticket ?? 100, 0);
  const wheelTop = topOf(ov?.wheel?.prizes, ov?.wheel?.ticket ?? 100, 0);
  const fishingTop = topOf(ov?.fishing?.prizes, ov?.fishing?.ticket ?? 100, 0);
  const farmTop = topOf(ov?.farm?.prizes, ov?.farm?.unit ?? 1, 0);
  const winMult = ov?.bigsmall?.win_mult ?? 1.9;

  /** 卡面角标：取自后端下发的真实赔率/奖池，不在前端写死。 */
  const badgeOf = (e: GameEntry): string | null => {
    switch (e.badge) {
      case "bigsmall":
        return dict.games.bigsmall.winMult.replace("{n}", fmtMult(winMult));
      case "scratch":
        return `${fmtMult(scratchTop)}x`;
      case "jgg":
        return `${fmtMult(jggTop)}x`;
      case "capsule":
        return capsuleTop > 0 ? `${fmtMult(capsuleTop)}x` : null;
      case "wheel":
        return wheelTop > 0 ? `${fmtMult(wheelTop)}x` : null;
      case "fishing":
        return fishingTop > 0 ? `${fmtMult(fishingTop)}x` : null;
      case "farm":
        return farmTop > 0 ? `${fmtMult(farmTop)}x` : dict.games.hallSlow;
      default:
        return null;
    }
  };
  const cards = dict.games.cards as Record<
    string,
    { title: string; sub: string }
  >;
  // 卡片副标题里的 {magic} 占位符按站点货币名替换
  const subOf = (e: GameEntry) =>
    (cards[e.key]?.sub ?? "").replace("{magic}", currency);

  // 清单来源：DB 优先（站长可调序/隐藏），空则用前端注册表兜底
  const regRows = ov?.registry ?? [];
  const list: GameEntry[] =
    regRows.length > 0 ? regRows.map(toEntry) : GAMES;
  const visible = list.filter((e) => mod(e.module ?? "games"));
  const liveCount = visible.filter((e) => e.live).length;
  const groups: { key: GameGroup; label: string }[] = [
    { key: "instant", label: dict.games.hall.groupInstant },
    { key: "session", label: dict.games.hall.groupSession },
  ];

  const arcadeMeta = await api
    .get<ArcadeMetaData>("/api/v1/games/arcade-meta")
    .catch(() => null);

  return (
    <div className="flex flex-col gap-5">
      <section className="gc-hero">
        <div className="gc-hero-main">
          <div className="pg-eyebrow">Arcade</div>
          <h1 className="font-display">{dict.games.title}</h1>
          <p className="gc-hero-sub">
            {dict.games.subtitle.replace("{magic}", currency)}
          </p>
        </div>
        <div className="gc-hero-stat">
          <b className="num">{liveCount}</b>
          <span>
            {dict.games.hall.count.replace("{n}", String(visible.length))}
          </span>
        </div>
        <span className="gc-hero-orb" aria-hidden />
      </section>

      {groups.map((g) => {
        const items = visible.filter((e) => e.group === g.key);
        if (!items.length) return null;
        return (
          <section key={g.key} className="flex flex-col gap-3">
            <div className="gc-group-hd">
              <h2>{g.label}</h2>
              <span className="gc-group-line" aria-hidden />
              <span className="gc-group-n">{items.length}</span>
            </div>
            <div className="gc-grid">
              {items.map((e) => {
                const badge = badgeOf(e);
                const title = cards[e.key]?.title ?? e.key;
                const inner = (
                  <>
                    <div className={`gc-art gc-tone-${e.tone}`}>
                      <span className="gc-art-grid" aria-hidden />
                      <span className="gc-glyph" aria-hidden>
                        {e.icon}
                      </span>
                      {e.hot && e.live && (
                        <span className="gc-hot">{dict.games.hall.hot}</span>
                      )}
                      {badge && e.live && (
                        <span className="gc-badge num">{badge}</span>
                      )}
                      {!e.live && (
                        <span className="gc-soon">
                          {dict.games.hall.soon}
                        </span>
                      )}
                    </div>
                    <div className="gc-body">
                      <h3 className="gc-title">{title}</h3>
                      <p className="gc-sub">{subOf(e)}</p>
                      <span className="gc-go">
                        {dict.games.hall.play}
                        <i aria-hidden>→</i>
                      </span>
                    </div>
                  </>
                );
                return e.live ? (
                  <Link
                    key={e.key}
                    href={e.href}
                    className="gc-card"
                    aria-label={title}
                  >
                    {inner}
                  </Link>
                ) : (
                  <div
                    key={e.key}
                    className="gc-card is-soon"
                    aria-disabled="true"
                  >
                    {inner}
                  </div>
                );
              })}
            </div>
          </section>
        );
      })}

      {arcadeMeta && <ArcadeMeta initial={arcadeMeta} />}

      <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-xs text-ink">
        {scratchPoolText(ov?.scratch?.prizes, "") ||
          dict.games.rule.replace("{magic}", currency)}
      </p>
      <p className="text-xs text-sub">{dict.games.hallHint}</p>

      <div className="flex flex-wrap gap-x-5 gap-y-2">
        <Link
          href="/games/odds"
          className="text-xs font-bold text-[var(--sky-deep)]"
        >
          {dict.games.odds.title} →
        </Link>
        {/* 趣味盒（旧站 fun.php 口径）已移出大厅游戏网格：它不是下注玩法 */}
        <Link
          href="/games/fun"
          className="text-xs font-bold text-[var(--sky-deep)]"
        >
          {dict.games.hall.funBoxEntry} →
        </Link>
      </div>
    </div>
  );
}
