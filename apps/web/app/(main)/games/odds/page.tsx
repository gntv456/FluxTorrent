import Link from "next/link";
import { api } from "@/lib/api-client";
import { getDict } from "@/i18n/server";
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
}
interface Overview {
  max_plays_per_hour?: number;
  scratch?: {
    prizes: { multiplier: number; pct: number }[];
    empty_pct: number;
    expected_value: number;
  };
  bigsmall?: { win_mult: number; expected_value: number; rule: string };
  jgg?: { ticket: number; prizes: Prize[]; expected_value: number };
  farm?: { slots: number; market_refresh: string; volatility: string };
  rate_limit?: string;
}

/** 千分比 / 百分数统一两位小数，避免同一页两种精度 */
const pct = (n: number) => `${n.toFixed(2)}%`;
const edge = (ev: number) => pct((1 - ev) * 100);

/**
 * 概率公示（L3 展示层）：每个奖项的概率、等值倍数与整桌期望回报。
 *
 * 数字一律取服务端下发 —— `expected_value` 与写侧闸门共用同一份算式
 * （jgg 走 pool_ev，刮刮乐走 ScratchOdds::ev），所以「公示的 EV」和
 * 「拦住坏配置的那个 EV」不可能是两个值。前端不重算，也就没有第二份公式。
 */
export default async function OddsPage() {
  const gate = await requireModule("games");
  if (gate) return gate;
  const { dict, currency } = await getDict();
  const t = dict.games.odds;
  const ov = await api
    .get<Overview>("/api/v1/games")
    .catch(() => null);

  const rows: {
    game: string;
    ev: number | null;
    body: React.ReactNode;
  }[] = [];

  if (ov?.jgg?.prizes?.length) {
    const tk = ov.jgg.ticket;
    rows.push({
      game: dict.games.jgg.title,
      ev: ov.jgg.expected_value,
      body: (
        <table className="w-full text-xs">
          <thead>
            <tr className="text-left text-sub">
              <th className="py-1">{t.colPrize}</th>
              <th className="py-1 text-right">{t.colChance}</th>
              <th className="py-1 text-right">{t.colValue}</th>
            </tr>
          </thead>
          <tbody>
            {ov.jgg.prizes.map((p, i) => (
              <tr key={i} className="border-t border-line">
                <td className="py-1.5">
                  {p.label}
                  {p.kind === "item" && (
                    <span className="ml-1 text-[10px] text-sub">
                      ×{p.qty ?? 1}
                    </span>
                  )}
                </td>
                <td className="num py-1.5 text-right">
                  {pct(p.weight_permille / 10)}
                </td>
                <td className="num py-1.5 text-right">
                  {p.kind === "item"
                    ? `${p.value.toLocaleString("en-US")} ${currency} · ${pct(
                        (p.value / tk) * 100,
                      )}`
                    : `×${(p.value / tk).toFixed(2)}`}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      ),
    });
  }

  if (ov?.scratch?.prizes?.length) {
    rows.push({
      game: dict.games.scratch.title,
      ev: ov.scratch.expected_value,
      body: (
        <table className="w-full text-xs">
          <thead>
            <tr className="text-left text-sub">
              <th className="py-1">{t.colPrize}</th>
              <th className="py-1 text-right">{t.colChance}</th>
            </tr>
          </thead>
          <tbody>
            {[
              {
                label: dict.games.scratch.thanks,
                chance: ov.scratch.empty_pct,
              },
              ...ov.scratch.prizes.map((p) => ({
                label: `×${p.multiplier}`,
                chance: p.pct,
              })),
            ].map((r, i) => (
              <tr key={i} className="border-t border-line">
                <td className="py-1.5">{r.label}</td>
                <td className="num py-1.5 text-right">{pct(r.chance)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      ),
    });
  }

  if (ov?.bigsmall) {
    rows.push({
      game: dict.games.bigsmall.title,
      ev: ov.bigsmall.expected_value,
      body: <p className="text-xs text-sub">{ov.bigsmall.rule}</p>,
    });
  }

  if (ov?.farm) {
    rows.push({
      game: dict.games.farmName.replace("{magic}", currency),
      ev: null,
      body: (
        <p className="text-xs text-sub">
          {ov.farm.slots} · {ov.farm.market_refresh} · {ov.farm.volatility}
        </p>
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
