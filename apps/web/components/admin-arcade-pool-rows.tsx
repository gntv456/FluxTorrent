"use client";

/**
 * 奖池档位表格（编辑器的一半：只管「这一行填什么」）。
 *
 * 从 admin-arcade-pool 拆出：壳子管读数、保存与回读，这一件管行的形状。
 * 猜大小多一列「哪一区付」，因为它的档位必须说明在赢/豹/输哪一侧生效；
 * 另两个玩法不显示这一列（落库为 any）。
 */

import { useI18n } from "@/i18n/client";

export interface PoolRowView {
  label: string;
  weight_permille: number;
  payout: number;
  kind?: string;
  item_key?: string;
  qty?: number;
  side?: string;
  /** 展示用稀有度 1..5（不参与 EV） */
  rarity?: number;
  /** 展示用配图 URL */
  image_url?: string;
}

export interface CatalogItem {
  key: string;
  name: string;
  icon: string;
  anchor: number;
}

const CELL =
  "w-full rounded-[var(--r-sm)] border border-line " +
  "bg-[var(--surface-card)] px-2 py-1 text-xs";
const ACT = "rounded border border-line px-1.5 py-0.5";

export function AdminArcadePoolRows({
  rows,
  catalog,
  totalWeight,
  showSide,
  valueColLabel,
  onSet,
  onMove,
  onDel,
}: {
  rows: PoolRowView[];
  catalog: CatalogItem[];
  totalWeight: number;
  showSide: boolean;
  /** 倍数那一列的尺子随玩法变（票价 / 农场种子价），故由壳子传进来 */
  valueColLabel?: string;
  onSet: (i: number, patch: Partial<PoolRowView>) => void;
  onMove: (i: number, dir: -1 | 1) => void;
  onDel: (i: number) => void;
}) {
  const { dict } = useI18n();
  const t = dict.adminArcade.pool;
  const pct = (weight: number) =>
    totalWeight > 0
      ? `${((weight / totalWeight) * 100).toFixed(1)}%`
      : "—";
  return (
      <table className="nexus-table w-full text-xs">
        <thead>
          <tr className="text-left text-sub">
            <th className="py-1">{t.colPrize}</th>
            <th className="py-1">{t.colWeight}</th>
            <th className="py-1">{t.colProb}</th>
            <th className="py-1">{t.colKind}</th>
            {showSide && (
              <th className="py-1">{t.colSide}</th>
            )}
            <th className="py-1 text-right">
              {valueColLabel ?? t.colValue}
            </th>
            <th className="py-1">{t.colRarity}</th>
            <th className="py-1">{t.colImage}</th>
            <th className="py-1">{t.colActions}</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((r, i) => (
            <tr key={i} className="border-t border-line">
              <td className="py-1 pr-2">
                <input
                  className={CELL}
                  value={r.label}
                  onChange={(e) => onSet(i, { label: e.target.value })}
                />
              </td>
              <td className="py-1 pr-2">
                <input
                  type="number"
                  min={0}
                  className={CELL}
                  value={r.weight_permille}
                  onChange={(e) =>
                    onSet(i, { weight_permille: Number(e.target.value) || 0 })
                  }
                />
              </td>
              <td className="py-1 pr-2 text-right num text-sub">
                {pct(r.weight_permille)}
              </td>
              <td className="py-1 pr-1">
                <select
                  className={CELL + " w-24"}
                  value={r.kind ?? "magic"}
                  onChange={(e) => onSet(i, { kind: e.target.value })}
                >
                  <option value="magic">{t.kindMagic}</option>
                  <option value="item">{t.kindItem}</option>
                </select>
              </td>
              {showSide && (
                <td className="py-1 pr-1">
                  {/* 猜大小的一档必须说明它在哪一区付：三区合起来才是 486/28/486。
                      取值域是 win/triple/lose（0265 三骰豹子）；历史值 tie 读到时
                      当 triple 显示，避免旧数据在下拉里「消失」被保存抹区 */}
                  <select
                    className={CELL + " w-20"}
                    value={r.side === "tie" ? "triple" : (r.side ?? "any")}
                    onChange={(e) => onSet(i, { side: e.target.value })}
                  >
                    <option value="win">{t.sideWin}</option>
                    <option value="triple">{t.sideTriple}</option>
                    <option value="lose">{t.sideLose}</option>
                  </select>
                </td>
              )}
              <td className="py-1 text-right">
                {r.kind === "item" ? (
                  <span className="text-sub">
                    <select
                      className={CELL + " inline-block w-40"}
                      value={r.item_key ?? ""}
                      onChange={(e) => {
                        const hit = catalog.find(
                          (c) => c.key === e.target.value,
                        );
                        onSet(i, {
                          item_key: e.target.value,
                          label: hit ? hit.name : r.label,
                        });
                      }}
                    >
                      {catalog.map((c) => (
                        <option key={c.key} value={c.key}>
                          {c.icon} {c.name}
                        </option>
                      ))}
                    </select>
                    ×
                    <input
                      type="number"
                      min={1}
                      className={`${CELL} ml-1 inline-block w-16 text-right`}
                      value={r.qty ?? 1}
                      onChange={(e) =>
                        onSet(i, { qty: Number(e.target.value) || 1 })
                      }
                    />
                  </span>
                ) : (
                  <input
                    type="number"
                    min={0}
                    className={`${CELL} w-20 text-right`}
                    value={r.payout}
                    onChange={(e) =>
                      onSet(i, { payout: Number(e.target.value) || 0 })
                    }
                  />
                )}
              </td>
              <td className="py-1 pr-1">
                {/* 稀有度：揭晓时的配色/星级（纯展示，不参与 EV） */}
                <select
                  className={CELL + " w-16"}
                  value={r.rarity ?? 1}
                  onChange={(e) =>
                    onSet(i, { rarity: Number(e.target.value) })
                  }
                >
                  {[1, 2, 3, 4, 5].map((n) => (
                    <option key={n} value={n}>
                      {n}
                    </option>
                  ))}
                </select>
              </td>
              <td className="py-1">
                <input
                  className={CELL + " w-40"}
                  placeholder="https://…"
                  value={r.image_url ?? ""}
                  onChange={(e) =>
                    onSet(i, { image_url: e.target.value })
                  }
                />
              </td>
              <td className="py-1">
                <div className="flex items-center gap-1">
                  <button
                    type="button"
                    className={ACT + " disabled:opacity-40"}
                    title={t.moveUp}
                    aria-label={t.moveUp}
                    disabled={i === 0}
                    onClick={() => onMove(i, -1)}
                  >
                    ↑
                  </button>
                  <button
                    type="button"
                    className={ACT + " disabled:opacity-40"}
                    title={t.moveDown}
                    aria-label={t.moveDown}
                    disabled={i === rows.length - 1}
                    onClick={() => onMove(i, 1)}
                  >
                    ↓
                  </button>
                  <button
                    type="button"
                    className={ACT + " text-[var(--coral)]"}
                    title={t.rowDel}
                    aria-label={t.rowDel}
                    onClick={() => onDel(i)}
                  >
                    ×
                  </button>
                </div>
              </td>
            </tr>
          ))}
        </tbody>
      </table>  );
}
