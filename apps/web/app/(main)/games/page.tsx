import Link from "next/link";
import { api } from "@/lib/api-client";
import { getDict } from "@/i18n/server";
import { getSiteProfile } from "@/lib/site-profile";
import { requireModule } from "@/components/module-gate";
import { FunBox } from "@/components/fun-box";
import { fmtMult, scratchPoolText, type ScratchPrize } from "@/lib/games";
import {
  ArcadeMeta,
  type ArcadeMeta as ArcadeMetaData,
} from "@/components/arcade/arcade-meta";

interface HallOverview {
  scratch?: { prizes: ScratchPrize[] };
  jgg?: { prizes: { payout: number }[] };
  bigsmall?: { win_mult: number };
}

/**
 * 娱乐屋大厅：只做「选哪个玩」，不在这里直接下注 ——
 * 降低误触与冲动下注，也让每个玩法有自己的舞台（专注页）。
 */
export default async function GamesPage() {
  // U1 模块页守卫：关闭时渲染统一空态
  const gate = await requireModule("games");
  if (gate) return gate;

  const { dict, currency } = await getDict();
  const profile = await getSiteProfile();
  // 模块开关口径与 requireModule / 页脚一致：缺键视为开
  const mod = (k: string) => profile.modules[k] !== false;
  const cardMod: Record<string, string> = { "/games/farm": "farm" };
  // 卡片角标取自后端下发的真实赔率/奖池（不在前端写死，避免展示与实现不符）
  const ov = await api.get<HallOverview>("/api/v1/games").catch(() => null);
  const scratchTop = ov?.scratch?.prizes?.length
    ? Math.max(...ov.scratch.prizes.map((p) => p.multiplier))
    : 10;
  const jggTop = ov?.jgg?.prizes?.length
    ? Math.max(...ov.jgg.prizes.map((p) => p.payout))
    : 50;
  const winMult = ov?.bigsmall?.win_mult ?? 1.9;
  // 大厅游戏表面（真数据；未登录/失败静默隐藏——不阻塞大厅选玩）
  const arcadeMeta = await api
    .get<ArcadeMetaData>("/api/v1/games/arcade-meta")
    .catch(() => null);

  const cards = [
    {
      href: "/games/scratch",
      icon: "🎫",
      title: dict.games.scratch.title,
      sub: scratchPoolText(ov?.scratch?.prizes, dict.games.scratch.poolLabel),
      tag: `${fmtMult(scratchTop)}x`,
      bg: "linear-gradient(135deg,var(--sun-soft),var(--coral-soft))",
      tagCls: "bg-sun-soft text-[var(--warning)]",
    },
    {
      href: "/games/bigsmall",
      icon: "🎯",
      title: dict.games.bigsmall.title,
      sub: dict.games.bigsmall.rule,
      tag: dict.games.bigsmall.winMult.replace("{n}", fmtMult(winMult)),
      bg: "linear-gradient(135deg,var(--sky-soft),var(--indigo-soft))",
      tagCls: "bg-sky-soft text-[var(--sky-deep)]",
    },
    {
      href: "/games/jgg",
      icon: "🎰",
      title: dict.games.jgg.title,
      sub: dict.games.jgg.ticket,
      tag: `${jggTop}x`,
      bg: "linear-gradient(135deg,var(--candy-soft),var(--sun-soft))",
      tagCls: "bg-candy-soft text-[var(--candy)]",
    },
    {
      href: "/games/farm",
      icon: "🌾",
      title: dict.games.farmName.replace("{magic}", currency),
      sub: dict.games.farmSub.replace("{magic}", currency),
      tag: dict.games.hallSlow,
      bg: "linear-gradient(135deg,var(--mint-soft),var(--sun-soft))",
      tagCls: "bg-mint-soft text-[var(--mint)]",
    },
  ].filter((c) => mod(cardMod[c.href] ?? "games"));

  return (
    <div className="flex flex-col gap-4">
      <div className="pghd">
        <div>
          <div className="pg-eyebrow">Arcade</div>
          <h1 className="font-display text-2xl">{dict.games.title}</h1>
        </div>
        <span className="sub">
          {dict.games.subtitle.replace("{magic}", currency)}
        </span>
      </div>

      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
        {cards.map((c) => (
          <Link
            key={c.href}
            href={c.href}
            className="group overflow-hidden rounded-[var(--r-lg)] border border-line bg-[var(--surface-card)] shadow-[var(--shadow-card)] transition-transform hover:-translate-y-0.5 hover:shadow-[var(--shadow-hover)]"
          >
            <div
              className="flex h-[104px] items-center justify-center text-[44px]"
              style={{ background: c.bg }}
              aria-hidden
            >
              {c.icon}
            </div>
            <div className="flex flex-col gap-1 p-4">
              <div className="flex items-center justify-between gap-2">
                <h2 className="font-display text-lg">{c.title}</h2>
                <span className={`rounded-full px-2 py-0.5 text-[11px] font-bold ${c.tagCls}`}>
                  {c.tag}
                </span>
              </div>
              <p className="text-xs text-sub">{c.sub}</p>
              <span className="mt-1 text-xs font-bold text-[var(--sky-deep)]">
                {dict.games.enter} →
              </span>
            </div>
          </Link>
        ))}
      </div>

      {arcadeMeta && <ArcadeMeta initial={arcadeMeta} />}

      <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-xs text-ink">
        {dict.games.rule.replace("{magic}", currency)}
      </p>
      <p className="text-xs text-sub">{dict.games.hallHint}</p>

      <Link href="/games/odds" className="text-xs font-bold text-[var(--sky-deep)]">
        {dict.games.odds.title} →
      </Link>

      {/* 趣味盒（旧站 fun.php 口径）仍挂在大厅底部 —— 它不是「下注玩法」，归在娱乐屋内容区 */}
      <FunBox embedded />
    </div>
  );
}
