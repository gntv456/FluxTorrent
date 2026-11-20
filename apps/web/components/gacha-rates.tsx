import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import type {
  GachaBannerRates,
  GachaRarityRow,
} from "@/app/(main)/gacha/_inner";

/**
 * G31-A 公示表（方案 §5）：概率双列——表定 base 与含保底综合 comp。
 * 综合列是红线口径：保底把概率从低档搬到高档，表定会系统性低估。
 * 稀有度色 token 全部来自后端行表（rarities），前端不持色表。
 * 2026-11-20：文案全部走 dict.review.*（此前中文硬编码）。
 */

const kindLabel = (kind: string, r: Record<string, string>) =>
  kind === "standard"
    ? r.gachaKindStandard
    : kind === "limited"
      ? r.gachaKindLimited
      : kind === "newbie"
        ? r.gachaKindNewbie
        : kind;

export function GachaRates({
  rarities,
  banners,
}: {
  rarities: GachaRarityRow[];
  banners: GachaBannerRates[];
}) {
  const { dict } = useI18n();
  const r = dict.review;
  if (banners.length === 0) {
    return <div className="card p-6 text-sm text-muted">{r.gachaNoBanner}</div>;
  }
  const pct = (x: number, d = 2) => `${(x * 100).toFixed(d)}%`;
  return (
    <div className="flex flex-col gap-6">
      {banners.map((b) => {
        const rmap = new Map(rarities.map((x) => [x.key, x]));

        return (
          <section key={b.banner.id} className="card overflow-hidden">
            <header className="flex flex-wrap items-baseline border-b p-4">
              <h2 className="font-display text-lg">{b.banner.name}</h2>
              <span className="text-xs text-muted">
                {kindLabel(b.banner.kind, r)}
              </span>
              <span className="ml-auto text-xs text-muted">
                {fmt(r.gachaTicketCost, { n: b.banner.ticketCost })}
              </span>
            </header>
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b text-left text-xs text-muted">
                    <th className="px-4 py-2">{r.gachaTier}</th>
                    <th className="px-4 py-2">{r.gachaBaseRate}</th>
                    <th className="px-4 py-2">{r.gachaCompRate}</th>
                    <th className="px-4 py-2">{r.gachaExpected}</th>
                  </tr>
                </thead>
                <tbody>
                  {b.rows.map((row) => {
                    const rr = rmap.get(row.r);
                    const color = rr?.frame ?? undefined;
                    return (
                      <tr key={row.r + row.type} className="border-b last:border-0">
                        <td className="px-4 py-2">
                          <span
                            className="font-semibold"
                            style={color ? { color } : undefined}
                          >
                            {row.r === "MISS"
                              ? r.gachaMiss
                              : row.r === "SHARD"
                                ? r.gachaShard
                                : (rr?.label ?? row.r)}
                          </span>
                        </td>
                        <td className="px-4 py-2 tabular-nums">{pct(row.base, 3)}</td>
                        <td className="px-4 py-2 tabular-nums font-semibold">
                          {pct(row.comp, 3)}
                        </td>
                        <td className="px-4 py-2 tabular-nums text-muted">
                          {row.type === "shard"
                            ? fmt(r.gachaShardsPlus, { n: row.shards })
                            : row.type === "miss"
                              ? "—"
                              : fmt(r.gachaValue, { n: row.value })}
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
            <footer className="flex flex-wrap gap-x-6 gap-y-1 p-4 text-xs text-muted">
              <span>
                {r.gachaGoldComp}{" "}
                <b className="text-base tabular-nums">{pct(b.goldComp)}</b>
                （{r.gachaBaseShort} {pct(b.goldBase)}）
              </span>
              <span>{fmt(r.gachaPityCycle, { n: b.cycle.toFixed(1) })}</span>
              <span>
                {r.gachaHardProb} {pct(b.hardProb, 3)}
              </span>
              <span>
                {r.gachaWorstRatio} {pct(b.ratioWorst, 1)}
              </span>
            </footer>
          </section>
        );
      })}
      <p className="text-xs text-muted">{r.gachaRateNote}</p>
    </div>
  );
}
