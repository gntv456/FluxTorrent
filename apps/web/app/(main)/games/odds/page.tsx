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
            {ov.jgg.prizes.map((p, i) => {
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
                  {/* 等值一律用「票价倍数」同一把尺；物品位再补一个绝对折算价 */}
                  <td className="num py-1.5 text-right">
                    ×{(p.value / tk).toFixed(2)}
                    {p.kind === "item" && (
                      <span className="text-sub">
                        {" "}
                        （{p.value.toLocaleString("en-US")} {currency}）
                      </span>
                    )}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      ),
    });
  }

  if (ov?.scratch?.prizes?.length) {
    const stk = ov.scratch.ticket ?? 1;
    rows.push({
      game: dict.games.scratch.title,
      ev: ov.scratch.expected_value,
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
            {/* 与九宫格同一份投影：档位、概率、等值都来自奖池行表现值，
                包括倍率为 0 的「未中奖」档，不再由前端拼一行假数据 */}
            {ov.scratch.prizes.map((p, i) => (
              <tr key={i} className="border-t border-line">
                <td className="py-1.5">{p.label}</td>
                <td className="num py-1.5 text-right">
                  {pct(p.weight_permille / 10)}
                </td>
                <td className="num py-1.5 text-right">
                  ×{(p.value / stk).toFixed(2)}
                  {p.kind === "item" && (
                    <span className="text-sub">
                      {" "}
                      （{p.value.toLocaleString("en-US")} {currency}）
                    </span>
                  )}
                </td>
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
