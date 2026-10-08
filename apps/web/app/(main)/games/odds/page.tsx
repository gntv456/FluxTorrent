import Link from "next/link";
import { api } from "@/lib/api-client";
import { getDict } from "@/i18n/server";
import { dateLocale } from "@/i18n/config";
import { requireModule } from "@/components/module-gate";
import { PANEL_LG } from "@/lib/ui-classes";

interface Prize {
  label: string;
  weight_permille: number;
  payout: number;
  value: number;
  kind?: string;
  qty?: number;
  anchor?: number;
  use_kind?: string;
  use_name?: string | null;
}
interface Overview {
  max_plays_per_hour?: number;
  scratch?: {
    /** 与九宫格同一份服务端投影，前端不再自己拼档位 */
    prizes: Prize[];
    ticket?: number;
    empty_pct: number;
    expected_value: number;
  };
  bigsmall?: {
    win_mult: number;
    expected_value: number;
    rule: string;
    ticket?: number;
    prizes?: Prize[];
  };
  jgg?: { ticket: number; prizes: Prize[]; expected_value: number };
  farm?: {
    slots: number;
    market_refresh: string;
    volatility: string;
    /** 彩蛋档的定标单位 = 最便宜作物的种子价 */
    unit: number;
    prizes?: Prize[];
    expected_value: number;
  };
  rate_limit?: string;
}

/** 千分比 / 百分数统一两位小数，避免同一页两种精度 */
const pct = (n: number) => `${n.toFixed(2)}%`;
const edge = (ev: number) => pct((1 - ev) * 100);

/**
 * 概率公示（L3 展示层）：每个奖项的概率、等值倍数与整桌期望回报。
 *
 * 数字一律取服务端下发 —— `expected_value` 与写侧闸门共用同一份算式
 * （四个玩法都走 pool_ev，农场再叠加确定性收获那 0.90），所以「公示的 EV」和
 * 「拦住坏配置的那个 EV」不可能是两个值。前端不重算，也就没有第二份公式。
 */
export default async function OddsPage() {
  const gate = await requireModule("games");
  if (gate) return gate;
  const { dict, locale, currency } = await getDict();
  const t = dict.games.odds;
  const ov = await api
    .get<Overview>("/api/v1/games")
    .catch(() => null);

  const rows: {
    game: string;
    ev: number | null;
    body: React.ReactNode;
  }[] = [];

  /** 档位表：四个玩法读同一张奖池行表，公示也只留一份实现（三份必然漂移）。
   *  `unit` 是这一池的定标单位 —— 票价，或农场的最便宜种子价。 */
  const prizeTable = (prizes: Prize[], unit: number) => (
    <table className="w-full text-xs">
      <thead>
        <tr className="text-left text-sub">
          <th className="py-1">{t.colPrize}</th>
          <th className="py-1 text-right">{t.colChance}</th>
          <th className="py-1 text-right">{t.colValue}</th>
        </tr>
      </thead>
      <tbody>
        {prizes.map((p, i) => {
          // 抽到之后拿到什么：绑 SKU 的报出实际授予物，兑现的报出口径，
          // 收藏件也要说明——公示不该只报概率让人自己猜价值。
          const hint =
            p.kind !== "item"
              ? null
              : p.use_kind === "sku" && p.use_name
                ? t.useSku.replace("{s}", p.use_name)
                : p.use_kind === "spark"
                  ? t.useSpark
                  : t.useCollect;
          return (
            <tr key={i} className="border-t border-line">
              <td className="py-1.5">
                {p.label}
                {hint && (
                  <span className="block text-[11px] text-sub">
                    {hint}
                  </span>
                )}
              </td>
              <td className="num py-1.5 text-right">
                {pct(p.weight_permille / 10)}
              </td>
              {/* 等值一律用「定标单位倍数」同一把尺；物品位再补绝对折算价 */}
              <td className="num py-1.5 text-right">
                ×{(p.value / (unit || 1)).toFixed(2)}
                {p.kind === "item" && (
                  <span className="text-sub">
                    {" "}
                    （{p.value.toLocaleString(dateLocale(locale))} {currency}）
                  </span>
                )}
              </td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );

  if (ov?.jgg?.prizes?.length) {
    rows.push({
      game: dict.games.jgg.title,
      ev: ov.jgg.expected_value,
      body: prizeTable(ov.jgg.prizes, ov.jgg.ticket),
    });
  }

  if (ov?.scratch?.prizes?.length) {
    rows.push({
      game: dict.games.scratch.title,
      ev: ov.scratch.expected_value,
      // 与九宫格同一份投影：包括倍率为 0 的「未中奖」档，
      // 不由前端拼一行假数据
      body: prizeTable(ov.scratch.prizes, ov.scratch.ticket ?? 1),
    });
  }

  if (ov?.bigsmall) {
    rows.push({
      game: dict.games.bigsmall.title,
      ev: ov.bigsmall.expected_value,
      body: (
        <div className="flex flex-col gap-2">
          <p className="text-xs text-sub">{ov.bigsmall.rule}</p>
          {/* 赔率、平局返本、以及「猜中给一件东西」都在行表里，公示照表念 */}
          {!!ov.bigsmall.prizes?.length &&
            prizeTable(ov.bigsmall.prizes, ov.bigsmall.ticket ?? 1)}
        </div>
      ),
    });
  }

  if (ov?.farm) {
    rows.push({
      game: dict.games.farmName.replace("{magic}", currency),
      // 农场报的是**总**回收：确定性收获 0.90 + 彩蛋那一注，服务端一次算好
      ev: ov.farm.expected_value,
      body: (
        <div className="flex flex-col gap-2">
          <p className="text-xs text-sub">
            {ov.farm.slots} · {ov.farm.market_refresh} ·{" "}
            {ov.farm.volatility}
          </p>
          {!!ov.farm.prizes?.length && (
            <>
              <p className="text-xs text-sub">
                {t.farmEgg
                  .replace("{n}", String(ov.farm.unit))
                  .replace("{magic}", currency)}
              </p>
              {prizeTable(ov.farm.prizes, ov.farm.unit)}
            </>
          )}
        </div>
      ),
    });
  }

  return (
    <div className="flex flex-col gap-4">
      <div className="pghd">
        <div>
          <div className="pg-eyebrow">Arcade</div>
          <h1 className="font-display text-2xl">{t.title}</h1>
        </div>
        <Link href="/games" className="text-xs text-[var(--sky-deep)]">
          ← {dict.games.title}
        </Link>
      </div>

      <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-xs text-ink">
        {t.note}
      </p>

      {rows.map((r) => (
        <section key={r.game} className={PANEL_LG}>
          <div className="mb-2 flex items-baseline justify-between gap-3">
            <h2 className="font-display text-base">{r.game}</h2>
            {r.ev !== null && (
              <div className="num text-xs text-sub">
                {t.ev} {r.ev.toFixed(3)} · {t.house} {edge(r.ev)}
              </div>
            )}
          </div>
          {r.body}
        </section>
      ))}

      {!rows.length && (
        <p className="text-sm text-sub">{dict.common.networkError}</p>
      )}

      {ov?.rate_limit && (
        <p className="text-xs text-sub">
          {t.limit} · {ov.rate_limit}
        </p>
      )}
    </div>
  );
}
