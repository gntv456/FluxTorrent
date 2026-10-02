import type {
  GachaBannerRates,
  GachaRarityRow,
} from "@/app/(main)/gacha/_inner";

/**
 * G31-A 公示表（方案 §5）：概率双列——表定 base 与含保底综合 comp。
 * 综合列是红线口径：保底把概率从低档搬到高档，表定会系统性低估
 * （样张实测同一池表定 91.6% / 综合 105.8% 的返还差）。
 * 稀有度色 token 全部来自后端行表（rarities），前端不持色表（样张判据）。
 */

/** 池类型（gacha_banners.kind，CHECK 固定三种）→ 人话；未知回落原值。
 *  本组件其余文案（档位/表定概率等）也是中文硬编码，口径一致即可。 */
const BANNER_KIND: Record<string, string> = {
  standard: "常驻池",
  limited: "限定池",
  newbie: "新手池",
};

export function GachaRates({
  rarities,
  banners,
}: {
  rarities: GachaRarityRow[];
  banners: GachaBannerRates[];
}) {
  if (banners.length === 0) {
    return (
      <div className="card p-6 text-sm text-muted">当前没有开放中的卡池。</div>
    );
  }
  const pct = (x: number, d = 2) => `${(x * 100).toFixed(d)}%`;
  return (
    <div className="flex flex-col gap-6">
      {banners.map((b) => {
        const rmap = new Map(rarities.map((r) => [r.key, r]));
        return (
          <section key={b.banner.id} className="card overflow-hidden">
            <header className="flex flex-wrap items-baseline border-b p-4">
              <h2 className="font-display text-lg">{b.banner.name}</h2>
              <span className="text-xs text-muted">
                {BANNER_KIND[b.banner.kind] ?? b.banner.kind}
              </span>
              <span className="ml-auto text-xs text-muted">
                单抽票价 {b.banner.ticketCost}
              </span>
            </header>
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b text-left text-xs text-muted">
                    <th className="px-4 py-2">档位</th>
                    <th className="px-4 py-2">表定概率</th>
                    <th className="px-4 py-2">综合概率（含保底）</th>
                    <th className="px-4 py-2">期望产出</th>
                  </tr>
                </thead>
                <tbody>
                  {b.rows.map((row) => {
                    const r = rmap.get(row.r);
                    const color = r?.frame ?? undefined;
                    return (
                      <tr
                        key={row.r + row.type}
                        className="border-b last:border-0"
                      >
                        <td className="px-4 py-2">
                          <span
                            className="font-semibold"
                            style={color ? { color } : undefined}
                          >
                            {row.r === "MISS"
                              ? "谢谢惠顾"
                              : row.r === "SHARD"
                                ? "碎片"
                                : (r?.label ?? row.r)}
                          </span>
                        </td>
                        <td className="px-4 py-2 tabular-nums">
                          {pct(row.base, 3)}
                        </td>
                        <td className="px-4 py-2 tabular-nums font-semibold">
                          {pct(row.comp, 3)}
                        </td>
                        <td className="px-4 py-2 tabular-nums text-muted">
                          {row.type === "shard"
                            ? `+${row.shards} 碎片`
                            : row.type === "miss"
                              ? "—"
                              : `价值 ${row.value}`}
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
            <footer className="flex flex-wrap gap-x-6 gap-y-1 p-4 text-xs text-muted">
              <span>
                综合出金率{" "}
                <b className="text-base tabular-nums">{pct(b.goldComp)}</b>
                （表定 {pct(b.goldBase)}）
              </span>
              <span>保底周期 ≈ {b.cycle.toFixed(1)} 抽</span>
              <span>硬保底触发概率 {pct(b.hardProb, 3)}</span>
              <span>返还率（最差口径）{pct(b.ratioWorst, 1)}</span>
            </footer>
          </section>
        );
      })}
      <p className="text-xs text-muted">
        「综合概率」含保底机制：随着未出金抂数增加，出金概率逐步提升，
        因此综合概率高于表定概率。返还率 = 每抽期望产出价值 ÷ 票价，
        最差口径取新手（新卡按全价）与毕业（重复卡按折算）中较高者。
      </p>
    </div>
  );
}
